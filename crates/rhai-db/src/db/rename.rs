use crate::db::DatabaseSnapshot;
use crate::db::imports::{export_matches_identity, project_identity_for_export};
use crate::types::{
    LocatedModuleExport, LocatedRenamePreflightIssue, LocatedSymbolIdentity, ProjectReferenceKind,
    ProjectRenamePlan,
};
use rhai_hir::{FileBackedSymbolIdentity, RenamePreflightIssue, RenamePreflightIssueKind};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

impl DatabaseSnapshot {
    pub fn rename_plan(
        &self,
        file_id: FileId,
        offset: TextSize,
        new_name: impl Into<String>,
    ) -> Option<ProjectRenamePlan> {
        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        let new_name = new_name.into();
        let mut issues = Vec::new();

        for target in &targets {
            let Some(analysis) = self.analysis.get(&target.file_id) else {
                continue;
            };

            let local_plan = analysis
                .hir
                .rename_plan(target.symbol.symbol, new_name.clone());
            issues.extend(
                local_plan
                    .issues
                    .into_iter()
                    .map(|issue| LocatedRenamePreflightIssue {
                        file_id: target.file_id,
                        issue,
                    }),
            );
        }

        issues.extend(self.project_rename_preflight_issues(&targets, &new_name));

        Some(ProjectRenamePlan {
            occurrences: self.collect_project_references(&targets),
            targets,
            new_name,
            issues,
        })
    }

    fn exports_for_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Vec<&LocatedModuleExport> {
        self.workspace_exports
            .iter()
            .filter(|export| export_matches_identity(export, identity))
            .collect()
    }

    fn project_rename_preflight_issues(
        &self,
        targets: &[LocatedSymbolIdentity],
        new_name: &str,
    ) -> Vec<LocatedRenamePreflightIssue> {
        let mut issues = Vec::new();

        for target in targets {
            for export in self.exports_for_identity(&target.symbol) {
                if export.export.exported_name.as_deref() == Some(new_name) {
                    continue;
                }

                for conflict in self.exports_named(new_name) {
                    if same_export_edge(export, conflict) {
                        continue;
                    }

                    issues.push(LocatedRenamePreflightIssue {
                        file_id: target.file_id,
                        issue: RenamePreflightIssue {
                            kind: RenamePreflightIssueKind::DuplicateDefinition,
                            message: format!(
                                "renaming exported symbol `{}` to `{new_name}` would collide with another workspace export",
                                target.symbol.name
                            ),
                            range: target.symbol.declaration_range,
                            related_symbol: project_identity_for_export(conflict).cloned(),
                        },
                    });
                }
            }
        }

        issues.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.issue.range.start().cmp(&right.issue.range.start()))
                .then_with(|| left.issue.message.cmp(&right.issue.message))
        });
        issues.dedup_by(|left, right| {
            left.file_id == right.file_id
                && left.issue.range == right.issue.range
                && left.issue.kind == right.issue.kind
                && left.issue.message == right.issue.message
        });
        issues
    }
}

pub(crate) fn project_reference_kind_rank(kind: ProjectReferenceKind) -> u8 {
    match kind {
        ProjectReferenceKind::Definition => 0,
        ProjectReferenceKind::Reference => 1,
        ProjectReferenceKind::LinkedImport => 2,
    }
}

pub(crate) fn same_export_edge(left: &LocatedModuleExport, right: &LocatedModuleExport) -> bool {
    left.file_id == right.file_id && left.export.export == right.export.export
}
