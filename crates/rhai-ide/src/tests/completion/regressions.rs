use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionItemSource, FilePosition};

#[test]
fn completions_do_not_panic_when_offset_lands_inside_multibyte_punctuation() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"demo\" as dd;\n\nlet q = 1.0 + 2。;\n\ndd::test();\nfn v() {}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let punctuation_offset =
        u32::try_from(text.find('。').expect("expected unicode punctuation") + 1).expect("offset");

    let _completions = analysis.completions(FilePosition {
        file_id,
        offset: punctuation_offset,
    });
}

#[test]
fn completions_work_at_end_of_file_without_trailing_newline() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let name = \"mikai233\";\nn",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.len()).expect("expected offset to fit");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "name" && item.source == CompletionItemSource::Visible),
        "expected visible completion at end of file, got {completions:?}"
    );
}

#[test]
fn member_completions_work_at_end_of_file_without_trailing_newline() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let name = \"mikai233\";\nname.",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.len()).expect("expected offset to fit");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "contains" && item.source == CompletionItemSource::Member),
        "expected member completion at end of file, got {completions:?}"
    );
}
