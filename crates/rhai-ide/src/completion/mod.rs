mod context;
mod docs;
mod postfix;
mod ranking;
mod snippets;
mod workspace;

use rhai_db::DatabaseSnapshot;
use rhai_hir::{SymbolKind, TypeRef};
use rhai_syntax::{TextRange, TextSize};

use crate::completion::context::completion_context;
use crate::completion::docs::doc_completion_items;
use crate::completion::postfix::postfix_completion_items;
use crate::completion::ranking::rank_completion_items;
use crate::completion::snippets::{callable_completion_text_edit, callable_parameter_names};
use crate::completion::workspace::{
    builtin_function_annotation, builtin_function_detail, builtin_function_docs,
    inferred_completion_type, workspace_completion_metadata,
};
use crate::support::convert::format_type_ref;
use crate::{
    CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource,
    CompletionResolveData, FilePosition,
};

pub(crate) fn completions(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> Vec<CompletionItem> {
    completion_items(snapshot, position, CompletionDetailLevel::Basic)
}

pub(crate) fn resolve_completion(
    snapshot: &DatabaseSnapshot,
    item: CompletionItem,
) -> CompletionItem {
    let Some(resolve_data) = item.resolve_data.clone() else {
        return item;
    };

    completion_items(
        snapshot,
        FilePosition {
            file_id: resolve_data.file_id,
            offset: resolve_data.offset,
        },
        CompletionDetailLevel::Full,
    )
    .into_iter()
    .find(|candidate| same_completion_item(candidate, &item))
    .unwrap_or(item)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionDetailLevel {
    Basic,
    Full,
}

#[derive(Debug, Clone)]
pub(super) struct CompletionContext {
    prefix: String,
    replace_range: TextRange,
    query_offset: usize,
    member_access: bool,
    module_path: Option<Vec<String>>,
    postfix_completion: Option<PostfixCompletionContext>,
    suppress_completion: bool,
    doc_completion: Option<DocCompletionContext>,
    next_char_is_open_paren: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PostfixCompletionContext {
    receiver_text: String,
    replace_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DocCompletionContext {
    Tag {
        prefix: String,
        replace_range: TextRange,
    },
    Type {
        prefix: String,
        replace_range: TextRange,
    },
}

fn completion_items(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    detail_level: CompletionDetailLevel,
) -> Vec<CompletionItem> {
    let context = completion_context(snapshot, position);
    if let Some(doc_context) = &context.doc_completion {
        let mut items = doc_completion_items(doc_context);
        rank_completion_items(&mut items, &context);
        return items;
    }

    if context.suppress_completion {
        return Vec::new();
    }

    if let Some(module_path) = &context.module_path {
        let mut items = snapshot
            .imported_module_completions(position.file_id, module_path)
            .into_iter()
            .map(|item| {
                let parameter_names =
                    item.file_id.zip(item.symbol).and_then(|(file_id, symbol)| {
                        callable_parameter_names(
                            snapshot,
                            file_id,
                            symbol,
                            item.annotation.as_ref(),
                        )
                    });
                let text_edit = callable_completion_text_edit(
                    &context,
                    item.name.as_str(),
                    item.annotation.as_ref(),
                    CompletionItemKind::Symbol(item.kind),
                    parameter_names.as_deref(),
                );
                let insert_format = if text_edit.is_some() {
                    CompletionInsertFormat::Snippet
                } else {
                    CompletionInsertFormat::PlainText
                };
                CompletionItem {
                    label: item.name,
                    kind: CompletionItemKind::Symbol(item.kind),
                    source: CompletionItemSource::Module,
                    origin: item.origin,
                    sort_text: String::new(),
                    detail: item.annotation.as_ref().map(format_type_ref),
                    docs: if matches!(detail_level, CompletionDetailLevel::Full) {
                        item.docs
                    } else {
                        None
                    },
                    filter_text: None,
                    text_edit,
                    insert_format,
                    file_id: None,
                    exported: true,
                    resolve_data: None,
                }
            })
            .collect::<Vec<_>>();
        rank_completion_items(&mut items, &context);
        return items;
    }

    let Some(inputs) = snapshot.completion_inputs(
        position.file_id,
        TextSize::from(context.query_offset as u32),
    ) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let hir = snapshot.hir(position.file_id);

    items.extend(postfix_completion_items(&context));

    items.extend(inputs.visible_symbols.iter().map(|symbol| {
        let docs = match (detail_level, &hir, symbol.docs) {
            (CompletionDetailLevel::Full, Some(hir), Some(docs)) => {
                Some(hir.doc_block(docs).text.clone())
            }
            _ => None,
        };
        let annotation = symbol
            .annotation
            .as_ref()
            .or_else(|| inferred_completion_type(snapshot, position.file_id, symbol.symbol));
        let parameter_names =
            callable_parameter_names(snapshot, position.file_id, symbol.symbol, annotation);
        let text_edit = callable_completion_text_edit(
            &context,
            symbol.name.as_str(),
            annotation,
            CompletionItemKind::Symbol(symbol.kind),
            parameter_names.as_deref(),
        );
        let insert_format = if text_edit.is_some() {
            CompletionInsertFormat::Snippet
        } else {
            CompletionInsertFormat::PlainText
        };

        CompletionItem {
            label: symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.kind),
            source: CompletionItemSource::Visible,
            origin: None,
            sort_text: String::new(),
            detail: annotation
                .filter(|ty| !matches!(ty, TypeRef::Unknown))
                .map(format_type_ref),
            docs,
            filter_text: None,
            text_edit,
            insert_format,
            file_id: Some(position.file_id),
            exported: false,
            resolve_data: Some(CompletionResolveData {
                file_id: position.file_id,
                offset: position.offset,
            }),
        }
    }));

    items.extend(inputs.project_symbols.iter().map(|symbol| {
        let (detail, docs, annotation, origin) =
            workspace_completion_metadata(snapshot, symbol, detail_level);
        let parameter_names = callable_parameter_names(
            snapshot,
            symbol.file_id,
            symbol.symbol.symbol,
            annotation.as_ref(),
        );
        let text_edit = callable_completion_text_edit(
            &context,
            symbol.symbol.name.as_str(),
            annotation.as_ref(),
            CompletionItemKind::Symbol(symbol.symbol.kind),
            parameter_names.as_deref(),
        );
        let insert_format = if text_edit.is_some() {
            CompletionInsertFormat::Snippet
        } else {
            CompletionInsertFormat::PlainText
        };

        CompletionItem {
            label: symbol.symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.symbol.kind),
            source: CompletionItemSource::Project,
            origin,
            sort_text: String::new(),
            detail,
            docs,
            filter_text: None,
            text_edit,
            insert_format,
            file_id: Some(symbol.file_id),
            exported: symbol.symbol.exported,
            resolve_data: Some(CompletionResolveData {
                file_id: position.file_id,
                offset: position.offset,
            }),
        }
    }));

    let existing_labels = items
        .iter()
        .map(|item| item.label.clone())
        .collect::<std::collections::HashSet<_>>();
    items.extend(
        builtin_global_completion_items(snapshot, &context)
            .into_iter()
            .filter(|item| !existing_labels.contains(item.label.as_str())),
    );

    items.extend(inputs.member_symbols.iter().map(|member| {
        let text_edit = callable_completion_text_edit(
            &context,
            member.name.as_str(),
            member.annotation.as_ref(),
            CompletionItemKind::Member,
            None,
        );
        let insert_format = if text_edit.is_some() {
            CompletionInsertFormat::Snippet
        } else {
            CompletionInsertFormat::PlainText
        };
        CompletionItem {
            label: member.name.clone(),
            kind: CompletionItemKind::Member,
            source: CompletionItemSource::Member,
            origin: None,
            sort_text: String::new(),
            detail: member.annotation.as_ref().map(format_type_ref),
            docs: member.docs.clone(),
            filter_text: None,
            text_edit,
            insert_format,
            file_id: None,
            exported: false,
            resolve_data: Some(CompletionResolveData {
                file_id: position.file_id,
                offset: position.offset,
            }),
        }
    }));

    rank_completion_items(&mut items, &context);
    items
}

fn builtin_global_completion_items(
    snapshot: &DatabaseSnapshot,
    context: &CompletionContext,
) -> Vec<CompletionItem> {
    snapshot
        .global_functions()
        .iter()
        .map(|function| {
            let annotation = builtin_function_annotation(function);
            let docs = builtin_function_docs(function);
            let text_edit = callable_completion_text_edit(
                context,
                function.name.as_str(),
                annotation.as_ref(),
                CompletionItemKind::Symbol(SymbolKind::Function),
                None,
            );
            let insert_format = if text_edit.is_some() {
                CompletionInsertFormat::Snippet
            } else {
                CompletionInsertFormat::PlainText
            };

            CompletionItem {
                label: function.name.clone(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Builtin,
                origin: None,
                sort_text: String::new(),
                detail: builtin_function_detail(function, annotation.as_ref()),
                docs,
                filter_text: None,
                text_edit,
                insert_format,
                file_id: None,
                exported: false,
                resolve_data: None,
            }
        })
        .collect()
}

fn same_completion_item(left: &CompletionItem, right: &CompletionItem) -> bool {
    left.label == right.label
        && left.kind == right.kind
        && left.source == right.source
        && left.origin == right.origin
        && left.detail == right.detail
        && left.file_id == right.file_id
        && left.exported == right.exported
}

pub(super) fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

#[cfg(test)]
mod tests {
    use crate::CompletionInsertFormat;
    use crate::completion::ranking::source_rank;
    use crate::{CompletionItem, CompletionItemKind, CompletionItemSource};
    use rhai_hir::SymbolKind;
    use rhai_syntax::{TextRange, TextSize};

    use crate::completion::CompletionContext;

    fn test_context(member_access: bool) -> CompletionContext {
        CompletionContext {
            prefix: "sha".to_owned(),
            replace_range: TextRange::empty(TextSize::from(0)),
            query_offset: 0,
            member_access,
            module_path: None,
            postfix_completion: None,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: false,
        }
    }

    fn test_item(source: CompletionItemSource) -> CompletionItem {
        CompletionItem {
            label: "shared_helper".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source,
            origin: None,
            sort_text: String::new(),
            detail: None,
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::PlainText,
            file_id: None,
            exported: true,
            resolve_data: None,
        }
    }

    #[test]
    fn module_candidates_rank_ahead_of_project_candidates() {
        assert!(
            source_rank(CompletionItemSource::Module, false)
                < source_rank(CompletionItemSource::Project, false)
        );

        let context = test_context(false);
        let mut items = vec![
            test_item(CompletionItemSource::Project),
            test_item(CompletionItemSource::Module),
        ];

        crate::completion::ranking::rank_completion_items(&mut items, &context);

        assert_eq!(items[0].source, CompletionItemSource::Module);
        assert_eq!(items[1].source, CompletionItemSource::Project);
    }
}
