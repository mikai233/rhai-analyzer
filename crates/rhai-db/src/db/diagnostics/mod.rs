use crate::db::DatabaseSnapshot;
use crate::db::diagnostics::caller_scope::{
    caller_scope_regular_call_diagnostics, unresolved_name_requires_caller_scope,
};
use crate::db::diagnostics::imports::{
    import_index_for_diagnostic, linked_import_usage_diagnostics,
    static_import_missing_module_diagnostics, unresolved_import_member_path_diagnostics,
};
use crate::types::{
    ProjectDiagnostic, ProjectDiagnosticCode, ProjectDiagnosticKind, ProjectDiagnosticSeverity,
    ProjectDiagnosticTag,
};
use rhai_hir::{FileHir, ReferenceId, SemanticDiagnostic, SemanticDiagnosticKind, SymbolId};
use rhai_syntax::TextRange;
use rhai_vfs::FileId;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) mod caller_scope;
pub(crate) mod imports;

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

pub(crate) fn project_diagnostic_kind_rank(kind: ProjectDiagnosticKind) -> u8 {
    match kind {
        ProjectDiagnosticKind::Syntax => 0,
        ProjectDiagnosticKind::Semantic => 1,
        ProjectDiagnosticKind::BrokenLinkedImport => 2,
        ProjectDiagnosticKind::AmbiguousLinkedImport => 3,
    }
}

pub(crate) fn project_semantic_diagnostics(
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

pub(crate) fn unresolved_name_is_known_external(
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

pub(crate) fn reference_id_for_diagnostic(
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> Option<ReferenceId> {
    hir.reference_at(diagnostic.range)
        .or_else(|| hir.reference_at_offset(diagnostic.range.start()))
}

pub(crate) fn semantic_diagnostic_severity(
    kind: SemanticDiagnosticKind,
) -> ProjectDiagnosticSeverity {
    match kind {
        SemanticDiagnosticKind::UnusedSymbol => ProjectDiagnosticSeverity::Warning,
        _ => ProjectDiagnosticSeverity::Error,
    }
}

pub(crate) fn semantic_diagnostic_tags(
    kind: SemanticDiagnosticKind,
) -> Arc<[ProjectDiagnosticTag]> {
    match kind {
        SemanticDiagnosticKind::UnusedSymbol => Arc::from([ProjectDiagnosticTag::Unnecessary]),
        _ => Arc::from([]),
    }
}
