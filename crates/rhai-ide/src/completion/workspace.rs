use rhai_db::{DatabaseSnapshot, HostFunction, LocatedWorkspaceSymbol};
use rhai_hir::{SymbolId, TypeRef};
use rhai_vfs::FileId;

use crate::completion::CompletionDetailLevel;
use crate::support::convert::format_type_ref;

pub(super) fn inferred_completion_type(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    symbol: SymbolId,
) -> Option<&TypeRef> {
    snapshot
        .inferred_symbol_type(file_id, symbol)
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
}

pub(super) fn workspace_completion_metadata(
    snapshot: &DatabaseSnapshot,
    symbol: &LocatedWorkspaceSymbol,
    detail_level: CompletionDetailLevel,
) -> (
    Option<String>,
    Option<String>,
    Option<TypeRef>,
    Option<String>,
) {
    let Some(hir) = snapshot.hir(symbol.file_id) else {
        return (None, None, None, None);
    };
    let Some(symbol_id) = hir.symbol_at(symbol.symbol.full_range) else {
        return (None, None, None, None);
    };
    let resolved_symbol = hir.symbol(symbol_id);
    let annotation = resolved_symbol
        .annotation
        .as_ref()
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
        .cloned()
        .or_else(|| {
            snapshot
                .inferred_symbol_type(symbol.file_id, symbol_id)
                .cloned()
                .filter(|ty| !matches!(ty, TypeRef::Unknown))
        });
    let detail = annotation.as_ref().map(format_type_ref);
    let docs = match (detail_level, resolved_symbol.docs) {
        (CompletionDetailLevel::Full, Some(docs)) => Some(hir.doc_block(docs).text.clone()),
        _ => None,
    };

    let origin = snapshot
        .normalized_path(symbol.file_id)
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .map(ToOwned::to_owned);

    (detail, docs, annotation, origin)
}

pub(super) fn builtin_function_annotation(function: &HostFunction) -> Option<TypeRef> {
    if function.overloads.len() != 1 {
        return None;
    }

    function
        .overloads
        .first()
        .and_then(|overload| overload.signature.clone())
        .map(TypeRef::Function)
}

pub(super) fn builtin_function_detail(
    function: &HostFunction,
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

pub(super) fn builtin_function_docs(function: &HostFunction) -> Option<String> {
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
