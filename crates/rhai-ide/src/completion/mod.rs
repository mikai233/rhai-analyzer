use rhai_db::DatabaseSnapshot;
use rhai_hir::{FunctionTypeRef, SymbolId, SymbolKind, TypeRef};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;

use crate::support::convert::format_type_ref;
use crate::types::CompletionTextEdit;
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
enum CompletionDetailLevel {
    Basic,
    Full,
}

#[derive(Debug, Clone)]
struct CompletionContext {
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
struct PostfixCompletionContext {
    receiver_text: String,
    replace_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocCompletionContext {
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
                    source: CompletionItemSource::Project,
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
        let (detail, docs, annotation) =
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
            sort_text: String::new(),
            detail: member.annotation.as_ref().map(format_type_ref),
            docs: None,
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
        && left.file_id == right.file_id
        && left.exported == right.exported
}

fn callable_completion_text_edit(
    context: &CompletionContext,
    label: &str,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
    parameter_names: Option<&[String]>,
) -> Option<CompletionTextEdit> {
    if context.next_char_is_open_paren {
        return None;
    }

    let snippet = completion_call_snippet(label, annotation, kind, parameter_names)?;
    Some(CompletionTextEdit {
        range: context.replace_range,
        new_text: snippet,
    })
}

fn completion_call_snippet(
    label: &str,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
    parameter_names: Option<&[String]>,
) -> Option<String> {
    match annotation {
        Some(TypeRef::Function(signature)) => {
            Some(function_call_snippet(label, signature, parameter_names))
        }
        _ if matches!(kind, CompletionItemKind::Symbol(SymbolKind::Function)) => {
            Some(format!("{label}()$0"))
        }
        _ => None,
    }
}

fn function_call_snippet(
    label: &str,
    signature: &FunctionTypeRef,
    parameter_names: Option<&[String]>,
) -> String {
    if signature.params.is_empty() {
        return format!("{label}()$0");
    }

    let placeholders = signature
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let tabstop = index + 1;
            let placeholder = snippet_placeholder(
                parameter,
                parameter_names.and_then(|names| names.get(index).map(String::as_str)),
                tabstop,
            );
            format!("${{{tabstop}:{placeholder}}}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{label}({placeholders})$0")
}

fn snippet_placeholder(parameter: &TypeRef, parameter_name: Option<&str>, index: usize) -> String {
    if let Some(name) = parameter_name
        && !name.is_empty()
    {
        return name.to_owned();
    }

    let label = format_type_ref(parameter);
    if label.is_empty() || label == "unknown" {
        format!("arg{index}")
    } else {
        label
    }
}

fn callable_parameter_names(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    symbol: SymbolId,
    annotation: Option<&TypeRef>,
) -> Option<Vec<String>> {
    let TypeRef::Function(signature) = annotation? else {
        return None;
    };
    let hir = snapshot.hir(file_id)?;
    let symbol_data = hir.symbol(symbol);
    if symbol_data.kind != SymbolKind::Function {
        return None;
    }

    let names = hir
        .function_parameters(symbol)
        .into_iter()
        .map(|parameter| hir.symbol(parameter).name.clone())
        .collect::<Vec<_>>();
    (names.len() == signature.params.len()).then_some(names)
}

fn completion_context(snapshot: &DatabaseSnapshot, position: FilePosition) -> CompletionContext {
    let Some(text) = snapshot.file_text(position.file_id) else {
        return CompletionContext {
            prefix: String::new(),
            replace_range: text_range(0, 0),
            query_offset: 0,
            member_access: false,
            module_path: None,
            postfix_completion: None,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: false,
        };
    };
    let offset = usize::try_from(position.offset)
        .unwrap_or(usize::MAX)
        .min(text.len());
    let bytes = text.as_bytes();
    if bytes.get(offset).copied() == Some(b'.') {
        let postfix_completion = postfix_completion_context_at_dot(text.as_ref(), offset);
        return CompletionContext {
            prefix: String::new(),
            replace_range: text_range(offset + 1, offset + 1),
            query_offset: offset + 1,
            member_access: true,
            module_path: None,
            postfix_completion,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: bytes.get(offset + 1).copied() == Some(b'('),
        };
    }

    let mut start = offset;

    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let prefix = text[start..offset].to_owned();
    let replace_range = text_range(start, offset);
    let member_access = start > 0 && bytes[start - 1] == b'.';
    let module_path = module_path_before_offset(text.as_ref(), start);
    let postfix_completion = postfix_completion_context(text.as_ref(), start, offset);
    let suppress_completion = single_colon_path_context_before_offset(text.as_ref(), start);
    let doc_completion = doc_completion_context(text.as_ref(), offset);
    let next_char_is_open_paren = bytes.get(offset).copied() == Some(b'(');

    CompletionContext {
        prefix,
        replace_range,
        query_offset: offset,
        member_access,
        module_path,
        postfix_completion,
        suppress_completion,
        doc_completion,
        next_char_is_open_paren,
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn module_path_before_offset(text: &str, prefix_start: usize) -> Option<Vec<String>> {
    if prefix_start < 2 || &text[prefix_start - 2..prefix_start] != "::" {
        return None;
    }

    let bytes = text.as_bytes();
    let mut end = prefix_start - 2;
    let mut parts = Vec::<String>::new();

    loop {
        let mut start = end;
        while start > 0 && is_identifier_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == end {
            return None;
        }
        parts.push(text[start..end].to_owned());
        if start < 2 || &text[start - 2..start] != "::" {
            break;
        }
        end = start - 2;
    }

    parts.reverse();
    Some(parts)
}

fn single_colon_path_context_before_offset(text: &str, prefix_start: usize) -> bool {
    if prefix_start == 0 || text.as_bytes()[prefix_start - 1] != b':' {
        return false;
    }
    if prefix_start >= 2 && &text[prefix_start - 2..prefix_start] == "::" {
        return false;
    }

    let bytes = text.as_bytes();
    let mut segment_start = prefix_start - 1;
    while segment_start > 0 && is_identifier_byte(bytes[segment_start - 1]) {
        segment_start -= 1;
    }

    segment_start < prefix_start - 1
}

fn doc_completion_context(text: &str, offset: usize) -> Option<DocCompletionContext> {
    let offset = offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line = &text[line_start..offset];
    let trimmed = line.trim_start();
    let marker = if trimmed.starts_with("///") {
        "///"
    } else if trimmed.starts_with("//!") {
        "//!"
    } else {
        return None;
    };
    let leading_ws = line.len() - trimmed.len();
    let marker_start = line_start + leading_ws;
    let after_marker_start = marker_start + marker.len();
    let after_marker = &text[after_marker_start..offset];
    let content = after_marker.trim_start();
    let content_start = after_marker_start + (after_marker.len() - content.len());

    if let Some(tag) = content.strip_prefix('@')
        && !tag.contains(char::is_whitespace)
    {
        let prefix_start = content_start + 1;
        return Some(DocCompletionContext::Tag {
            prefix: tag.to_owned(),
            replace_range: text_range(prefix_start, offset),
        });
    }

    let parts = content.split_whitespace().collect::<Vec<_>>();
    let trailing_space = content.chars().last().is_some_and(char::is_whitespace);

    match parts.first().copied() {
        Some("@type") | Some("@return") => {
            let replace_start = if trailing_space {
                offset
            } else {
                offset.saturating_sub(parts.get(1).copied().unwrap_or_default().len())
            };
            let prefix = if trailing_space {
                String::new()
            } else {
                parts.get(1).copied().unwrap_or_default().to_owned()
            };
            Some(DocCompletionContext::Type {
                prefix,
                replace_range: text_range(replace_start, offset),
            })
        }
        Some("@param") | Some("@field") => {
            let replace_start = if trailing_space {
                match parts.len() {
                    0..=2 => return None,
                    _ => offset,
                }
            } else if parts.len() >= 3 {
                offset.saturating_sub(parts.last().copied().unwrap_or_default().len())
            } else {
                return None;
            };
            let prefix = if trailing_space {
                match parts.len() {
                    0..=2 => return None,
                    _ => String::new(),
                }
            } else if parts.len() >= 3 {
                parts.last().copied().unwrap_or_default().to_owned()
            } else {
                return None;
            };
            Some(DocCompletionContext::Type {
                prefix,
                replace_range: text_range(replace_start, offset),
            })
        }
        _ => None,
    }
}

fn doc_completion_items(context: &DocCompletionContext) -> Vec<CompletionItem> {
    match context {
        DocCompletionContext::Tag { replace_range, .. } => doc_tag_completion_items(*replace_range),
        DocCompletionContext::Type { replace_range, .. } => {
            doc_type_completion_items(*replace_range)
        }
    }
}

fn postfix_completion_context(
    text: &str,
    prefix_start: usize,
    offset: usize,
) -> Option<PostfixCompletionContext> {
    if prefix_start == 0 || text.as_bytes().get(prefix_start - 1).copied() != Some(b'.') {
        return None;
    }

    let bytes = text.as_bytes();
    let mut receiver_start = prefix_start - 1;
    while receiver_start > 0 && is_identifier_byte(bytes[receiver_start - 1]) {
        receiver_start -= 1;
    }

    if receiver_start == prefix_start - 1 {
        return None;
    }

    Some(PostfixCompletionContext {
        receiver_text: text[receiver_start..prefix_start - 1].to_owned(),
        replace_range: text_range(receiver_start, offset),
    })
}

fn postfix_completion_context_at_dot(
    text: &str,
    dot_offset: usize,
) -> Option<PostfixCompletionContext> {
    if text.as_bytes().get(dot_offset).copied() != Some(b'.') || dot_offset == 0 {
        return None;
    }

    let bytes = text.as_bytes();
    let mut receiver_start = dot_offset;
    while receiver_start > 0 && is_identifier_byte(bytes[receiver_start - 1]) {
        receiver_start -= 1;
    }

    if receiver_start == dot_offset {
        return None;
    }

    Some(PostfixCompletionContext {
        receiver_text: text[receiver_start..dot_offset].to_owned(),
        replace_range: text_range(receiver_start, dot_offset + 1),
    })
}

fn postfix_completion_items(context: &CompletionContext) -> Vec<CompletionItem> {
    let Some(postfix) = &context.postfix_completion else {
        return Vec::new();
    };
    if context.prefix.is_empty() {
        return Vec::new();
    }

    [
        (
            "if",
            "Expand into an `if` statement using the receiver as the condition.",
            format!("if {} {{\n    $0\n}}", postfix.receiver_text),
        ),
        (
            "while",
            "Expand into a `while` loop using the receiver as the condition.",
            format!("while {} {{\n    $0\n}}", postfix.receiver_text),
        ),
        (
            "for",
            "Expand into a `for ... in ...` loop.",
            format!(
                "for ${{1:item}} in {} {{\n    $0\n}}",
                postfix.receiver_text
            ),
        ),
        (
            "switch",
            "Expand into a `switch` expression using the receiver.",
            format!(
                "switch {} {{\n    ${{1:_}} => {{\n        $0\n    }}\n}}",
                postfix.receiver_text
            ),
        ),
        (
            "return",
            "Expand into a `return` statement using the receiver.",
            format!("return {};$0", postfix.receiver_text),
        ),
        (
            "print",
            "Expand into a `print(...)` call using the receiver.",
            format!("print({})$0", postfix.receiver_text),
        ),
        (
            "debug",
            "Expand into a `debug(...)` call using the receiver.",
            format!("debug({})$0", postfix.receiver_text),
        ),
    ]
    .into_iter()
    .map(|(label, docs, new_text)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionItemKind::Keyword,
        source: CompletionItemSource::Postfix,
        sort_text: String::new(),
        detail: Some("postfix template".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(format!("{}.{}", postfix.receiver_text, label)),
        text_edit: Some(CompletionTextEdit {
            range: postfix.replace_range,
            new_text,
        }),
        insert_format: CompletionInsertFormat::Snippet,
        file_id: None,
        exported: false,
        resolve_data: None,
    })
    .collect()
}

fn doc_tag_completion_items(replace_range: TextRange) -> Vec<CompletionItem> {
    [
        ("type", "Attach a type annotation to the next declaration."),
        (
            "param",
            "Attach a parameter type annotation for the next function.",
        ),
        (
            "return",
            "Attach a return type annotation for the next function.",
        ),
        (
            "field",
            "Attach an object field type annotation in documentation.",
        ),
    ]
    .into_iter()
    .map(|(label, docs)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionItemKind::Keyword,
        source: CompletionItemSource::Builtin,
        sort_text: String::new(),
        detail: Some("doc tag".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(label.to_owned()),
        text_edit: Some(CompletionTextEdit {
            range: replace_range,
            new_text: label.to_owned(),
        }),
        insert_format: CompletionInsertFormat::PlainText,
        file_id: None,
        exported: false,
        resolve_data: None,
    })
    .collect()
}

fn doc_type_completion_items(replace_range: TextRange) -> Vec<CompletionItem> {
    [
        ("any", "The widest type."),
        ("unknown", "An unknown type."),
        ("never", "A type that should not produce a value."),
        ("Dynamic", "The Rhai dynamic value type."),
        ("bool", "Boolean values."),
        ("int", "Integer values."),
        ("float", "Floating-point values."),
        ("decimal", "Decimal values."),
        ("string", "String values."),
        ("char", "Character values."),
        ("blob", "Binary blob values."),
        ("timestamp", "Timestamp values."),
        ("Fn", "Function pointer values."),
        ("()", "The unit type."),
        ("range", "An exclusive range."),
        ("range=", "An inclusive range."),
        ("array<int>", "An array with integer items."),
        (
            "map<string, int>",
            "A map from string keys to integer values.",
        ),
        ("fun(int) -> bool", "A function type."),
    ]
    .into_iter()
    .map(|(label, docs)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionItemKind::Type,
        source: CompletionItemSource::Builtin,
        sort_text: String::new(),
        detail: Some("type".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(label.to_owned()),
        text_edit: Some(CompletionTextEdit {
            range: replace_range,
            new_text: label.to_owned(),
        }),
        insert_format: CompletionInsertFormat::PlainText,
        file_id: None,
        exported: false,
        resolve_data: None,
    })
    .collect()
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

fn rank_completion_items(items: &mut [CompletionItem], context: &CompletionContext) {
    for item in items.iter_mut() {
        item.sort_text = completion_sort_text(item, context);
    }

    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| {
                source_rank(left.source, context.member_access)
                    .cmp(&source_rank(right.source, context.member_access))
            })
    });
}

fn completion_sort_text(item: &CompletionItem, context: &CompletionContext) -> String {
    let prefix_rank = prefix_match_rank(item.label.as_str(), context.prefix.as_str());
    let source_rank = source_rank(item.source, context.member_access);
    let kind_rank = kind_rank(item.kind);

    format!(
        "{prefix_rank}:{source_rank}:{kind_rank}:{}",
        item.label.to_ascii_lowercase()
    )
}

fn prefix_match_rank(label: &str, prefix: &str) -> u8 {
    if prefix.is_empty() {
        return 1;
    }

    let label_lower = label.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();

    if label_lower == prefix_lower {
        0
    } else if label_lower.starts_with(prefix_lower.as_str()) {
        1
    } else if label_lower.contains(prefix_lower.as_str()) {
        2
    } else {
        3
    }
}

fn source_rank(source: CompletionItemSource, member_access: bool) -> u8 {
    match (member_access, source) {
        (true, CompletionItemSource::Member) => 0,
        (true, CompletionItemSource::Builtin) => 1,
        (true, CompletionItemSource::Postfix) => 2,
        (true, CompletionItemSource::Visible) => 3,
        (true, CompletionItemSource::Project) => 4,
        (false, CompletionItemSource::Visible) => 0,
        (false, CompletionItemSource::Project) => 1,
        (false, CompletionItemSource::Builtin) => 2,
        (false, CompletionItemSource::Postfix) => 3,
        (false, CompletionItemSource::Member) => 4,
    }
}

fn kind_rank(kind: CompletionItemKind) -> u8 {
    match kind {
        CompletionItemKind::Member => 0,
        CompletionItemKind::Symbol(SymbolKind::Variable | SymbolKind::Parameter) => 0,
        CompletionItemKind::Symbol(SymbolKind::Constant) => 1,
        CompletionItemKind::Symbol(SymbolKind::Function) => 2,
        CompletionItemKind::Symbol(SymbolKind::ImportAlias | SymbolKind::ExportAlias) => 3,
        CompletionItemKind::Type => 4,
        CompletionItemKind::Keyword => 5,
    }
}

fn inferred_completion_type(
    snapshot: &DatabaseSnapshot,
    file_id: rhai_vfs::FileId,
    symbol: rhai_hir::SymbolId,
) -> Option<&TypeRef> {
    snapshot
        .inferred_symbol_type(file_id, symbol)
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
}

fn workspace_completion_metadata(
    snapshot: &DatabaseSnapshot,
    symbol: &rhai_db::LocatedWorkspaceSymbol,
    detail_level: CompletionDetailLevel,
) -> (Option<String>, Option<String>, Option<TypeRef>) {
    let Some(hir) = snapshot.hir(symbol.file_id) else {
        return (None, None, None);
    };
    let Some(symbol_id) = hir.symbol_at(symbol.symbol.full_range) else {
        return (None, None, None);
    };
    let resolved_symbol = hir.symbol(symbol_id);
    let annotation = resolved_symbol
        .annotation
        .as_ref()
        .filter(|ty| !matches!(ty, TypeRef::Unknown));
    let detail = annotation.map(format_type_ref);
    let docs = match (detail_level, resolved_symbol.docs) {
        (CompletionDetailLevel::Full, Some(docs)) => Some(hir.doc_block(docs).text.clone()),
        _ => None,
    };

    (detail, docs, annotation.cloned())
}

fn builtin_function_annotation(function: &rhai_db::HostFunction) -> Option<TypeRef> {
    if function.overloads.len() != 1 {
        return None;
    }

    function
        .overloads
        .first()
        .and_then(|overload| overload.signature.clone())
        .map(TypeRef::Function)
}

fn builtin_function_detail(
    function: &rhai_db::HostFunction,
    annotation: Option<&TypeRef>,
) -> Option<String> {
    if let Some(annotation) = annotation {
        return Some(format_type_ref(annotation));
    }

    (!function.overloads.is_empty()).then(|| match function.overloads.len() {
        1 => "builtin function".to_owned(),
        count => format!("{count} overloads"),
    })
}

fn builtin_function_docs(function: &rhai_db::HostFunction) -> Option<String> {
    let mut docs = function
        .overloads
        .iter()
        .filter_map(|overload| overload.docs.as_deref())
        .map(str::trim)
        .filter(|docs| !docs.is_empty())
        .collect::<Vec<_>>();
    docs.sort_unstable();
    docs.dedup();

    (!docs.is_empty()).then(|| docs.join("\n\n"))
}
