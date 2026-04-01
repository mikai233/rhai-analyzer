use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, HoverSignatureSource};

#[test]
fn hover_supports_host_method_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param widget Widget
                fn run(widget) {
                    widget.open("home");
                }
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
                                signature: "fun(int) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by id".to_owned()),
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by route".to_owned()),
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
    let offset =
        u32::try_from(text.find(".open").expect("expected method call") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected host method hover");

    assert_eq!(hover.signature, "fn open(string) -> bool");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
    assert_eq!(hover.docs.as_deref(), Some("Open by route"));
    assert!(
        hover
            .notes
            .iter()
            .any(|note| { note == "2 overloads are available for this callable." })
    );
}
#[test]
fn hover_supports_host_method_member_accesses() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;
                let open_fn = widget.open;
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
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(string) -> bool".to_owned(),
                            return_type: None,
                            docs: Some("Open the widget".to_owned()),
                        }],
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
    let offset =
        u32::try_from(text.find(".open").expect("expected method access") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected host method member hover");

    assert_eq!(hover.signature, "fn open(string) -> bool");
    assert_eq!(hover.docs.as_deref(), Some("Open the widget"));
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}
#[test]
fn hover_supports_builtin_host_methods() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                "hello".to_blob();
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
        u32::try_from(text.find(".to_blob").expect("expected builtin method") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin method hover");

    assert_eq!(hover.signature, "fn to_blob() -> blob");
    assert_eq!(
        hover.docs.as_deref(),
        Some("Converts the string into a UTF-8 encoded BLOB.")
    );
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}
#[test]
fn hover_supports_builtin_universal_methods() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run(value) {
                value.type_of();
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
    let offset = u32::try_from(text.find(".type_of").expect("expected universal method") + 2)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin universal method hover");

    assert_eq!(hover.signature, "fn type_of() -> string");
    assert_eq!(
        hover.docs.as_deref(),
        Some("Returns the dynamic type name of the current value.")
    );
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}
