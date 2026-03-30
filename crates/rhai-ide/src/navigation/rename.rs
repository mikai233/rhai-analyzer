use std::collections::BTreeMap;

use rhai_db::{DatabaseSnapshot, ProjectRenamePlan};

use crate::support::convert::{
    navigation_target_from_identity, reference_location_from_db, text_size,
};
use crate::{FilePosition, FileTextEdit, RenameIssue, RenamePlan, SourceChange, TextEdit};

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
