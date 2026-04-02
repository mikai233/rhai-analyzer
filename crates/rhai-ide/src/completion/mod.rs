mod context;
mod docs;
mod postfix;
mod ranking;
mod snippets;
mod workspace;

use rhai_db::{DatabaseSnapshot, SignatureMatchQuality, signature_match_quality};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_syntax::{TextRange, TextSize};

use crate::completion::context::completion_context;
use crate::completion::docs::doc_completion_items;
use crate::completion::postfix::postfix_completion_items;
use crate::completion::ranking::rank_completion_items;
use crate::completion::snippets::{
    callable_completion_text_edit, callable_completion_text_edit_with_base_text,
    callable_parameter_names,
};
use crate::completion::workspace::{
    builtin_function_annotation, builtin_function_detail, builtin_function_docs,
    inferred_completion_type, workspace_completion_metadata,
};
use crate::support::convert::format_type_ref;
use crate::types::{
    CompletionRelevance, CompletionRelevanceActiveParameterMatch,
    CompletionRelevanceCallableArityMatch, CompletionRelevanceCallableMatch,
    CompletionRelevanceCallableSignatureMatch, CompletionRelevanceNameMatch,
    CompletionRelevanceTypeMatch,
};
use crate::{
    CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource,
    CompletionResolveData, FilePosition, TextEdit,
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

#[derive(Debug, Clone, Copy)]
struct CompletionCallContext<'a> {
    argument_count: Option<usize>,
    argument_types: Option<&'a [Option<TypeRef>]>,
    active_parameter_index: Option<usize>,
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

    let query_offset = TextSize::from(context.query_offset as u32);
    let expected_type = snapshot.expected_type_at(position.file_id, query_offset);
    let call_argument_count = snapshot.call_argument_count_at(position.file_id, query_offset);
    let call_argument_types = snapshot.call_argument_types_at(position.file_id, query_offset);
    let active_parameter_index = snapshot.active_parameter_index_at(position.file_id, query_offset);
    let call_context = CompletionCallContext {
        argument_count: call_argument_count,
        argument_types: call_argument_types.as_deref(),
        active_parameter_index,
    };

    if let Some(module_path) = &context.module_path {
        let mut items = snapshot
            .imported_module_completions(position.file_id, module_path)
            .into_iter()
            .map(|item| {
                let name_match = completion_name_match(item.name.as_str(), &context);
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
                    relevance: CompletionRelevance {
                        name_match,
                        callable_match: completion_callable_match(
                            &context,
                            item.annotation.as_ref(),
                            CompletionItemKind::Symbol(item.kind),
                        ),
                        callable_arity_match: completion_callable_arity_match(
                            &context,
                            item.annotation.as_ref(),
                            None,
                        ),
                        callable_signature_match: completion_callable_signature_match(
                            &context,
                            item.annotation.as_ref(),
                            None,
                        ),
                        active_parameter_match: completion_active_parameter_match(
                            &context,
                            item.annotation.as_ref(),
                            call_context,
                        ),
                        ..CompletionRelevance::default()
                    },
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
        items.extend(
            snapshot
                .auto_import_candidates(position.file_id, query_offset)
                .into_iter()
                .map(|candidate| {
                    let parameter_names = callable_parameter_names(
                        snapshot,
                        candidate.provider_file_id,
                        candidate.symbol,
                        candidate.annotation.as_ref(),
                    );
                    let mut text_edit = callable_completion_text_edit_with_base_text(
                        &context,
                        candidate.qualified_reference_text.as_str(),
                        candidate.annotation.as_ref(),
                        CompletionItemKind::Symbol(candidate.kind),
                        parameter_names.as_deref(),
                    )
                    .unwrap_or_else(|| crate::CompletionTextEdit {
                        replace_range: candidate.replace_range,
                        insert_range: Some(context.replace_range),
                        new_text: candidate.qualified_reference_text.clone(),
                        additional_edits: Vec::new(),
                    });
                    if !candidate.insert_text.is_empty() {
                        text_edit.additional_edits.push(TextEdit::insert(
                            candidate.insertion_offset,
                            candidate.insert_text.clone(),
                        ));
                    }
                    let insert_format = if candidate
                        .annotation
                        .as_ref()
                        .is_some_and(|annotation| matches!(annotation, TypeRef::Function(_)))
                        || matches!(candidate.kind, SymbolKind::Function)
                    {
                        CompletionInsertFormat::Snippet
                    } else {
                        CompletionInsertFormat::PlainText
                    };

                    CompletionItem {
                        label: candidate.name.clone(),
                        kind: CompletionItemKind::Symbol(candidate.kind),
                        source: CompletionItemSource::AutoImport,
                        relevance: CompletionRelevance {
                            requires_import: !candidate.insert_text.is_empty(),
                            import_cost: Some(candidate.import_cost),
                            name_match: completion_name_match(candidate.name.as_str(), &context),
                            callable_match: completion_callable_match(
                                &context,
                                candidate.annotation.as_ref(),
                                CompletionItemKind::Symbol(candidate.kind),
                            ),
                            callable_arity_match: completion_callable_arity_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context.argument_count,
                            ),
                            callable_signature_match: completion_callable_signature_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context.argument_types,
                            ),
                            active_parameter_match: completion_active_parameter_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context,
                            ),
                            type_match: completion_type_match(
                                expected_type.as_ref(),
                                candidate.annotation.as_ref(),
                            ),
                            ..CompletionRelevance::default()
                        },
                        origin: Some(candidate.module_name.clone()),
                        sort_text: String::new(),
                        detail: candidate.annotation.as_ref().map(format_type_ref),
                        docs: if matches!(detail_level, CompletionDetailLevel::Full) {
                            candidate.docs.clone()
                        } else {
                            None
                        },
                        filter_text: Some(candidate.name.clone()),
                        text_edit: Some(text_edit),
                        insert_format,
                        file_id: Some(candidate.provider_file_id),
                        exported: true,
                        resolve_data: None,
                    }
                }),
        );
        rank_completion_items(&mut items, &context);
        return items;
    }

    let Some(inputs) = snapshot.completion_inputs(
        position.file_id,
        TextSize::from(context.query_offset as u32),
    ) else {
        return Vec::new();
    };
    let member_only_context = context.member_access;

    let mut items = Vec::new();
    let hir = snapshot.hir(position.file_id);

    items.extend(postfix_completion_items(&context));

    if !member_only_context {
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
                relevance: CompletionRelevance {
                    is_local: matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Parameter),
                    scope_distance: matches!(
                        symbol.kind,
                        SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::ImportAlias
                    )
                    .then_some(symbol.scope_distance),
                    name_match: completion_name_match(symbol.name.as_str(), &context),
                    callable_match: completion_callable_match(
                        &context,
                        annotation,
                        CompletionItemKind::Symbol(symbol.kind),
                    ),
                    callable_arity_match: completion_callable_arity_match(
                        &context,
                        annotation,
                        call_context.argument_count,
                    ),
                    callable_signature_match: completion_callable_signature_match(
                        &context,
                        annotation,
                        call_context.argument_types,
                    ),
                    active_parameter_match: completion_active_parameter_match(
                        &context,
                        annotation,
                        call_context,
                    ),
                    type_match: completion_type_match(expected_type.as_ref(), annotation),
                    ..CompletionRelevance::default()
                },
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
                relevance: CompletionRelevance {
                    name_match: completion_name_match(symbol.symbol.name.as_str(), &context),
                    callable_match: completion_callable_match(
                        &context,
                        annotation.as_ref(),
                        CompletionItemKind::Symbol(symbol.symbol.kind),
                    ),
                    callable_arity_match: completion_callable_arity_match(
                        &context,
                        annotation.as_ref(),
                        call_context.argument_count,
                    ),
                    callable_signature_match: completion_callable_signature_match(
                        &context,
                        annotation.as_ref(),
                        call_context.argument_types,
                    ),
                    active_parameter_match: completion_active_parameter_match(
                        &context,
                        annotation.as_ref(),
                        call_context,
                    ),
                    type_match: completion_type_match(expected_type.as_ref(), annotation.as_ref()),
                    ..CompletionRelevance::default()
                },
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

        items.extend(
            snapshot
                .auto_import_candidates(position.file_id, query_offset)
                .into_iter()
                .filter(|candidate| {
                    context.prefix.is_empty()
                        || candidate
                            .name
                            .to_ascii_lowercase()
                            .contains(context.prefix.to_ascii_lowercase().as_str())
                })
                .map(|candidate| {
                    let parameter_names = callable_parameter_names(
                        snapshot,
                        candidate.provider_file_id,
                        candidate.symbol,
                        candidate.annotation.as_ref(),
                    );
                    let mut text_edit = callable_completion_text_edit_with_base_text(
                        &context,
                        candidate.qualified_reference_text.as_str(),
                        candidate.annotation.as_ref(),
                        CompletionItemKind::Symbol(candidate.kind),
                        parameter_names.as_deref(),
                    )
                    .unwrap_or_else(|| crate::CompletionTextEdit {
                        replace_range: candidate.replace_range,
                        insert_range: Some(context.replace_range),
                        new_text: candidate.qualified_reference_text.clone(),
                        additional_edits: Vec::new(),
                    });
                    if !candidate.insert_text.is_empty() {
                        text_edit.additional_edits.push(TextEdit::insert(
                            candidate.insertion_offset,
                            candidate.insert_text.clone(),
                        ));
                    }
                    let insert_format = if candidate
                        .annotation
                        .as_ref()
                        .is_some_and(|annotation| matches!(annotation, TypeRef::Function(_)))
                        || matches!(candidate.kind, SymbolKind::Function)
                    {
                        CompletionInsertFormat::Snippet
                    } else {
                        CompletionInsertFormat::PlainText
                    };

                    CompletionItem {
                        label: candidate.name.clone(),
                        kind: CompletionItemKind::Symbol(candidate.kind),
                        source: CompletionItemSource::AutoImport,
                        relevance: CompletionRelevance {
                            requires_import: !candidate.insert_text.is_empty(),
                            import_cost: Some(candidate.import_cost),
                            name_match: completion_name_match(candidate.name.as_str(), &context),
                            callable_match: completion_callable_match(
                                &context,
                                candidate.annotation.as_ref(),
                                CompletionItemKind::Symbol(candidate.kind),
                            ),
                            callable_arity_match: completion_callable_arity_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context.argument_count,
                            ),
                            callable_signature_match: completion_callable_signature_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context.argument_types,
                            ),
                            active_parameter_match: completion_active_parameter_match(
                                &context,
                                candidate.annotation.as_ref(),
                                call_context,
                            ),
                            type_match: completion_type_match(
                                expected_type.as_ref(),
                                candidate.annotation.as_ref(),
                            ),
                            ..CompletionRelevance::default()
                        },
                        origin: Some(candidate.module_name.clone()),
                        sort_text: String::new(),
                        detail: candidate.annotation.as_ref().map(format_type_ref),
                        docs: if matches!(detail_level, CompletionDetailLevel::Full) {
                            candidate.docs.clone()
                        } else {
                            None
                        },
                        filter_text: Some(candidate.name.clone()),
                        text_edit: Some(text_edit),
                        insert_format,
                        file_id: Some(candidate.provider_file_id),
                        exported: true,
                        resolve_data: None,
                    }
                }),
        );

        let existing_labels = items
            .iter()
            .map(|item| item.label.clone())
            .collect::<std::collections::HashSet<_>>();
        items.extend(
            builtin_global_completion_items(
                snapshot,
                &context,
                expected_type.as_ref(),
                call_context,
            )
            .into_iter()
            .filter(|item| !existing_labels.contains(item.label.as_str())),
        );
    }

    items.extend(inputs.member_symbols.iter().flat_map(|member| {
        member_completion_items(
            member,
            &context,
            position,
            expected_type.as_ref(),
            call_context,
        )
    }));

    rank_completion_items(&mut items, &context);
    items
}

