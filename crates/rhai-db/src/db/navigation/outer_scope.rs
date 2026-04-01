use rhai_hir::{FileHir, ReferenceId, ScopeId, SymbolId, SymbolKind};
use rhai_syntax::TextSize;

pub(crate) fn resolve_unresolved_name_in_outer_scope(
    hir: &FileHir,
    reference_id: ReferenceId,
) -> Option<SymbolId> {
    let reference = hir.reference(reference_id);
    if reference.kind != rhai_hir::ReferenceKind::Name || reference.target.is_some() {
        return None;
    }

    let function_scope = enclosing_function_scope(hir, reference.scope)?;
    let capture = resolve_name_in_outer_scopes(
        hir,
        hir.scope(function_scope).parent?,
        reference.name.as_str(),
        reference.range.start(),
    )?;

    if matches!(
        hir.symbol(capture).kind,
        SymbolKind::Function | SymbolKind::ImportAlias | SymbolKind::ExportAlias
    ) {
        return None;
    }

    Some(capture)
}

pub(crate) fn unresolved_outer_scope_references_to_symbol(
    hir: &FileHir,
    target_symbol: SymbolId,
) -> Vec<ReferenceId> {
    let target = hir.symbol(target_symbol);
    hir.references
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            if reference.kind != rhai_hir::ReferenceKind::Name
                || reference.target.is_some()
                || reference.name != target.name
            {
                return None;
            }
            let reference_id = ReferenceId(index as u32);
            (resolve_unresolved_name_in_outer_scope(hir, reference_id) == Some(target_symbol))
                .then_some(reference_id)
        })
        .collect()
}

fn enclosing_function_scope(hir: &FileHir, mut scope: ScopeId) -> Option<ScopeId> {
    loop {
        let scope_data = hir.scope(scope);
        if scope_data.kind == rhai_hir::ScopeKind::Function {
            return Some(scope);
        }
        scope = scope_data.parent?;
    }
}

fn resolve_name_in_outer_scopes(
    hir: &FileHir,
    mut scope: ScopeId,
    name: &str,
    reference_start: TextSize,
) -> Option<SymbolId> {
    loop {
        if let Some(symbol) = hir
            .scope(scope)
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = hir.symbol(*symbol_id);
                symbol.name == name && symbol.range.start() <= reference_start
            })
        {
            return Some(symbol);
        }
        scope = hir.scope(scope).parent?;
    }
}
