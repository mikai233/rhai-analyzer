use rhai_db::DatabaseSnapshot;
use rhai_hir::{CompletionSymbol, SymbolKind, TypeRef};

use crate::support::convert::{format_type_ref, text_size};
use crate::{
    CompletionItem, CompletionItemKind, CompletionItemSource, CompletionResolveData, FilePosition,
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
    member_access: bool,
}

fn completion_items(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    detail_level: CompletionDetailLevel,
) -> Vec<CompletionItem> {
    let Some(inputs) = snapshot.completion_inputs(position.file_id, text_size(position.offset))
    else {
        return Vec::new();
    };

    let context = completion_context(snapshot, position);
    let mut items = Vec::new();
    let hir = snapshot.hir(position.file_id);

    items.extend(inputs.visible_symbols.iter().map(|symbol| {
        let docs = match (detail_level, &hir, symbol.docs) {
            (CompletionDetailLevel::Full, Some(hir), Some(docs)) => {
                Some(hir.doc_block(docs).text.clone())
            }
            _ => None,
        };

        CompletionItem {
            label: symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.kind),
            source: CompletionItemSource::Visible,
            sort_text: String::new(),
            detail: completion_detail(snapshot, position.file_id, symbol),
            docs,
            file_id: Some(position.file_id),
            exported: false,
            resolve_data: Some(CompletionResolveData {
                file_id: position.file_id,
                offset: position.offset,
            }),
        }
    }));

    items.extend(inputs.project_symbols.iter().map(|symbol| {
        let (detail, docs) = workspace_completion_metadata(snapshot, symbol, detail_level);

        CompletionItem {
            label: symbol.symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.symbol.kind),
            source: CompletionItemSource::Project,
            sort_text: String::new(),
            detail,
            docs,
            file_id: Some(symbol.file_id),
            exported: symbol.symbol.exported,
            resolve_data: Some(CompletionResolveData {
                file_id: position.file_id,
                offset: position.offset,
            }),
        }
    }));

    items.extend(inputs.member_symbols.iter().map(|member| CompletionItem {
        label: member.name.clone(),
        kind: CompletionItemKind::Member,
        source: CompletionItemSource::Member,
        sort_text: String::new(),
        detail: member.annotation.as_ref().map(format_type_ref),
        docs: None,
        file_id: None,
        exported: false,
        resolve_data: Some(CompletionResolveData {
            file_id: position.file_id,
            offset: position.offset,
        }),
    }));

    rank_completion_items(&mut items, &context);
    items
}

fn same_completion_item(left: &CompletionItem, right: &CompletionItem) -> bool {
    left.label == right.label
        && left.kind == right.kind
        && left.source == right.source
        && left.file_id == right.file_id
        && left.exported == right.exported
}

fn completion_context(snapshot: &DatabaseSnapshot, position: FilePosition) -> CompletionContext {
    let Some(text) = snapshot.file_text(position.file_id) else {
        return CompletionContext {
            prefix: String::new(),
            member_access: false,
        };
    };
    let offset = usize::try_from(position.offset)
        .unwrap_or(usize::MAX)
        .min(text.len());
    let bytes = text.as_bytes();
    let mut start = offset;

    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let prefix = text[start..offset].to_owned();
    let member_access = start > 0 && bytes[start - 1] == b'.';

    CompletionContext {
        prefix,
        member_access,
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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
        (true, CompletionItemSource::Visible) => 1,
        (true, CompletionItemSource::Project) => 2,
        (false, CompletionItemSource::Visible) => 0,
        (false, CompletionItemSource::Project) => 1,
        (false, CompletionItemSource::Member) => 2,
    }
}

fn kind_rank(kind: CompletionItemKind) -> u8 {
    match kind {
        CompletionItemKind::Member => 0,
        CompletionItemKind::Symbol(SymbolKind::Variable | SymbolKind::Parameter) => 0,
        CompletionItemKind::Symbol(SymbolKind::Constant) => 1,
        CompletionItemKind::Symbol(SymbolKind::Function) => 2,
        CompletionItemKind::Symbol(SymbolKind::ImportAlias | SymbolKind::ExportAlias) => 3,
    }
}

fn completion_detail(
    snapshot: &DatabaseSnapshot,
    file_id: rhai_vfs::FileId,
    symbol: &CompletionSymbol,
) -> Option<String> {
    symbol
        .annotation
        .as_ref()
        .or_else(|| inferred_completion_type(snapshot, file_id, symbol.symbol))
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
        .map(format_type_ref)
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
) -> (Option<String>, Option<String>) {
    let Some(hir) = snapshot.hir(symbol.file_id) else {
        return (None, None);
    };
    let Some(symbol_id) = hir.symbol_at(symbol.symbol.full_range) else {
        return (None, None);
    };
    let resolved_symbol = hir.symbol(symbol_id);
    let detail = resolved_symbol
        .annotation
        .as_ref()
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
        .map(format_type_ref);
    let docs = match (detail_level, resolved_symbol.docs) {
        (CompletionDetailLevel::Full, Some(docs)) => Some(hir.doc_block(docs).text.clone()),
        _ => None,
    };

    (detail, docs)
}
