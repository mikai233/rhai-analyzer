use crate::db::DatabaseSnapshot;
use crate::db::diagnostics::{reference_id_for_diagnostic, unresolved_name_is_known_external};
use crate::db::imports::linked_import_targets_for_path_reference;
use crate::types::{
    ProjectDiagnostic, ProjectDiagnosticCode, ProjectDiagnosticKind, ProjectDiagnosticSeverity,
};
use rhai_hir::{
    FileHir, ReferenceId, ScopeId, SemanticDiagnostic, SemanticDiagnosticKind, SymbolId, SymbolKind,
};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct CallerScopeUnresolvedContext {
    pub(crate) regular_call_sites: Vec<(FileId, TextRange)>,
}

#[derive(Debug, Clone)]
pub(crate) struct CallerScopeCaptureContext {
    function_symbol: SymbolId,
    function_name: String,
    function_range: TextRange,
}

pub(crate) fn unresolved_name_requires_caller_scope(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
    caller_scope_regular_calls: &mut HashMap<SymbolId, Vec<(FileId, TextRange)>>,
) -> Option<CallerScopeUnresolvedContext> {
    let capture = unresolved_name_caller_scope_capture(snapshot, file_id, hir, diagnostic)?;

    let regular_call_sites = caller_scope_regular_calls
        .entry(capture.function_symbol)
        .or_insert_with(|| collect_regular_call_sites(snapshot, file_id, capture.function_symbol))
        .clone();

    Some(CallerScopeUnresolvedContext { regular_call_sites })
}

