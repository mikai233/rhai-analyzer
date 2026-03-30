use rhai_db::DatabaseSnapshot;
use rhai_hir::{BodyId, BodyKind, FileHir, FunctionTypeRef, SymbolId, SymbolKind, TypeRef};
use rhai_vfs::FileId;

use crate::support::convert::format_type_ref;
use crate::{InlayHint, InlayHintKind, InlayHintSource};

pub(crate) fn inlay_hints(snapshot: &DatabaseSnapshot, file_id: FileId) -> Vec<InlayHint> {
    let Some(hir) = snapshot.hir(file_id) else {
        return Vec::new();
    };
    let mut hints = Vec::new();

    hints.extend(variable_type_hints(snapshot, file_id, hir.as_ref()));
    hints.extend(parameter_type_hints(snapshot, file_id, hir.as_ref()));
    hints.extend(function_return_type_hints(snapshot, file_id, hir.as_ref()));
    hints.extend(closure_return_type_hints(snapshot, file_id, hir.as_ref()));

    hints.sort_by_key(|hint| (hint.offset, hint.label.clone()));
    hints.dedup();
    hints
}

fn variable_type_hints(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<InlayHint> {
    hir.symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Variable).then_some((SymbolId(index as u32), symbol))
        })
        .filter_map(|(symbol_id, symbol)| {
            let ty = snapshot.inferred_symbol_type(file_id, symbol_id)?;
            is_useful_type_hint(ty).then(|| InlayHint {
                offset: u32::from(symbol.range.end()),
                label: format!(": {}", format_type_ref(ty)),
                kind: InlayHintKind::Type,
                source: InlayHintSource::Variable,
            })
        })
        .collect()
}

fn parameter_type_hints(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<InlayHint> {
    let mut parameter_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Function).then_some(SymbolId(index as u32))
        })
        .filter_map(|symbol_id| {
            hir.body_of(symbol_id)
                .map(|body_id| hir.body(body_id).scope)
        })
        .flat_map(|scope| hir.scope(scope).symbols.iter().copied())
        .filter(|symbol_id| hir.symbol(*symbol_id).kind == SymbolKind::Parameter)
        .collect::<Vec<_>>();

    parameter_symbols.extend(hir.closure_exprs.iter().flat_map(|closure| {
        let scope = hir.body(closure.body).scope;
        hir.scope(scope)
            .symbols
            .iter()
            .copied()
            .filter(|symbol_id| hir.symbol(*symbol_id).kind == SymbolKind::Parameter)
            .collect::<Vec<_>>()
    }));

    parameter_symbols.sort_unstable_by_key(|symbol_id| symbol_id.0);
    parameter_symbols.dedup();

    parameter_symbols
        .into_iter()
        .filter_map(|symbol_id| {
            let symbol = hir.symbol(symbol_id);
            let ty = snapshot.inferred_symbol_type(file_id, symbol_id)?;
            is_useful_type_hint(ty).then(|| InlayHint {
                offset: u32::from(symbol.range.end()),
                label: format!(": {}", format_type_ref(ty)),
                kind: InlayHintKind::Type,
                source: InlayHintSource::Parameter,
            })
        })
        .collect()
}

fn function_return_type_hints(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<InlayHint> {
    hir.symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Function).then_some((SymbolId(index as u32), symbol))
        })
        .filter_map(|(symbol_id, _symbol)| {
            let body = hir.body_of(symbol_id).map(|body_id| hir.body(body_id))?;
            if body.kind != BodyKind::Function {
                return None;
            }
            let inferred = snapshot.inferred_symbol_type(file_id, symbol_id)?;
            let signature = function_signature(inferred)?;
            is_useful_type_hint(signature.ret.as_ref()).then(|| InlayHint {
                offset: u32::from(body.range.start()),
                label: format!(" -> {}", format_type_ref(signature.ret.as_ref())),
                kind: InlayHintKind::Type,
                source: InlayHintSource::ReturnType,
            })
        })
        .collect()
}

fn closure_return_type_hints(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<InlayHint> {
    hir.closure_exprs
        .iter()
        .filter_map(|closure| {
            let ty = inferred_body_return_type(snapshot, file_id, hir, closure.body)?;
            is_useful_type_hint(&ty).then(|| InlayHint {
                offset: u32::from(hir.body(closure.body).range.start()),
                label: format!(" -> {}", format_type_ref(&ty)),
                kind: InlayHintKind::Type,
                source: InlayHintSource::ReturnType,
            })
        })
        .collect()
}

fn function_signature(ty: &TypeRef) -> Option<&FunctionTypeRef> {
    match ty {
        TypeRef::Function(signature) => Some(signature),
        _ => None,
    }
}

fn is_useful_type_hint(ty: &TypeRef) -> bool {
    !matches!(
        ty,
        TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never
    )
}

fn inferred_body_return_type(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    body_id: BodyId,
) -> Option<TypeRef> {
    let inference = snapshot.type_inference(file_id)?;
    let mut types = hir
        .body_return_values(body_id)
        .filter_map(|expr| hir.expr_type(expr, &inference.expr_types).cloned())
        .collect::<Vec<_>>();

    if hir.body_may_fall_through(body_id)
        && let Some(tail) = hir.body_tail_value(body_id)
        && let Some(ty) = hir.expr_type(tail, &inference.expr_types).cloned()
    {
        types.push(ty);
    }

    merge_hint_types(types)
}

fn merge_hint_types(types: Vec<TypeRef>) -> Option<TypeRef> {
    let mut unique = Vec::<TypeRef>::new();
    for ty in types {
        if !is_useful_type_hint(&ty) || unique.iter().any(|existing| existing == &ty) {
            continue;
        }
        unique.push(ty);
    }

    match unique.len() {
        0 => None,
        1 => unique.into_iter().next(),
        _ => Some(TypeRef::Union(unique)),
    }
}
