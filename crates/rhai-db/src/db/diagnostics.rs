use crate::db::DatabaseSnapshot;
use crate::db::imports::linked_import_targets_for_path_reference;
use crate::types::{
    ProjectDiagnostic, ProjectDiagnosticCode, ProjectDiagnosticKind, ProjectDiagnosticSeverity,
    ProjectDiagnosticTag,
};
use rhai_hir::{
    FileHir, ReferenceId, ScopeId, SemanticDiagnostic, SemanticDiagnosticCode,
    SemanticDiagnosticKind, SymbolId, SymbolKind,
};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

impl DatabaseSnapshot {
    pub fn project_diagnostics(&self, file_id: FileId) -> Vec<ProjectDiagnostic> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        let mut diagnostics = self
            .syntax_diagnostics(file_id)
            .iter()
            .map(|diagnostic| ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Syntax,
                code: ProjectDiagnosticCode::Syntax(diagnostic.code().clone()),
                severity: ProjectDiagnosticSeverity::Error,
                range: diagnostic.range(),
                message: diagnostic.message().to_owned(),
                related_range: None,
                tags: Arc::from([]),
            })
            .collect::<Vec<_>>();

        diagnostics.extend(project_semantic_diagnostics(
            self,
            file_id,
            analysis.hir.as_ref(),
            analysis.semantic_diagnostics.as_ref(),
        ));

        diagnostics.sort_by(|left, right| {
            left.range
                .start()
                .cmp(&right.range.start())
                .then_with(|| {
                    project_diagnostic_kind_rank(left.kind)
                        .cmp(&project_diagnostic_kind_rank(right.kind))
                })
                .then_with(|| left.message.cmp(&right.message))
        });
        diagnostics.dedup_by(|left, right| {
            left.kind == right.kind
                && left.code == right.code
                && left.range == right.range
                && left.related_range == right.related_range
                && left.message == right.message
        });
        diagnostics
    }
}

fn project_diagnostic_kind_rank(kind: ProjectDiagnosticKind) -> u8 {
    match kind {
        ProjectDiagnosticKind::Syntax => 0,
        ProjectDiagnosticKind::Semantic => 1,
        ProjectDiagnosticKind::BrokenLinkedImport => 2,
        ProjectDiagnosticKind::AmbiguousLinkedImport => 3,
    }
}

fn project_semantic_diagnostics(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    diagnostics: &[SemanticDiagnostic],
) -> Vec<ProjectDiagnostic> {
    let mut projected = Vec::new();
    let mut caller_scope_regular_calls = HashMap::<SymbolId, Vec<(FileId, TextRange)>>::new();

    for diagnostic in diagnostics {
        if diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && unresolved_name_is_known_external(snapshot, file_id, hir, diagnostic)
        {
            continue;
        }

        if diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && let Some(context) = unresolved_name_requires_caller_scope(
                snapshot,
                file_id,
                hir,
                diagnostic,
                &mut caller_scope_regular_calls,
            )
        {
            if context.regular_call_sites.is_empty() {
                continue;
            }

            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                code: ProjectDiagnosticCode::Semantic(diagnostic.code.clone()),
                severity: semantic_diagnostic_severity(diagnostic.kind),
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
                tags: semantic_diagnostic_tags(diagnostic.kind),
            });
            continue;
        }

        if diagnostic.kind != SemanticDiagnosticKind::UnresolvedImport {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                code: ProjectDiagnosticCode::Semantic(diagnostic.code.clone()),
                severity: semantic_diagnostic_severity(diagnostic.kind),
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
                tags: semantic_diagnostic_tags(diagnostic.kind),
            });
            continue;
        }

        let Some(import_index) = import_index_for_diagnostic(hir, diagnostic) else {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                code: ProjectDiagnosticCode::Semantic(diagnostic.code.clone()),
                severity: semantic_diagnostic_severity(diagnostic.kind),
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
                tags: semantic_diagnostic_tags(diagnostic.kind),
            });
            continue;
        };

        match snapshot.linked_import(file_id, import_index) {
            Some(linked_import) if linked_import.exports.len() == 1 => {}
            Some(linked_import) => {
                projected.push(ProjectDiagnostic {
                    kind: ProjectDiagnosticKind::AmbiguousLinkedImport,
                    code: ProjectDiagnosticCode::AmbiguousLinkedImport,
                    severity: ProjectDiagnosticSeverity::Error,
                    range: diagnostic.range,
                    message: format!(
                        "ambiguous import module `{}` matches multiple workspace exports",
                        linked_import.module_name
                    ),
                    related_range: Some(hir.import(import_index).range),
                    tags: Arc::from([]),
                });
                projected.extend(linked_import_usage_diagnostics(
                    hir,
                    import_index,
                    ProjectDiagnosticKind::AmbiguousLinkedImport,
                    format!(
                        "import alias cannot be resolved uniquely because module `{}` matches multiple workspace exports",
                        linked_import.module_name
                    ),
                ));
            }
            None => {
                projected.push(ProjectDiagnostic {
                    kind: ProjectDiagnosticKind::Semantic,
                    code: ProjectDiagnosticCode::Semantic(diagnostic.code.clone()),
                    severity: semantic_diagnostic_severity(diagnostic.kind),
                    range: diagnostic.range,
                    message: diagnostic.message.clone(),
                    related_range: diagnostic.related_range,
                    tags: semantic_diagnostic_tags(diagnostic.kind),
                });
                projected.extend(linked_import_usage_diagnostics(
                    hir,
                    import_index,
                    ProjectDiagnosticKind::BrokenLinkedImport,
                    format!(
                        "import alias no longer resolves because module `{}` is unavailable in the workspace",
                        hir.reference(
                            hir.import(import_index)
                                .module_reference
                                .expect("expected import reference")
                        )
                        .name
                    ),
                ));
            }
        }
    }

    projected.extend(static_import_missing_module_diagnostics(
        snapshot, file_id, hir,
    ));
    projected.extend(unresolved_import_member_path_diagnostics(
        snapshot, file_id, hir,
    ));
    projected.extend(caller_scope_regular_call_diagnostics(
        snapshot, file_id, hir,
    ));
    projected
}

