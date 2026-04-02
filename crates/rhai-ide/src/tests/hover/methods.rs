use std::path::Path;

use rhai_db::{ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, HoverSignatureSource};

fn assert_structured_builtin_docs(docs: &str, topic: &str) {
    assert!(!docs.trim().is_empty());
    assert!(docs.contains("## Usage"));
    assert!(docs.contains("## Examples"));
    assert!(docs.contains("## Official Rhai Reference"));
    assert!(docs.contains(topic));
}

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
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "to_blob");
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
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "type_of");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}

#[test]
fn hover_supports_builtin_shared_introspection_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run(value) {
                value.is_shared();
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
    let offset = u32::try_from(text.find(".is_shared").expect("expected is_shared method") + 2)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin universal method hover");

    assert_eq!(hover.signature, "fn is_shared() -> bool");
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "is_shared");
    assert!(docs.contains("shared"));
}

#[test]
fn hover_supports_builtin_dynamic_tag_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run(value) {
                value.tag();
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
        u32::try_from(text.find(".tag").expect("expected tag method") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin universal method hover");

    assert_eq!(hover.signature, "fn tag() -> int");
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "tag");
}

#[test]
fn hover_prefers_map_tag_field_function_over_builtin_dynamic_tag_method() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ tag: || "field-fn", name: "Ada" };
                user.tag();
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
        u32::try_from(text.find(".tag").expect("expected tag method") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected field function hover");

    assert_eq!(hover.signature, "field tag: fun() -> string");
}

#[test]
fn hover_supports_builtin_map_methods_with_examples() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ name: "Ada", active: true };
                user.get("name");
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
        u32::try_from(text.find(".get").expect("expected map method call") + 2).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected builtin map method hover");

    assert_eq!(hover.signature, "fn get(string) -> any | ()");
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "get");
    assert!(docs.contains("map") || docs.contains("property"));
}
