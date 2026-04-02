use crate::completion::{CompletionContext, PostfixCompletionContext};
use crate::types::CompletionTextEdit;
use crate::{CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource};

pub(super) fn postfix_completion_items(context: &CompletionContext) -> Vec<CompletionItem> {
    let Some(postfix) = &context.postfix_completion else {
        return Vec::new();
    };

    postfix_templates(postfix, context.prefix.as_str(), context.replace_range)
}

fn postfix_templates(
    postfix: &PostfixCompletionContext,
    prefix: &str,
    insert_range: rhai_syntax::TextRange,
) -> Vec<CompletionItem> {
    let prefix = prefix.to_ascii_lowercase();
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
            "not",
            "Negate the receiver with the unary `!` operator.",
            format!("!{}$0", postfix.receiver_text),
        ),
        (
            "let",
            "Bind the receiver to a new local variable.",
            format!("let ${{1:value}} = {};$0", postfix.receiver_text),
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
    .filter(|(label, _, _)| prefix.is_empty() || label.starts_with(prefix.as_str()))
    .map(|(label, docs, new_text)| CompletionItem {
        label: label.to_owned(),
        kind: CompletionItemKind::Snippet,
        source: CompletionItemSource::Postfix,
        origin: None,
        sort_text: String::new(),
        detail: Some("postfix template".to_owned()),
        docs: Some(docs.to_owned()),
        filter_text: Some(label.to_owned()),
        text_edit: Some(CompletionTextEdit {
            replace_range: postfix.replace_range,
            insert_range: Some(insert_range),
            new_text,
        }),
        insert_format: CompletionInsertFormat::Snippet,
        file_id: None,
        exported: false,
        resolve_data: None,
    })
    .collect()
}
