use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, HoverSignatureSource};

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
    assert_eq!(function_hover.source, HoverSignatureSource::Inferred);
    assert!(function_hover.declared_signature.is_none());
    assert_eq!(
        function_hover.inferred_signature.as_deref(),
        Some("fn echo(blob) -> blob")
    );

    let variable_offset =
        u32::try_from(text.find("result =").expect("expected result declaration")).expect("offset");
    let variable_hover = analysis
        .hover(FilePosition {
            file_id,
            offset: variable_offset,
        })
        .expect("expected variable hover");
    assert_eq!(variable_hover.signature, "let result: blob");
    assert_eq!(variable_hover.source, HoverSignatureSource::Inferred);
}

#[test]
fn hover_formats_ambiguous_inferred_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;
                let seed = if flag { 1 } else { "home" };
                let result = widget.open(seed);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "open".to_owned(),
                        vec![
                            rhai_project::FunctionSpec {
                                signature: "fun(int) -> int".to_owned(),
                                return_type: None,
                                docs: None,
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: None,
                            },
                        ],
                    )]
                    .into_iter()
                    .collect(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let variable_offset =
        u32::try_from(text.find("result =").expect("expected result declaration")).expect("offset");
    let hover = analysis
        .hover(FilePosition {
            file_id,
            offset: variable_offset,
        })
        .expect("expected variable hover");

    assert_eq!(hover.signature, "let result: ambiguous<int | bool>");
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Multiple call candidates remain viable at this location.")
    );
}

#[test]
fn hover_keeps_declared_signature_and_surfaces_inferred_type_notes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                /// @type any
                let result = blob(10);
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
    let variable_offset =
        u32::try_from(text.find("result =").expect("expected result declaration")).expect("offset");

    let hover = analysis
        .hover(FilePosition {
            file_id,
            offset: variable_offset,
        })
        .expect("expected variable hover");

    assert_eq!(hover.signature, "let result: any");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
    assert_eq!(hover.declared_signature.as_deref(), Some("let result: any"));
    assert_eq!(
        hover.inferred_signature.as_deref(),
        Some("let result: any | blob")
    );
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Inferred type: let result: any | blob")
    );
}

#[test]
fn hover_supports_builtin_global_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                print("hello");
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
    let offset = u32::try_from(text.find("print").expect("expected print call")).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin hover");

    assert_eq!(hover.signature, "fn print(any) -> ()");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
    assert_eq!(
        hover.docs.as_deref(),
        Some("Print a value via the engine's print callback.")
    );
}
