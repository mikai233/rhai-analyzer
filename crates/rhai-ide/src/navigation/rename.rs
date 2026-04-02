use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rhai_db::{DatabaseSnapshot, ProjectRenamePlan};
use rhai_syntax::TextRange;

use crate::support::convert::{
    navigation_target_from_identity, reference_location_from_db, text_size,
};
use crate::{
    FilePosition, FileRename, FileTextEdit, RenameIssue, RenamePlan, SourceChange, TextEdit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleResolutionMode {
    Relative,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRename {
    pub plan: RenamePlan,
    pub source_change: Option<SourceChange>,
}

pub(crate) fn prepare_rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> Option<PreparedRename> {
    prepare_rename_impl(snapshot, position, None)
}

pub(crate) fn rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: impl Into<String>,
) -> Option<PreparedRename> {
    prepare_rename_impl(snapshot, position, Some(new_name.into()))
}

fn prepare_rename_impl(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: Option<String>,
) -> Option<PreparedRename> {
    let is_prepare = new_name.is_none();
    let new_name = new_name.unwrap_or_default();
    if let Some(prepared) =
        prepare_static_import_module_rename(snapshot, position, &new_name, is_prepare)
    {
        return Some(prepared);
    }
    if let Some(prepared) = prepare_object_field_rename(snapshot, position, &new_name) {
        return Some(prepared);
    }

    let db_plan = snapshot.rename_plan(position.file_id, text_size(position.offset), new_name)?;
    let plan = rename_plan_from_db(&db_plan);
    let source_change = plan
        .issues
        .is_empty()
        .then(|| source_change_from_db_plan(&db_plan));

    Some(PreparedRename {
        plan,
        source_change,
    })
}

fn prepare_object_field_rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: &str,
) -> Option<PreparedRename> {
    let offset = text_size(position.offset);
    let hir = snapshot.hir(position.file_id)?;
    let on_field_reference = hir.reference_at_offset(offset).is_some_and(|reference_id| {
        hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
    });
    let on_field_declaration = hir
        .object_fields
        .iter()
        .any(|field| field.range.contains(offset));
    if !on_field_reference && !on_field_declaration {
        return None;
    }

    let references = snapshot.find_references(position.file_id, offset)?;
    let has_object_field_definition = references.references.iter().any(|reference| {
        reference.kind == rhai_db::ProjectReferenceKind::Definition
            && snapshot
                .hir(reference.file_id)
                .is_some_and(|definition_hir| {
                    definition_hir
                        .object_fields
                        .iter()
                        .any(|field| field.range == reference.range)
                })
    });
    if !has_object_field_definition {
        return None;
    }

    let mut issues = Vec::new();
    if new_name.trim().is_empty() {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: "field name cannot be empty".to_owned(),
            range: TextRange::new(offset, offset),
        });
    } else if !is_valid_field_identifier(new_name) {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: "field name must be a valid identifier".to_owned(),
            range: TextRange::new(offset, offset),
        });
    }

    let occurrences = references
        .references
        .iter()
        .map(reference_location_from_db)
        .collect::<Vec<_>>();
    let source_change = issues.is_empty().then(|| {
        let mut edits_by_file = BTreeMap::<_, Vec<TextEdit>>::new();
        for reference in &references.references {
            edits_by_file
                .entry(reference.file_id)
                .or_default()
                .push(TextEdit::replace(reference.range, new_name.to_owned()));
        }
        let file_edits = edits_by_file
            .into_iter()
            .map(|(file_id, mut edits)| {
                edits.sort_by(|left, right| {
                    left.range
                        .start()
                        .cmp(&right.range.start())
                        .then_with(|| left.range.end().cmp(&right.range.end()))
                });
                FileTextEdit::new(file_id, edits)
            })
            .collect::<Vec<_>>();
        SourceChange::new(file_edits)
    });

    Some(PreparedRename {
        plan: RenamePlan {
            new_name: new_name.to_owned(),
            targets: references
                .targets
                .iter()
                .map(navigation_target_from_identity)
                .collect(),
            occurrences,
            issues,
        },
        source_change,
    })
}

