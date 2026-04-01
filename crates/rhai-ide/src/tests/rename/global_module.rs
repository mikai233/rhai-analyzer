use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn rename_updates_automatic_global_constant_usages() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            const ANSWER = 42;

            fn run() {
                global::ANSWER
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected file text");
    let offset =
        u32::try_from(text.find("ANSWER =").expect("expected declaration")).expect("offset");

    let prepared = analysis
        .rename(FilePosition { file_id, offset }, "RESULT".to_owned())
        .expect("expected rename");
    assert!(
        prepared.plan.issues.is_empty(),
        "{:?}",
        prepared.plan.issues
    );

    let source_change = prepared
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_edits.len(), 1);
    let file_edit = &source_change.file_edits[0];
    assert_eq!(file_edit.file_id, file_id);
    assert_eq!(file_edit.edits.len(), 2);
    assert!(file_edit.edits.iter().all(|edit| edit.new_text == "RESULT"));
}
