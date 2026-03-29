use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn hover_falls_back_to_inferred_function_and_variable_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                return value;
            }

            fn run() {
                let result = echo(blob(10));
                echo(blob(10));
                result;
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

    let function_offset =
        u32::try_from(text.rfind("echo(blob(10));").expect("expected echo call")).expect("offset");
    let function_hover = analysis
        .hover(FilePosition {
            file_id,
            offset: function_offset,
        })
        .expect("expected function hover");
    assert_eq!(function_hover.signature, "fn echo(blob) -> blob");

    let variable_offset =
        u32::try_from(text.find("result =").expect("expected result declaration")).expect("offset");
    let variable_hover = analysis
        .hover(FilePosition {
            file_id,
            offset: variable_offset,
        })
        .expect("expected variable hover");
    assert_eq!(variable_hover.signature, "let result: blob");
}
