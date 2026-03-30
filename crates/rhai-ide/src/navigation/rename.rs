use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rhai_db::{DatabaseSnapshot, ProjectRenamePlan};

use crate::support::convert::{
    navigation_target_from_identity, reference_location_from_db, text_size,
};
use crate::{
    FilePosition, FileRename, FileTextEdit, RenameIssue, RenamePlan, SourceChange, TextEdit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRename {
    pub plan: RenamePlan,
    pub source_change: Option<SourceChange>,
}

pub(crate) fn prepare_rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: impl Into<String>,
) -> Option<PreparedRename> {
    let new_name = new_name.into();
    if let Some(prepared) = prepare_static_import_module_rename(snapshot, position, &new_name) {
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

fn prepare_static_import_module_rename(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
    new_name: &str,
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
    let provider_path = snapshot.normalized_path(linked_import.provider_file_id)?;
    let new_path = renamed_module_path(provider_path, new_name);

    let mut issues = Vec::new();
    if new_name.trim().is_empty() {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: "module name cannot be empty".to_owned(),
            range: import.module_range?,
        });
    }
    if new_name.contains(['/', '\\']) {
        issues.push(RenameIssue {
            file_id: position.file_id,
            message: "module rename only supports changing the file name, not path segments"
                .to_owned(),
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
        new_name,
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
    new_name: &str,
) -> Vec<StaticImportOccurrence> {
    let mut occurrences = snapshot
        .workspace_files()
        .into_iter()
        .filter_map(|file| {
            let hir = snapshot.hir(file.file_id)?;
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
                            linked_import.module_name.as_str(),
                            new_name,
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

fn renamed_module_literal(
    original_literal: &str,
    module_name: &str,
    new_name: &str,
) -> Option<String> {
    let quote = original_literal.chars().next()?;
    let suffix = original_literal.chars().last()?;
    if quote != suffix {
        return None;
    }
    let renamed = rename_module_specifier(module_name, new_name)?;
    Some(format!("{quote}{renamed}{quote}"))
}

fn rename_module_specifier(module_name: &str, new_name: &str) -> Option<String> {
    let (prefix, leaf) = module_name
        .rsplit_once(['/', '\\'])
        .map_or(("", module_name), |(prefix, leaf)| (prefix, leaf));
    if leaf.is_empty() {
        return None;
    }

    let extension = Path::new(leaf)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let renamed_leaf = if extension.is_empty() {
        new_name.to_owned()
    } else {
        format!("{new_name}{extension}")
    };

    Some(if prefix.is_empty() {
        renamed_leaf
    } else {
        format!("{prefix}/{renamed_leaf}")
    })
}

fn renamed_module_path(provider_path: &Path, new_name: &str) -> PathBuf {
    let extension = provider_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let new_file_name = if Path::new(new_name).extension().is_some() {
        new_name.to_owned()
    } else {
        format!("{new_name}{extension}")
    };
    provider_path.with_file_name(new_file_name)
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