fn member_completion_items(
    member: &rhai_hir::MemberCompletion,
    context: &CompletionContext,
    position: FilePosition,
    expected_type: Option<&TypeRef>,
    call_context: CompletionCallContext<'_>,
) -> Vec<CompletionItem> {
    if member.callable_overloads.len() > 1 {
        let mut overloads = member.callable_overloads.clone();
        overloads.sort_by(|left, right| {
            left.params.len().cmp(&right.params.len()).then_with(|| {
                format_type_ref(&TypeRef::Function(left.clone()))
                    .cmp(&format_type_ref(&TypeRef::Function(right.clone())))
            })
        });

        return overloads
            .into_iter()
            .map(|signature| {
                let annotation = TypeRef::Function(signature);
                member_completion_item(
                    member,
                    Some(&annotation),
                    context,
                    position,
                    expected_type,
                    call_context,
                )
            })
            .collect();
    }

    vec![member_completion_item(
        member,
        member.annotation.as_ref(),
        context,
        position,
        expected_type,
        call_context,
    )]
}

fn member_completion_item(
    member: &rhai_hir::MemberCompletion,
    annotation: Option<&TypeRef>,
    context: &CompletionContext,
    position: FilePosition,
    expected_type: Option<&TypeRef>,
    call_context: CompletionCallContext<'_>,
) -> CompletionItem {
    let text_edit = callable_completion_text_edit(
        context,
        member.name.as_str(),
        annotation,
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
        relevance: CompletionRelevance {
            name_match: completion_name_match(member.name.as_str(), context),
            callable_match: completion_callable_match(
                context,
                annotation,
                CompletionItemKind::Member,
            ),
            callable_arity_match: completion_callable_arity_match(
                context,
                annotation,
                call_context.argument_count,
            ),
            callable_signature_match: completion_callable_signature_match(
                context,
                annotation,
                call_context.argument_types,
            ),
            active_parameter_match: completion_active_parameter_match(
                context,
                annotation,
                call_context,
            ),
            type_match: completion_type_match(expected_type, annotation),
            ..CompletionRelevance::default()
        },
        origin: None,
        sort_text: String::new(),
        detail: annotation.map(format_type_ref),
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
}

fn builtin_global_completion_items(
    snapshot: &DatabaseSnapshot,
    context: &CompletionContext,
    expected_type: Option<&TypeRef>,
    call_context: CompletionCallContext<'_>,
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
                relevance: CompletionRelevance {
                    name_match: completion_name_match(function.name.as_str(), context),
                    callable_match: completion_callable_match(
                        context,
                        annotation.as_ref(),
                        CompletionItemKind::Symbol(SymbolKind::Function),
                    ),
                    callable_arity_match: completion_callable_arity_match(
                        context,
                        annotation.as_ref(),
                        call_context.argument_count,
                    ),
                    callable_signature_match: completion_callable_signature_match(
                        context,
                        annotation.as_ref(),
                        call_context.argument_types,
                    ),
                    active_parameter_match: completion_active_parameter_match(
                        context,
                        annotation.as_ref(),
                        call_context,
                    ),
                    type_match: completion_type_match(expected_type, annotation.as_ref()),
                    ..CompletionRelevance::default()
                },
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

fn completion_type_match(
    expected: Option<&TypeRef>,
    candidate: Option<&TypeRef>,
) -> Option<CompletionRelevanceTypeMatch> {
    let (Some(expected), Some(candidate)) = (expected, candidate) else {
        return None;
    };

    if types_match_exact(expected, candidate) {
        return Some(CompletionRelevanceTypeMatch::Exact);
    }

    types_could_unify(expected, candidate).then_some(CompletionRelevanceTypeMatch::CouldUnify)
}

fn completion_name_match(
    label: &str,
    context: &CompletionContext,
) -> Option<CompletionRelevanceNameMatch> {
    let prefix = context.prefix.as_str();
    if prefix.is_empty() {
        return None;
    }

    let label_lower = label.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();

    if label_lower == prefix_lower {
        Some(CompletionRelevanceNameMatch::Exact)
    } else if label_lower.starts_with(prefix_lower.as_str()) {
        Some(CompletionRelevanceNameMatch::Prefix)
    } else if name_matches_subsequence(label, prefix) {
        Some(CompletionRelevanceNameMatch::Subsequence)
    } else if label_lower.contains(prefix_lower.as_str()) {
        Some(CompletionRelevanceNameMatch::Contains)
    } else {
        None
    }
}

fn name_matches_subsequence(label: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }

    let mut prefix_chars = prefix.chars().map(|ch| ch.to_ascii_lowercase());
    let Some(mut needle) = prefix_chars.next() else {
        return false;
    };

    for ch in label.chars().map(|ch| ch.to_ascii_lowercase()) {
        if ch == needle {
            match prefix_chars.next() {
                Some(next) => needle = next,
                None => return true,
            }
        }
    }

    false
}

fn completion_callable_match(
    context: &CompletionContext,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
) -> Option<CompletionRelevanceCallableMatch> {
    if !context.next_char_is_open_paren {
        return None;
    }

    match (annotation, kind) {
        (Some(TypeRef::Function(_)), _) => Some(CompletionRelevanceCallableMatch::Invocable),
        (_, CompletionItemKind::Symbol(SymbolKind::Function)) => {
            Some(CompletionRelevanceCallableMatch::Invocable)
        }
        _ => None,
    }
}

fn completion_callable_arity_match(
    context: &CompletionContext,
    annotation: Option<&TypeRef>,
    call_argument_count: Option<usize>,
) -> Option<CompletionRelevanceCallableArityMatch> {
    if !context.next_char_is_open_paren {
        return None;
    }

    match annotation {
        Some(TypeRef::Function(signature))
            if call_argument_count.is_some_and(|count| signature.params.len() == count) =>
        {
            Some(CompletionRelevanceCallableArityMatch::Exact)
        }
        _ => None,
    }
}

fn completion_callable_signature_match(
    context: &CompletionContext,
    annotation: Option<&TypeRef>,
    call_argument_types: Option<&[Option<TypeRef>]>,
) -> Option<CompletionRelevanceCallableSignatureMatch> {
    if !context.next_char_is_open_paren {
        return None;
    }

    let (Some(TypeRef::Function(signature)), Some(arg_types)) = (annotation, call_argument_types)
    else {
        return None;
    };

    match signature_match_quality(signature, arg_types)? {
        SignatureMatchQuality::Exact => Some(CompletionRelevanceCallableSignatureMatch::Exact),
        SignatureMatchQuality::Partial => Some(CompletionRelevanceCallableSignatureMatch::Partial),
        SignatureMatchQuality::Unknown | SignatureMatchQuality::Mismatch => None,
    }
}

fn completion_active_parameter_match(
    context: &CompletionContext,
    annotation: Option<&TypeRef>,
    call_context: CompletionCallContext<'_>,
) -> Option<CompletionRelevanceActiveParameterMatch> {
    if !context.next_char_is_open_paren {
        return None;
    }

    let (Some(TypeRef::Function(signature)), Some(index), Some(arg_types)) = (
        annotation,
        call_context.active_parameter_index,
        call_context.argument_types,
    ) else {
        return None;
    };
    let actual = arg_types.get(index).and_then(|ty| ty.as_ref())?;
    let expected = signature.params.get(index)?;
    if !is_informative_type(expected) || !is_informative_type(actual) {
        return None;
    }

    if types_match_exact(expected, actual) {
        Some(CompletionRelevanceActiveParameterMatch::Exact)
    } else if types_could_unify(expected, actual) {
        Some(CompletionRelevanceActiveParameterMatch::Partial)
    } else {
        None
    }
}

fn is_informative_type(ty: &TypeRef) -> bool {
    !matches!(
        ty,
        TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never
    )
}

fn types_match_exact(expected: &TypeRef, candidate: &TypeRef) -> bool {
    expected == candidate
}

fn types_could_unify(expected: &TypeRef, candidate: &TypeRef) -> bool {
    if expected == candidate {
        return true;
    }

    match (expected, candidate) {
        (TypeRef::Nullable(expected), other) | (other, TypeRef::Nullable(expected)) => {
            types_could_unify(expected, other)
        }
        (TypeRef::Union(items), other) | (TypeRef::Ambiguous(items), other) => {
            items.iter().any(|item| types_could_unify(item, other))
        }
        (other, TypeRef::Union(items)) | (other, TypeRef::Ambiguous(items)) => {
            items.iter().any(|item| types_could_unify(other, item))
        }
        (TypeRef::FnPtr, TypeRef::Function(_)) | (TypeRef::Function(_), TypeRef::FnPtr) => true,
        (TypeRef::Array(expected), TypeRef::Array(candidate)) => {
            types_could_unify(expected, candidate)
        }
        (
            TypeRef::Map(expected_key, expected_value),
            TypeRef::Map(candidate_key, candidate_value),
        ) => {
            types_could_unify(expected_key, candidate_key)
                && types_could_unify(expected_value, candidate_value)
        }
        (TypeRef::Object(expected_fields), TypeRef::Object(candidate_fields)) => {
            expected_fields.iter().all(|(name, expected)| {
                candidate_fields
                    .get(name)
                    .is_some_and(|candidate| types_could_unify(expected, candidate))
            })
        }
        (
            TypeRef::Applied {
                name: expected_name,
                args: expected_args,
            },
            TypeRef::Applied {
                name: candidate_name,
                args: candidate_args,
            },
        ) => {
            expected_name == candidate_name
                && expected_args.len() == candidate_args.len()
                && expected_args
                    .iter()
                    .zip(candidate_args.iter())
                    .all(|(expected, candidate)| types_could_unify(expected, candidate))
        }
        (TypeRef::Named(expected), TypeRef::Named(candidate)) => expected == candidate,
        (TypeRef::Named(expected), TypeRef::Applied { name, .. })
        | (TypeRef::Applied { name, .. }, TypeRef::Named(expected)) => expected == name,
        (TypeRef::Function(expected), TypeRef::Function(candidate)) => {
            expected.params.len() == candidate.params.len()
                && expected
                    .params
                    .iter()
                    .zip(candidate.params.iter())
                    .all(|(expected, candidate)| types_could_unify(expected, candidate))
                && types_could_unify(expected.ret.as_ref(), candidate.ret.as_ref())
        }
        _ => false,
    }
}

pub(super) fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

#[cfg(test)]
mod tests {
    use crate::CompletionInsertFormat;
    use crate::completion::ranking::{rank_completion_items, source_rank};
    use crate::types::{CompletionRelevance, CompletionRelevancePostfixMatch};
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
            relevance: CompletionRelevance::default(),
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
            source_rank(CompletionItemSource::Module, &test_context(false))
                < source_rank(CompletionItemSource::Project, &test_context(false))
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

    #[test]
    fn exact_postfix_matches_rank_ahead_of_other_candidates() {
        let context = CompletionContext {
            prefix: "switch".to_owned(),
            ..test_context(true)
        };
        let mut items = vec![
            CompletionItem {
                label: "switch".to_owned(),
                kind: CompletionItemKind::Snippet,
                source: CompletionItemSource::Postfix,
                relevance: CompletionRelevance {
                    postfix_match: Some(CompletionRelevancePostfixMatch::Exact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: Some("switch".to_owned()),
                text_edit: None,
                insert_format: CompletionInsertFormat::Snippet,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "substring".to_owned(),
                kind: CompletionItemKind::Member,
                source: CompletionItemSource::Member,
                relevance: CompletionRelevance::default(),
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "switch");
        assert_eq!(items[0].source, CompletionItemSource::Postfix);
    }

    #[test]
    fn non_exact_postfix_matches_rank_after_normal_candidates() {
        let context = CompletionContext {
            prefix: "s".to_owned(),
            ..test_context(true)
        };
        let mut items = vec![
            CompletionItem {
                label: "switch".to_owned(),
                kind: CompletionItemKind::Snippet,
                source: CompletionItemSource::Postfix,
                relevance: CompletionRelevance {
                    postfix_match: Some(CompletionRelevancePostfixMatch::NonExact),
                    ..CompletionRelevance::default()
                },
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: Some("switch".to_owned()),
                text_edit: None,
                insert_format: CompletionInsertFormat::Snippet,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
            CompletionItem {
                label: "split".to_owned(),
                kind: CompletionItemKind::Member,
                source: CompletionItemSource::Member,
                relevance: CompletionRelevance::default(),
                origin: None,
                sort_text: String::new(),
                detail: None,
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: false,
                resolve_data: None,
            },
        ];

        rank_completion_items(&mut items, &context);

        assert_eq!(items[0].label, "split");
        assert_eq!(items[1].label, "switch");
    }
}
