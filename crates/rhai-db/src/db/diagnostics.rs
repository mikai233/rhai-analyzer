use crate::db::DatabaseSnapshot;
use crate::types::{ProjectDiagnostic, ProjectDiagnosticKind};
use rhai_hir::{FileHir, SemanticDiagnostic, SemanticDiagnosticKind};
use rhai_vfs::FileId;

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
                range: diagnostic.range(),
                message: diagnostic.message().to_owned(),
                related_range: None,
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

    for diagnostic in diagnostics {
        if diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && unresolved_name_is_known_external(snapshot, hir, diagnostic)
        {
            continue;
        }

        if diagnostic.kind != SemanticDiagnosticKind::UnresolvedImport {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
            });
            continue;
        }

        let Some(import_index) = import_index_for_diagnostic(hir, diagnostic) else {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
            });
            continue;
        };

        match snapshot.linked_import(file_id, import_index) {
            Some(linked_import) if linked_import.exports.len() == 1 => {}
            Some(linked_import) => {
                projected.push(ProjectDiagnostic {
                    kind: ProjectDiagnosticKind::AmbiguousLinkedImport,
                    range: diagnostic.range,
                    message: format!(
                        "ambiguous import module `{}` matches multiple workspace exports",
                        linked_import.module_name
                    ),
                    related_range: Some(hir.import(import_index).range),
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
                    range: diagnostic.range,
                    message: diagnostic.message.clone(),
                    related_range: diagnostic.related_range,
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

    projected
}

fn unresolved_name_is_known_external(
    snapshot: &DatabaseSnapshot,
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> bool {
    let Some(reference_id) = hir
        .reference_at(diagnostic.range)
        .or_else(|| hir.reference_at_offset(diagnostic.range.start()))
    else {
        return false;
    };
    let name = hir.reference(reference_id).name.as_str();
    snapshot.external_signatures().get(name).is_some() || snapshot.global_function(name).is_some()
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
            range: hir.reference(reference_id).range,
            message: message.clone(),
            related_range: Some(hir.import(import_index).range),
        })
        .collect()
}
