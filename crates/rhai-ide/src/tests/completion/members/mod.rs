use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use rhai_vfs::FileId;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{Analysis, AnalysisHost, CompletionItemSource, FilePosition};

pub(crate) mod builtins;
pub(crate) mod inferred;
pub(crate) mod project_types;

pub(crate) fn load_analysis(source: &str) -> (Analysis, FileId, String) {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        source,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    (analysis, file_id, text.to_string())
}

pub(crate) fn load_analysis_with_project(
    source: &str,
    project: ProjectConfig,
) -> (Analysis, FileId, String) {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: source.to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(project),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    (analysis, file_id, text.to_string())
}

pub(crate) fn completions_at(
    analysis: &Analysis,
    file_id: FileId,
    offset: u32,
) -> Vec<crate::CompletionItem> {
    analysis.completions(FilePosition { file_id, offset })
}

pub(crate) fn member_completion<'a>(
    completions: &'a [crate::CompletionItem],
    label: &str,
) -> &'a crate::CompletionItem {
    completions
        .iter()
        .find(|item| item.label == label && item.source == CompletionItemSource::Member)
        .expect("expected member completion")
}
