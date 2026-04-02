use rhai_db::{DatabaseSnapshot, ProjectDiagnosticKind};
use rhai_hir::ReferenceKind as HirReferenceKind;
use rhai_syntax::TextRange;
use rhai_vfs::FileId;

use crate::imports::{
    import_source_assists, ranges_intersect_or_touch, remove_import_edit,
    unused_import_assists_for_diagnostic,
};
use crate::{Diagnostic, FileTextEdit, SourceChange, TextEdit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssistId(&'static str);

impl AssistId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssistKind {
    QuickFix,
    Refactor,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assist {
    pub id: AssistId,
    pub kind: AssistKind,
    pub group: Option<String>,
    pub label: String,
    pub target: TextRange,
    pub source_change: SourceChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticWithFixes {
    pub diagnostic: Diagnostic,
    pub fixes: Vec<Assist>,
}

const AUTO_IMPORT_ASSIST_ID: AssistId = AssistId::new("import.auto");
const REMOVE_BROKEN_IMPORT_ASSIST_ID: AssistId = AssistId::new("import.remove_broken");

pub(crate) fn assists_for_range(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    range: TextRange,
) -> Vec<Assist> {
    let mut assists = auto_import_assists(snapshot, file_id, range);
    assists.extend(import_cleanup_assists(snapshot, file_id, range));
    assists.extend(import_source_assists(snapshot, file_id, range));
    dedupe_assists(assists)
}

pub(crate) fn diagnostics_with_fixes(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
) -> Vec<DiagnosticWithFixes> {
    let project_diagnostics = snapshot.project_diagnostics(file_id);

    project_diagnostics
        .into_iter()
        .map(|diagnostic| {
            let range = diagnostic.range;
            let related_range = diagnostic.related_range;
            let fixes = match diagnostic.kind {
                ProjectDiagnosticKind::Syntax => Vec::new(),
                ProjectDiagnosticKind::Semantic => {
                    let mut fixes = auto_import_assists(snapshot, file_id, range);
                    fixes.extend(unused_import_assists_for_diagnostic(
                        snapshot, file_id, range,
                    ));
                    fixes.extend(import_cleanup_assists_for_diagnostic(
                        snapshot,
                        file_id,
                        diagnostic.kind,
                        range,
                        related_range,
                    ));
                    dedupe_assists(fixes)
                }
                ProjectDiagnosticKind::BrokenLinkedImport
                | ProjectDiagnosticKind::AmbiguousLinkedImport => {
                    import_cleanup_assists_for_diagnostic(
                        snapshot,
                        file_id,
                        diagnostic.kind,
                        range,
                        related_range,
                    )
                }
            };

            DiagnosticWithFixes {
                diagnostic: Diagnostic {
                    code: diagnostic.code,
                    message: diagnostic.message,
                    range,
                    severity: match diagnostic.severity {
                        rhai_db::ProjectDiagnosticSeverity::Error => {
                            crate::DiagnosticSeverity::Error
                        }
                        rhai_db::ProjectDiagnosticSeverity::Warning => {
                            crate::DiagnosticSeverity::Warning
                        }
                    },
                    tags: diagnostic
                        .tags
                        .iter()
                        .map(|tag| match tag {
                            rhai_db::ProjectDiagnosticTag::Unnecessary => {
                                crate::DiagnosticTag::Unnecessary
                            }
                        })
                        .collect(),
                },
                fixes,
            }
        })
        .collect()
}

fn auto_import_assists(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    range: TextRange,
) -> Vec<Assist> {
    let Some(hir) = snapshot.hir(file_id) else {
        return Vec::new();
    };

    hir.references
        .iter()
        .filter(|reference| {
            reference.target.is_none()
                && matches!(
                    reference.kind,
                    HirReferenceKind::Name | HirReferenceKind::PathSegment
                )
                && ranges_intersect_or_touch(range, reference.range)
        })
        .filter_map(|reference| {
            hir.reference_at(reference.range)
                .map(|reference_id| (reference_id, reference))
        })
        .flat_map(|(_, reference)| {
            snapshot
                .auto_import_candidates(file_id, reference.range.start())
                .into_iter()
                .map(move |candidate| {
                    let label = if candidate.insert_text.is_empty() {
                        format!("Qualify with `{}`", candidate.alias)
                    } else {
                        format!("Import `{}`", candidate.module_name)
                    };
                    let mut edits = vec![TextEdit::replace(
                        candidate.replace_range,
                        candidate.qualified_reference_text,
                    )];
                    if !candidate.insert_text.is_empty() {
                        edits.push(TextEdit::insert(
                            candidate.insertion_offset,
                            candidate.insert_text,
                        ));
                    }

                    Assist {
                        id: AUTO_IMPORT_ASSIST_ID,
                        kind: AssistKind::QuickFix,
                        group: Some("Import".to_owned()),
                        label,
                        target: reference.range,
                        source_change: SourceChange::new(vec![FileTextEdit::new(file_id, edits)]),
                    }
                })
        })
        .collect()
}

fn import_cleanup_assists(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    range: TextRange,
) -> Vec<Assist> {
    snapshot
        .project_diagnostics(file_id)
        .into_iter()
        .filter(|diagnostic| {
            ranges_intersect_or_touch(range, diagnostic.range)
                || diagnostic
                    .related_range
                    .is_some_and(|related| ranges_intersect_or_touch(range, related))
        })
        .flat_map(|diagnostic| {
            import_cleanup_assists_for_diagnostic(
                snapshot,
                file_id,
                diagnostic.kind,
                diagnostic.range,
                diagnostic.related_range,
            )
        })
        .collect()
}

fn import_cleanup_assists_for_diagnostic(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    kind: ProjectDiagnosticKind,
    diagnostic_range: TextRange,
    related_range: Option<TextRange>,
) -> Vec<Assist> {
    let Some(import_range) = related_range else {
        return Vec::new();
    };
    let Some(file_text) = snapshot.file_text(file_id) else {
        return Vec::new();
    };

    let label = match kind {
        ProjectDiagnosticKind::Semantic => "Remove unresolved import".to_owned(),
        ProjectDiagnosticKind::BrokenLinkedImport => "Remove broken import".to_owned(),
        ProjectDiagnosticKind::AmbiguousLinkedImport => "Remove ambiguous import".to_owned(),
        ProjectDiagnosticKind::Syntax => return Vec::new(),
    };

    let edit = remove_import_edit(import_range, file_text.as_ref());
    vec![Assist {
        id: REMOVE_BROKEN_IMPORT_ASSIST_ID,
        kind: AssistKind::QuickFix,
        group: Some("Import".to_owned()),
        label,
        target: diagnostic_range,
        source_change: SourceChange::new(vec![FileTextEdit::new(file_id, vec![edit])]),
    }]
}

fn dedupe_assists(mut assists: Vec<Assist>) -> Vec<Assist> {
    assists.sort_by(|left, right| {
        left.id
            .as_str()
            .cmp(right.id.as_str())
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.target.start().cmp(&right.target.start()))
            .then_with(|| left.target.end().cmp(&right.target.end()))
    });
    assists.dedup_by(|left, right| {
        left.id == right.id
            && left.kind == right.kind
            && left.label == right.label
            && left.target == right.target
            && left.source_change == right.source_change
    });
    assists
}
