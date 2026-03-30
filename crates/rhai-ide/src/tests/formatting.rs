use crate::AnalysisHost;
use crate::tests::assert_no_syntax_diagnostics;
use rhai_db::ChangeSet;
use rhai_syntax::TextRange;
use rhai_vfs::DocumentVersion;

#[test]
fn format_document_returns_whole_file_rewrite_when_formatter_changes_text() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run(){let value=1+2;value}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .format_document(file_id)
        .expect("expected formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert_eq!(edit.range.start(), 0.into());
    assert_eq!(
        edit.new_text,
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n"
    );
}

#[test]
fn format_document_returns_none_when_text_is_already_stable() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(analysis.format_document(file_id).is_none());
}

#[test]
fn format_range_returns_partial_edit_when_selection_intersects_changed_region() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let prefix = 1;\nfn run(){let value=1+2;value}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .file_id_for_path(std::path::Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let start = "let prefix = 1;\n".len() as u32;
    let end = start + "fn run(){let value=1+2;value}\n".len() as u32;
    let change = analysis
        .format_range(file_id, TextRange::new(start.into(), end.into()))
        .expect("expected range formatting change");
    let [file_edit] = change.file_edits.as_slice() else {
        panic!("expected one file edit");
    };
    let [edit] = file_edit.edits.as_slice() else {
        panic!("expected one text edit");
    };

    assert_eq!(u32::from(edit.range.start()), start);
    assert!(u32::from(edit.range.end()) <= end);
    assert!(edit.new_text.contains("fn run"));
    assert!(edit.new_text.contains("let value = 1 + 2;"));
}
