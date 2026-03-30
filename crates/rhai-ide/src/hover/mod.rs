use rhai_vfs::FileId;

use rhai_hir::{SymbolKind, TypeRef};

use crate::support::convert::{format_symbol_signature, text_size};
use crate::{FilePosition, HoverResult, HoverSignatureSource};

pub(crate) fn hover(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let symbol_hover = if let Some(target) = snapshot
        .goto_definition(position.file_id, text_size(position.offset))
        .into_iter()
        .next()
    {
        let hir = snapshot.hir(target.file_id)?;
        Some((target.file_id, hir.symbol_at(target.target.full_range)?))
    } else {
        let hir = snapshot.hir(position.file_id)?;
        hir.definition_at_offset(text_size(position.offset))
            .map(|symbol_id| (position.file_id, symbol_id))
    };

    symbol_hover
        .and_then(|(file_id, symbol_id)| hover_for_symbol(snapshot, file_id, symbol_id))
        .or_else(|| hover_for_builtin(snapshot, position))
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

fn hover_for_builtin(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let reference_id = hir.reference_at_offset(text_size(position.offset))?;
    let reference = hir.reference(reference_id);
    if reference.target.is_some() || !matches!(reference.kind, rhai_hir::ReferenceKind::Name) {
        return None;
    }

    let function = snapshot.global_function(reference.name.as_str())?;
    let declared_signature = builtin_function_signature(function);
    let signature = declared_signature.clone().unwrap_or_else(|| {
        format_symbol_signature(function.name.as_str(), SymbolKind::Function, None)
    });
    let mut notes = Vec::new();
    if function.overloads.len() > 1 {
        notes.push(format!(
            "{} overloads are available for this builtin function.",
            function.overloads.len()
        ));
    }

    Some(HoverResult {
        signature,
        docs: builtin_function_docs(function),
        source: HoverSignatureSource::Declared,
        declared_signature,
        inferred_signature: None,
        notes,
    })
}

fn builtin_function_signature(function: &rhai_db::HostFunction) -> Option<String> {
    if function.overloads.len() != 1 {
        return None;
    }

    function
        .overloads
        .first()
        .and_then(|overload| overload.signature.as_ref())
        .map(|signature| {
            format_symbol_signature(
                function.name.as_str(),
                SymbolKind::Function,
                Some(&TypeRef::Function(signature.clone())),
            )
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
