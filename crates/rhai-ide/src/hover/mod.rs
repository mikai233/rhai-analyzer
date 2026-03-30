use rhai_vfs::FileId;

use rhai_hir::TypeRef;

use crate::support::convert::{format_symbol_signature, text_size};
use crate::{FilePosition, HoverResult, HoverSignatureSource};

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
    let declared_signature = symbol
        .annotation
        .as_ref()
        .filter(|annotation| !matches!(annotation, TypeRef::Unknown))
        .map(|annotation| {
            format_symbol_signature(symbol.name.as_str(), symbol.kind, Some(annotation))
        });
    let inferred_annotation = snapshot
        .inferred_symbol_type(file_id, symbol_id)
        .filter(|annotation| !matches!(annotation, TypeRef::Unknown));
    let inferred_signature = inferred_annotation.map(|annotation| {
        format_symbol_signature(symbol.name.as_str(), symbol.kind, Some(annotation))
    });

    let (signature, source) = if let Some(signature) = declared_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Declared)
    } else if let Some(signature) = inferred_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Inferred)
    } else {
        (
            format_symbol_signature(symbol.name.as_str(), symbol.kind, None),
            HoverSignatureSource::Structural,
        )
    };

    let mut notes = Vec::new();
    if declared_signature.is_none() && inferred_signature.is_some() {
        notes
            .push("Signature shown is inferred from local flow and call-site analysis.".to_owned());
    }
    if let (Some(declared), Some(inferred)) =
        (declared_signature.as_ref(), inferred_signature.as_ref())
        && declared != inferred
    {
        notes.push(format!("Inferred type: {inferred}"));
    }
    if matches!(inferred_annotation, Some(TypeRef::Ambiguous(_))) {
        notes.push("Multiple call candidates remain viable at this location.".to_owned());
    }

    Some(HoverResult {
        signature,
        docs,
        source,
        declared_signature,
        inferred_signature,
        notes,
    })
}
