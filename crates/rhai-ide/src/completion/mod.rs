use rhai_db::DatabaseSnapshot;
use rhai_hir::{CompletionSymbol, TypeRef};

use crate::support::convert::{format_type_ref, text_size};
use crate::{CompletionItem, CompletionItemKind, CompletionItemSource, FilePosition};

pub(crate) fn completions(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> Vec<CompletionItem> {
    let Some(inputs) = snapshot.completion_inputs(position.file_id, text_size(position.offset))
    else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let hir = snapshot.hir(position.file_id);

    items.extend(inputs.visible_symbols.iter().map(|symbol| {
        let docs = match (&hir, symbol.docs) {
            (Some(hir), Some(docs)) => Some(hir.doc_block(docs).text.clone()),
            _ => None,
        };

        CompletionItem {
            label: symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.kind),
            source: CompletionItemSource::Visible,
            detail: completion_detail(snapshot, position.file_id, symbol),
            docs,
            file_id: Some(position.file_id),
            exported: false,
        }
    }));

    items.extend(inputs.project_symbols.iter().map(|symbol| CompletionItem {
        label: symbol.symbol.name.clone(),
        kind: CompletionItemKind::Symbol(symbol.symbol.kind),
        source: CompletionItemSource::Project,
        detail: None,
        docs: None,
        file_id: Some(symbol.file_id),
        exported: symbol.symbol.exported,
    }));

    items.extend(inputs.member_symbols.iter().map(|member| CompletionItem {
        label: member.name.clone(),
        kind: CompletionItemKind::Member,
        source: CompletionItemSource::Member,
        detail: member.annotation.as_ref().map(format_type_ref),
        docs: None,
        file_id: None,
        exported: false,
    }));

    items
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