fn is_valid_field_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn prepare_static_import_module_rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: &str,
    is_prepare: bool,
) -> Option<PreparedRename> {
    let offset = text_size(position.offset);
    let hir = snapshot.hir(position.file_id)?;
    let (import_index, import) = hir.imports.iter().enumerate().find(|(_, import)| {
        import
            .module_range
            .is_some_and(|module_range| module_range.contains(offset))
            && import.linkage == rhai_hir::ImportLinkageKind::StaticText
    })?;
    let linked_import = snapshot.linked_import(position.file_id, import_index)?;
    let importer_path = snapshot.normalized_path(position.file_id)?;
    let provider_path = snapshot.normalized_path(linked_import.provider_file_id)?;
    let occurrences = collect_static_import_module_occurrences(
        snapshot,
        linked_import.provider_file_id,
        provider_path,
    );

    if is_prepare {
        return Some(PreparedRename {
            plan: RenamePlan {
                new_name: String::new(),
                targets: Vec::new(),
                occurrences: occurrences
                    .iter()
                    .map(|occurrence| crate::ReferenceLocation {
                        file_id: occurrence.file_id,
                        range: occurrence.range,
                        kind: crate::ReferenceKind::Reference,
                    })
                    .collect(),
                issues: Vec::new(),
            },
            source_change: None,
        });
    }

    let new_path = renamed_module_path(
        importer_path,
        provider_path,
        linked_import.module_name.as_str(),
        new_name,
    );

    let mut issues = Vec::new();
    if new_name.trim().is_empty() {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: "module name cannot be empty".to_owned(),
            range: import.module_range?,
        });
    }
    if let Some(existing_file_id) = snapshot.vfs().file_id(&new_path)
        && existing_file_id != linked_import.provider_file_id
    {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: format!(
                "renaming module `{}` would collide with existing file `{}`",
                linked_import.module_name,
                new_path.display()
            ),
            range: import.module_range?,
        });
    }

    let occurrences = collect_static_import_module_occurrences(
        snapshot,
        linked_import.provider_file_id,
        &new_path,
    );
    let source_change = issues.is_empty().then(|| {
        SourceChange::new(group_occurrence_text_edits(&occurrences)).with_file_renames(vec![
            FileRename::new(linked_import.provider_file_id, new_path.clone()),
        ])
    });

    Some(PreparedRename {
        plan: RenamePlan {
            new_name: new_name.to_owned(),
            targets: Vec::new(),
            occurrences: occurrences
                .iter()
                .map(|occurrence| crate::ReferenceLocation {
                    file_id: occurrence.file_id,
                    range: occurrence.range,
                    kind: crate::ReferenceKind::Reference,
                })
                .collect(),
            issues,
        },
        source_change,
    })
}

#[derive(Debug, Clone)]
struct StaticImportOccurrence {
    file_id: rhai_vfs::FileId,
    range: rhai_syntax::TextRange,
    replacement: String,
}

fn collect_static_import_module_occurrences(
    snapshot: &DatabaseSnapshot,
    provider_file_id: rhai_vfs::FileId,
    new_provider_path: &Path,
) -> Vec<StaticImportOccurrence> {
    let current_provider_path = snapshot.normalized_path(provider_file_id);
    let mut occurrences = snapshot
        .workspace_files()
        .into_iter()
        .filter_map(|file| {
            let hir = snapshot.hir(file.file_id)?;
            let importer_path = snapshot.normalized_path(file.file_id)?;
            Some(
                snapshot
                    .linked_imports(file.file_id)
                    .iter()
                    .filter(|linked_import| linked_import.provider_file_id == provider_file_id)
                    .filter_map(|linked_import| {
                        let import = hir.import(linked_import.import);
                        let range = import.module_range?;
                        let replacement = renamed_module_literal(
                            import.module_text.as_deref()?,
                            renamed_module_specifier(
                                importer_path,
                                current_provider_path?,
                                new_provider_path,
                                linked_import.module_name.as_str(),
                            )?,
                        )?;
                        Some(StaticImportOccurrence {
                            file_id: file.file_id,
                            range,
                            replacement,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect::<Vec<_>>();

    occurrences.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| left.range.start().cmp(&right.range.start()))
    });
    occurrences.dedup_by(|left, right| {
        left.file_id == right.file_id
            && left.range == right.range
            && left.replacement == right.replacement
    });
    occurrences
}

fn group_occurrence_text_edits(occurrences: &[StaticImportOccurrence]) -> Vec<FileTextEdit> {
    let mut edits_by_file = BTreeMap::<_, Vec<TextEdit>>::new();
    for occurrence in occurrences {
        edits_by_file
            .entry(occurrence.file_id)
            .or_default()
            .push(TextEdit::replace(
                occurrence.range,
                occurrence.replacement.clone(),
            ));
    }

    edits_by_file
        .into_iter()
        .map(|(file_id, mut edits)| {
            edits.sort_by(|left, right| {
                left.range
                    .start()
                    .cmp(&right.range.start())
                    .then_with(|| left.range.end().cmp(&right.range.end()))
            });
            FileTextEdit::new(file_id, edits)
        })
        .collect()
}

fn renamed_module_literal(original_literal: &str, new_module_name: String) -> Option<String> {
    let quote = original_literal.chars().next()?;
    let suffix = original_literal.chars().last()?;
    if quote != suffix {
        return None;
    }
    Some(format!("{quote}{new_module_name}{quote}"))
}

fn renamed_module_specifier(
    importer_path: &Path,
    current_provider_path: &Path,
    new_provider_path: &Path,
    original_module_name: &str,
) -> Option<String> {
    let mode = module_resolution_mode(importer_path, current_provider_path, original_module_name);
    let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));
    let mut module_path = match mode {
        ModuleResolutionMode::Relative => lexical_relative_path(importer_dir, new_provider_path)?,
        ModuleResolutionMode::Direct => new_provider_path.to_path_buf(),
    };

    if Path::new(original_module_name).extension().is_none()
        && current_provider_path.extension() == module_path.extension()
    {
        module_path.set_extension("");
    }

    let renamed = module_path.to_string_lossy().replace('\\', "/");
    (!renamed.is_empty()).then_some(renamed)
}

fn renamed_module_path(
    importer_path: &Path,
    provider_path: &Path,
    original_module_name: &str,
    new_name: &str,
) -> PathBuf {
    let mode = module_resolution_mode(importer_path, provider_path, original_module_name);
    match mode {
        ModuleResolutionMode::Relative => {
            let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));
            normalize_module_path(&importer_dir.join(module_path_with_extension(new_name)))
        }
        ModuleResolutionMode::Direct => {
            normalize_module_path(&module_path_with_extension(new_name))
        }
    }
}

