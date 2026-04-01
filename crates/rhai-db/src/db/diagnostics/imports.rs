use crate::db::DatabaseSnapshot;
use crate::db::imports::linked_import_targets_for_path_reference;
use crate::types::{
    ProjectDiagnostic, ProjectDiagnosticCode, ProjectDiagnosticKind, ProjectDiagnosticSeverity,
};
use rhai_hir::{FileHir, ReferenceId, SemanticDiagnostic, SemanticDiagnosticCode};
use rhai_vfs::FileId;
use std::path::Path;
use std::sync::Arc;

pub(crate) fn static_import_missing_module_diagnostics(
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

pub(crate) fn unresolved_import_member_path_diagnostics(
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

pub(crate) fn parse_static_import_module_name(module_text: &str) -> Option<String> {
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

pub(crate) fn module_name_looks_path_like(module_name: &str) -> bool {
    module_name.contains('/')
        || module_name.contains('\\')
        || module_name.ends_with(".rhai")
        || module_name.starts_with("./")
        || module_name.starts_with("../")
        || module_name.starts_with(".\\")
        || module_name.starts_with("..\\")
        || Path::new(module_name).is_absolute()
}

pub(crate) fn import_index_for_diagnostic(
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> Option<usize> {
    hir.imports.iter().position(|import| {
        import
            .module_reference
            .is_some_and(|reference_id| hir.reference(reference_id).range == diagnostic.range)
    })
}

pub(crate) fn linked_import_usage_diagnostics(
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
