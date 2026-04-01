use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::AnalysisHost;
use crate::tests::assert_no_syntax_diagnostics;

#[test]
fn diagnostics_return_empty_for_missing_files() {
    let host = AnalysisHost::default();
    let analysis = host.snapshot();

    assert!(analysis.diagnostics(rhai_vfs::FileId(999)).is_empty());
}
#[test]
fn document_symbols_use_database_indexes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn outer() {}
            fn helper() {}

            const LIMIT = 1;
            let exported_limit = LIMIT;
            export exported_limit as public_outer;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let document_symbols = analysis.document_symbols(file_id);

    assert_eq!(
        document_symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["outer", "helper", "LIMIT", "exported_limit", "public_outer"]
    );
    assert!(document_symbols[0].children.is_empty());
    assert!(document_symbols[1].children.is_empty());
}
#[test]
fn workspace_symbols_include_file_identity() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "one.rhai".into(),
                text: "fn alpha() {}".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "two.rhai".into(),
                text: "fn beta() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let one = analysis
        .db
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = analysis
        .db
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");
    assert_no_syntax_diagnostics(&analysis, one);
    assert_no_syntax_diagnostics(&analysis, two);

    assert_eq!(
        analysis
            .workspace_symbols()
            .iter()
            .map(|symbol| (symbol.file_id, symbol.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(one, "alpha"), (two, "beta")]
    );
}
