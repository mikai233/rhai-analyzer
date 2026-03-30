use rhai_vfs::FileId;

use crate::support::convert::{format_symbol_signature, text_size};
use crate::{FilePosition, HoverResult};

pub(crate) fn hover(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let (file_id, symbol_id) = if let Some(target) = snapshot
        .goto_definition(position.file_id, text_size(position.offset))
        .into_iter()
        .next()
    {
        let hir = snapshot.hir(target.file_id)?;
        (target.file_id, hir.symbol_at(target.target.full_range)?)
    } else {
        let hir = snapshot.hir(position.file_id)?;
        (
            position.file_id,
            hir.definition_at_offset(text_size(position.offset))?,
        )
    };
    hover_for_symbol(snapshot, file_id, symbol_id)
}

pub(crate) fn hover_for_symbol(
    snapshot: &rhai_db::DatabaseSnapshot,
    file_id: FileId,
    symbol_id: rhai_hir::SymbolId,
) -> Option<HoverResult> {
    let hir = snapshot.hir(file_id)?;
    let symbol = hir.symbol(symbol_id);
    let docs = symbol.docs.map(|docs| hir.doc_block(docs).text.clone());
    let annotation = snapshot
        .inferred_symbol_type(file_id, symbol_id)
        .or(symbol.annotation.as_ref());

    Some(HoverResult {
        signature: format_symbol_signature(symbol.name.as_str(), symbol.kind, annotation),
        docs,
    })
}
