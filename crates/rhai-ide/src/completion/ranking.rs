use rhai_hir::SymbolKind;

use crate::completion::CompletionContext;
use crate::{CompletionItem, CompletionItemKind, CompletionItemSource};

pub(super) fn rank_completion_items(items: &mut [CompletionItem], context: &CompletionContext) {
    for item in items.iter_mut() {
        item.sort_text = completion_sort_text(item, context);
    }

    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| {
                source_rank(left.source, context).cmp(&source_rank(right.source, context))
            })
    });
}

fn completion_sort_text(item: &CompletionItem, context: &CompletionContext) -> String {
    let relevance_rank = relevance_rank(item, context);
    let prefix_rank = prefix_match_rank(item.label.as_str(), context.prefix.as_str());
    let source_rank = source_rank(item.source, context);
    let kind_rank = kind_rank(item.kind);

    format!(
        "{relevance_rank}:{prefix_rank}:{source_rank}:{kind_rank}:{}",
        item.label.to_ascii_lowercase()
    )
}

fn relevance_rank(item: &CompletionItem, context: &CompletionContext) -> u8 {
    if item.source != CompletionItemSource::Postfix {
        return 1;
    }

    if context.prefix.is_empty() {
        return 1;
    }

    if item.label.eq_ignore_ascii_case(context.prefix.as_str()) {
        0
    } else {
        2
    }
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

pub(super) fn source_rank(source: CompletionItemSource, context: &CompletionContext) -> u8 {
    match (context.member_access, source) {
        (true, CompletionItemSource::Member) => 0,
        (true, CompletionItemSource::Builtin) => 1,
        (true, CompletionItemSource::Postfix) => 2,
        (true, CompletionItemSource::Visible) => 3,
        (true, CompletionItemSource::Module) => 4,
        (true, CompletionItemSource::Project) => 5,
        (false, CompletionItemSource::Visible) => 0,
        (false, CompletionItemSource::Module) => 1,
        (false, CompletionItemSource::Project) => 2,
        (false, CompletionItemSource::Builtin) => 3,
        (false, CompletionItemSource::Postfix) => 4,
        (false, CompletionItemSource::Member) => 5,
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
        CompletionItemKind::Snippet => 5,
        CompletionItemKind::Keyword => 6,
    }
}
