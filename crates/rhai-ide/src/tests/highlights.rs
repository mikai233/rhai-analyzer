use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, DocumentHighlightKind, FilePosition};

#[test]
fn document_highlights_include_local_function_declaration_and_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}

            fn run() {
                helper();
                helper();
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.rfind("helper();").expect("expected helper call")).expect("offset");

    let highlights = analysis.document_highlights(FilePosition { file_id, offset });
    assert_eq!(highlights.len(), 3);
    assert_eq!(highlights[0].kind, DocumentHighlightKind::Write);
    assert!(
        highlights[1..]
            .iter()
            .all(|highlight| highlight.kind == DocumentHighlightKind::Read),
        "expected call highlights to be read accesses, got {highlights:?}"
    );
}

#[test]
fn document_highlights_include_local_variable_definition_and_reads() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = 1;
                value + value
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.rfind("value").expect("expected value read")).expect("offset");

    let highlights = analysis.document_highlights(FilePosition { file_id, offset });
    assert_eq!(highlights.len(), 3);
    assert_eq!(highlights[0].kind, DocumentHighlightKind::Write);
    assert!(
        highlights[1..]
            .iter()
            .all(|highlight| highlight.kind == DocumentHighlightKind::Read),
        "expected value reads to be read highlights, got {highlights:?}"
    );
}