fn unresolved_name_is_known_external(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> bool {
    let Some(reference_id) = reference_id_for_diagnostic(hir, diagnostic) else {
        return false;
    };
    let name = hir.reference(reference_id).name.as_str();
    snapshot.global_function(name).is_some()
        || snapshot
            .effective_external_signatures(file_id)
            .get(name)
            .is_some()
        || snapshot
            .comment_directives(file_id)
            .is_some_and(|directives| directives.allowed_unresolved_names.contains(name))
}

#[derive(Debug, Clone)]
struct CallerScopeUnresolvedContext {
    regular_call_sites: Vec<(FileId, TextRange)>,
}

#[derive(Debug, Clone)]
struct CallerScopeCaptureContext {
    function_symbol: SymbolId,
    function_name: String,
    function_range: TextRange,
}

fn unresolved_name_requires_caller_scope(
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

fn unresolved_name_caller_scope_capture(
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

fn collect_regular_call_sites(
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

fn call_targets_function(
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

fn caller_scope_regular_call_diagnostics(
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

fn call_function_targets(
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

fn call_linked_import_targets(
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

fn caller_scope_requirement_for_function(
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

fn static_import_missing_module_diagnostics(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<ProjectDiagnostic> {
    hir.imports
        .iter()
        .enumerate()
        .filter(|(index, import)| {
            import.linkage == rhai_hir::ImportLinkageKind::StaticText
                && snapshot.linked_import(file_id, *index).is_none()
        })
        .filter_map(|(index, import)| {
            let module_name = parse_static_import_module_name(import.module_text.as_deref()?)?;
            Some((index, module_name))
        })
        .filter(|(_, module_name)| {
            !snapshot
                .host_modules()
                .iter()
                .any(|module| module.name == *module_name)
                && !snapshot
                    .comment_directives(file_id)
                    .is_some_and(|directives| {
                        directives.external_modules.contains(module_name)
                            || directives.allowed_unresolved_imports.contains(module_name)
                    })
        })
        .map(|(index, module_name)| {
            let (kind, message) = if module_name_looks_path_like(module_name.as_str()) {
                (
                    ProjectDiagnosticKind::BrokenLinkedImport,
                    format!(
                        "import module `{}` does not resolve to an existing workspace file",
                        module_name
                    ),
                )
            } else {
                (
                    ProjectDiagnosticKind::Semantic,
                    format!("unresolved import module `{}`", module_name),
                )
            };

            ProjectDiagnostic {
                kind,
                code: match kind {
                    ProjectDiagnosticKind::BrokenLinkedImport => {
                        ProjectDiagnosticCode::BrokenLinkedImport
                    }
                    _ => ProjectDiagnosticCode::Semantic(
                        SemanticDiagnosticCode::UnresolvedImportModule,
                    ),
                },
                severity: ProjectDiagnosticSeverity::Error,
                range: hir
                    .import(index)
                    .module_range
                    .unwrap_or(hir.import(index).range),
                message,
                related_range: Some(hir.import(index).range),
                tags: Arc::from([]),
            }
        })
        .collect()
}

fn unresolved_import_member_path_diagnostics(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
) -> Vec<ProjectDiagnostic> {
    hir.references
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            let reference_id = ReferenceId(index as u32);
            let reference = hir.reference(reference_id);
            if reference.kind != rhai_hir::ReferenceKind::PathSegment {
                return None;
            }

            let expr_id = hir.expr_at_offset(reference.range.start())?;
            let path_expr = hir.path_expr(expr_id)?;
            if path_expr.segments.last().copied() != Some(reference_id) {
                return None;
            }

            let imported_path = hir.imported_module_path(expr_id)?;
            let has_linked_import = snapshot
                .linked_import(file_id, imported_path.import)
                .is_some();
            let (member_name, module_path) = imported_path.parts.split_last()?;
            let has_inline_completion = snapshot
                .imported_module_completions(file_id, module_path)
                .iter()
                .any(|completion| completion.name == *member_name);

            (has_linked_import
                && linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                    .is_empty()
                && !has_inline_completion)
                .then_some((reference_id, imported_path.import, imported_path.parts))
        })
        .map(|(reference_id, import_index, parts)| ProjectDiagnostic {
            kind: ProjectDiagnosticKind::Semantic,
            code: ProjectDiagnosticCode::UnresolvedImportMember,
            severity: ProjectDiagnosticSeverity::Error,
            range: hir.reference(reference_id).range,
            message: format!("unresolved import member `{}`", parts.join("::")),
            related_range: Some(hir.import(import_index).range),
            tags: Arc::from([]),
        })
        .collect()
}

fn parse_static_import_module_name(module_text: &str) -> Option<String> {
    if module_text.len() < 2 {
        return None;
    }

    if let Some(text) = module_text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .or_else(|| {
            module_text
                .strip_prefix('`')
                .and_then(|text| text.strip_suffix('`'))
        })
    {
        return Some(text.to_owned());
    }

    if !module_text.starts_with('r') {
        return None;
    }
    let quote = module_text.find('"')?;
    if !module_text.get(1..quote)?.chars().all(|ch| ch == '#') {
        return None;
    }
    let hashes = module_text.get(1..quote)?;
    let suffix = format!("\"{hashes}");
    module_text
        .ends_with(suffix.as_str())
        .then(|| {
            module_text
                .get(quote + 1..module_text.len() - suffix.len())
                .map(str::to_owned)
        })
        .flatten()
}

fn module_name_looks_path_like(module_name: &str) -> bool {
    module_name.contains('/')
        || module_name.contains('\\')
        || module_name.ends_with(".rhai")
        || module_name.starts_with("./")
        || module_name.starts_with("../")
        || module_name.starts_with(".\\")
        || module_name.starts_with("..\\")
        || Path::new(module_name).is_absolute()
}

fn reference_id_for_diagnostic(
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> Option<ReferenceId> {
    hir.reference_at(diagnostic.range)
        .or_else(|| hir.reference_at_offset(diagnostic.range.start()))
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

fn import_index_for_diagnostic(hir: &FileHir, diagnostic: &SemanticDiagnostic) -> Option<usize> {
    hir.imports.iter().position(|import| {
        import
            .module_reference
            .is_some_and(|reference_id| hir.reference(reference_id).range == diagnostic.range)
    })
}

fn linked_import_usage_diagnostics(
    hir: &FileHir,
    import_index: usize,
    kind: ProjectDiagnosticKind,
    message: String,
) -> Vec<ProjectDiagnostic> {
    let Some(alias_symbol) = hir.import(import_index).alias else {
        return Vec::new();
    };

    hir.references_to(alias_symbol)
        .map(|reference_id| ProjectDiagnostic {
            kind,
            code: match kind {
                ProjectDiagnosticKind::BrokenLinkedImport => {
                    ProjectDiagnosticCode::BrokenLinkedImport
                }
                ProjectDiagnosticKind::AmbiguousLinkedImport => {
                    ProjectDiagnosticCode::AmbiguousLinkedImport
                }
                ProjectDiagnosticKind::Semantic => {
                    ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
                }
                ProjectDiagnosticKind::Syntax => {
                    unreachable!("syntax diagnostics are not emitted here")
                }
            },
            severity: ProjectDiagnosticSeverity::Error,
            range: hir.reference(reference_id).range,
            message: message.clone(),
            related_range: Some(hir.import(import_index).range),
            tags: Arc::from([]),
        })
        .collect()
}

fn semantic_diagnostic_severity(kind: SemanticDiagnosticKind) -> ProjectDiagnosticSeverity {
    match kind {
        SemanticDiagnosticKind::UnusedSymbol => ProjectDiagnosticSeverity::Warning,
        _ => ProjectDiagnosticSeverity::Error,
    }
}

fn semantic_diagnostic_tags(kind: SemanticDiagnosticKind) -> Arc<[ProjectDiagnosticTag]> {
    match kind {
        SemanticDiagnosticKind::UnusedSymbol => Arc::from([ProjectDiagnosticTag::Unnecessary]),
        _ => Arc::from([]),
    }
}