fn module_resolution_mode(
    importer_path: &Path,
    provider_path: &Path,
    module_name: &str,
) -> ModuleResolutionMode {
    let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));
    let relative =
        normalize_module_path(&importer_dir.join(module_path_with_extension(module_name)));
    if relative == provider_path {
        ModuleResolutionMode::Relative
    } else {
        ModuleResolutionMode::Direct
    }
}

fn module_path_with_extension(module_name: &str) -> PathBuf {
    let mut path = PathBuf::from(module_name);
    if path.extension().is_none() {
        path.set_extension("rhai");
    }
    path
}

fn normalize_module_path(path: &Path) -> PathBuf {
    rhai_vfs::normalize_path(path)
}

fn lexical_relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    let base_components = base.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();

    if path_prefix(base) != path_prefix(target) || base.has_root() != target.has_root() {
        return None;
    }

    let mut shared = 0;
    while shared < base_components.len()
        && shared < target_components.len()
        && base_components[shared] == target_components[shared]
    {
        shared += 1;
    }

    let mut relative = PathBuf::new();
    for _ in shared..base_components.len() {
        relative.push("..");
    }
    for component in &target_components[shared..] {
        relative.push(component.as_os_str());
    }

    Some(relative)
}

fn path_prefix(path: &Path) -> Option<std::ffi::OsString> {
    path.components()
        .next()
        .and_then(|component| match component {
            std::path::Component::Prefix(prefix) => Some(prefix.as_os_str().to_owned()),
            _ => None,
        })
}

pub(crate) fn rename_plan_from_db(plan: &ProjectRenamePlan) -> RenamePlan {
    RenamePlan {
        new_name: plan.new_name.clone(),
        targets: plan
            .targets
            .iter()
            .map(navigation_target_from_identity)
            .collect(),
        occurrences: plan
            .occurrences
            .iter()
            .map(reference_location_from_db)
            .collect(),
        issues: plan
            .issues
            .iter()
            .map(|issue| RenameIssue {
                file_id: issue.file_id,
                message: issue.issue.message.clone(),
                range: issue.issue.range,
            })
            .collect(),
    }
}

fn source_change_from_db_plan(plan: &ProjectRenamePlan) -> SourceChange {
    let mut edits_by_file = BTreeMap::<_, Vec<TextEdit>>::new();

    for occurrence in &plan.occurrences {
        edits_by_file
            .entry(occurrence.file_id)
            .or_default()
            .push(TextEdit::replace(occurrence.range, plan.new_name.clone()));
    }

    let file_edits = edits_by_file
        .into_iter()
        .map(|(file_id, mut edits)| {
            edits.sort_by(|left, right| {
                left.range
                    .start()
                    .cmp(&right.range.start())
                    .then_with(|| left.range.end().cmp(&right.range.end()))
            });
            FileTextEdit::new(file_id, edits)
        })
        .collect();

    SourceChange::new(file_edits)
}