pub(crate) fn unresolved_name_caller_scope_capture(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> Option<CallerScopeCaptureContext> {
    if diagnostic.kind != SemanticDiagnosticKind::UnresolvedName
        || unresolved_name_is_known_external(snapshot, file_id, hir, diagnostic)
    {
        return None;
    }

    let reference_id = reference_id_for_diagnostic(hir, diagnostic)?;
    let reference = hir.reference(reference_id);
    if reference.kind != rhai_hir::ReferenceKind::Name || reference.target.is_some() {
        return None;
    }

    let function_symbol = hir.enclosing_function_symbol_at(reference.range.start())?;
    if hir.symbol(function_symbol).kind != SymbolKind::Function {
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

    Some(CallerScopeCaptureContext {
        function_symbol,
        function_name: hir.symbol(function_symbol).name.clone(),
        function_range: hir.symbol(function_symbol).range,
    })
}

pub(crate) fn collect_regular_call_sites(
    snapshot: &DatabaseSnapshot,
    target_file_id: FileId,
    function_symbol: SymbolId,
) -> Vec<(FileId, TextRange)> {
    let mut call_sites = snapshot
        .analysis
        .iter()
        .flat_map(|(&file_id, analysis)| {
            analysis
                .hir
                .calls
                .iter()
                .filter(move |call| {
                    call_targets_function(
                        snapshot,
                        target_file_id,
                        function_symbol,
                        file_id,
                        analysis.hir.as_ref(),
                        call,
                    )
                })
                .filter(|call| !call.caller_scope)
                .map(move |call| (file_id, call.callee_range.unwrap_or(call.range)))
        })
        .collect::<Vec<_>>();

    call_sites.sort_by(|left, right| {
        left.0
            .0
            .cmp(&right.0.0)
            .then_with(|| left.1.start().cmp(&right.1.start()))
            .then_with(|| left.1.end().cmp(&right.1.end()))
    });
    call_sites.dedup();
    call_sites
}

pub(crate) fn call_targets_function(
    snapshot: &DatabaseSnapshot,
    target_file_id: FileId,
    target_symbol: SymbolId,
    file_id: FileId,
    hir: &FileHir,
    call: &rhai_hir::CallSite,
) -> bool {
    if file_id == target_file_id && call.resolved_callee == Some(target_symbol) {
        return true;
    }

    if call_linked_import_targets(snapshot, file_id, hir, call)
        .iter()
        .any(|target| {
            target.file_id == target_file_id
                && target.symbol.kind == SymbolKind::Function
                && target.symbol.symbol == target_symbol
        })
    {
        return true;
    }

    call.callee_range
        .map(|range| {
            snapshot
                .project_targets_at(file_id, range.start())
                .iter()
                .any(|target| {
                    target.file_id == target_file_id
                        && target.symbol.kind == SymbolKind::Function
                        && target.symbol.symbol == target_symbol
                })
        })
        .unwrap_or(false)
}

pub(crate) fn caller_scope_regular_call_diagnostics(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<ProjectDiagnostic> {
    let mut projected = Vec::new();
    let mut requirement_cache =
        HashMap::<(FileId, SymbolId), Option<CallerScopeCaptureContext>>::new();

    for call in hir.calls.iter().filter(|call| !call.caller_scope) {
        let call_range = call.callee_range.unwrap_or(call.range);
        if call_range.len() == TextSize::from(0_u32) {
            continue;
        }

        for target in call_function_targets(snapshot, file_id, hir, call) {
            let key = (target.file_id, target.symbol.symbol);
            let requirement = requirement_cache
                .entry(key)
                .or_insert_with(|| {
                    caller_scope_requirement_for_function(
                        snapshot,
                        target.file_id,
                        target.symbol.symbol,
                    )
                })
                .clone();
            let Some(requirement) = requirement else {
                continue;
            };

            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                code: ProjectDiagnosticCode::CallerScopeRequired,
                severity: ProjectDiagnosticSeverity::Error,
                range: call_range,
                message: format!(
                    "call to `{}` must use caller scope (`call!`) because the function references outer-scope names",
                    requirement.function_name
                ),
                related_range: Some(requirement.function_range),
                tags: Arc::from([]),
            });
        }
    }

    projected
}

pub(crate) fn call_function_targets(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    call: &rhai_hir::CallSite,
) -> Vec<crate::types::LocatedSymbolIdentity> {
    let mut targets = Vec::new();

    if let Some(callee) = call.resolved_callee
        && hir.symbol(callee).kind == SymbolKind::Function
    {
        targets.extend(
            snapshot
                .locate_symbol(&hir.file_backed_symbol_identity(callee))
                .iter()
                .cloned(),
        );
    }

    targets.extend(call_linked_import_targets(snapshot, file_id, hir, call));

    if let Some(range) = call.callee_range {
        targets.extend(snapshot.project_targets_at(file_id, range.start()));
    }

    targets.retain(|target| target.symbol.kind == SymbolKind::Function);
    targets.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    targets.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    targets
}

pub(crate) fn call_linked_import_targets(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    call: &rhai_hir::CallSite,
) -> Vec<crate::types::LocatedSymbolIdentity> {
    let mut targets = Vec::new();

    if let Some(callee_range) = call.callee_range {
        targets.extend(
            hir.references
                .iter()
                .enumerate()
                .filter_map(|(index, reference)| {
                    callee_range
                        .contains_range(reference.range)
                        .then_some(ReferenceId(index as u32))
                })
                .flat_map(|reference_id| {
                    linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                }),
        );
    }

    if targets.is_empty()
        && let Some(reference_id) = call.callee_reference
    {
        targets.extend(linked_import_targets_for_path_reference(
            snapshot,
            file_id,
            hir,
            reference_id,
        ));
    }

    targets.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    targets.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    targets
}

pub(crate) fn caller_scope_requirement_for_function(
    snapshot: &DatabaseSnapshot,
    target_file_id: FileId,
    target_symbol: SymbolId,
) -> Option<CallerScopeCaptureContext> {
    let analysis = snapshot.analysis.get(&target_file_id)?;

    analysis
        .semantic_diagnostics
        .iter()
        .filter_map(|diagnostic| {
            unresolved_name_caller_scope_capture(
                snapshot,
                target_file_id,
                analysis.hir.as_ref(),
                diagnostic,
            )
        })
        .find(|capture| capture.function_symbol == target_symbol)
}

pub(crate) fn enclosing_function_scope(hir: &FileHir, mut scope: ScopeId) -> Option<ScopeId> {
    loop {
        let scope_data = hir.scope(scope);
        if scope_data.kind == rhai_hir::ScopeKind::Function {
            return Some(scope);
        }
        scope = scope_data.parent?;
    }
}

pub(crate) fn resolve_name_in_outer_scopes(
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
