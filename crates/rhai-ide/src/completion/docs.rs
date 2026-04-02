use rhai_syntax::TextRange;

use crate::completion::DocCompletionContext;
use crate::types::CompletionTextEdit;
use crate::{CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource};

pub(super) fn doc_completion_items(context: &DocCompletionContext) -> Vec<CompletionItem> {
    match context {
        DocCompletionContext::Tag { replace_range, .. } => doc_tag_completion_items(*replace_range),
        DocCompletionContext::Type { replace_range, .. } => {
            doc_type_completion_items(*replace_range)
        }
    }
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
        origin: None,
        sort_text: String::new(),
        detail: Some("doc tag".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(label.to_owned()),
        text_edit: Some(CompletionTextEdit {
            replace_range,
            insert_range: None,
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
        origin: None,
        sort_text: String::new(),
        detail: Some("type".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(label.to_owned()),
        text_edit: Some(CompletionTextEdit {
            replace_range,
            insert_range: None,
            new_text: label.to_owned(),
        }),
        insert_format: CompletionInsertFormat::PlainText,
        file_id: None,
        exported: false,
        resolve_data: None,
    })
    .collect()
}
