use lsp_types::{Hover, HoverContents};
use rhai_ide::{HoverResult, HoverSignatureSource};

use crate::protocol::markup;

pub(crate) fn hover_to_lsp(hover: HoverResult) -> Hover {
    let mut lines = vec![rhai_code_block(hover.signature.as_str())];

    if let Some(docs) = hover.docs {
        lines.push(format_hover_docs(docs));
    }

    lines.push(format!(
        "### Source\n\n{}",
        hover_source_label(hover.source)
    ));

    if let Some(declared) = hover
        .declared_signature
        .as_ref()
        .filter(|declared| declared.as_str() != hover.signature)
    {
        lines.push(format!(
            "### Declared Signature\n\n{}",
            rhai_code_block(declared)
        ));
    }
    if let Some(inferred) = hover
        .inferred_signature
        .as_ref()
        .filter(|inferred| inferred.as_str() != hover.signature)
    {
        lines.push(format!(
            "### Inferred Signature\n\n{}",
            rhai_code_block(inferred)
        ));
    }
    if !hover.overload_signatures.is_empty() {
        lines.push(format!(
            "### Other Overloads\n\n{}",
            hover
                .overload_signatures
                .into_iter()
                .map(|signature| rhai_code_block(signature.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n")
        ));
    }
    if !hover.notes.is_empty() {
        lines.push(format!(
            "### Notes\n\n{}",
            hover
                .notes
                .into_iter()
                .map(|note| format!("- {note}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Hover {
        contents: HoverContents::Markup(markup(lines.join("\n\n"))),
        range: None,
    }
}

fn rhai_code_block(value: &str) -> String {
    format!("```rhai\n{value}\n```")
}

fn hover_source_label(source: HoverSignatureSource) -> &'static str {
    match source {
        HoverSignatureSource::Declared => "Declared",
        HoverSignatureSource::Inferred => "Inferred",
        HoverSignatureSource::Structural => "Structural",
    }
}

fn format_hover_docs(docs: String) -> String {
    let trimmed = docs.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.starts_with("```")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("1. ")
        || trimmed.starts_with('#')
    {
        trimmed.to_owned()
    } else {
        format!("### Documentation\n\n{trimmed}")
    }
}
