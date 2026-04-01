use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn signature_help_returns_host_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
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
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by route".to_owned()),
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(int) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by id".to_owned()),
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
    let offset = u32::try_from(text.find("\"home\"").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.active_signature, 0);
    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.signatures[0].label, "fn open(string) -> bool");
    assert_eq!(help.signatures[1].label, "fn open(int) -> bool");
    assert_eq!(help.signatures[0].docs.as_deref(), Some("Open by route"));
    assert_eq!(help.signatures[1].docs.as_deref(), Some("Open by id"));
}
#[test]
fn signature_help_prefers_host_method_overload_matching_argument_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
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
    let offset = u32::try_from(text.find("\"home\"").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.active_signature, 1);
    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.signatures[0].label, "fn open(int) -> bool");
    assert_eq!(help.signatures[1].label, "fn open(string) -> bool");
}
#[test]
fn signature_help_specializes_generic_host_method_signatures_from_receiver_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param boxed Box<int>
                fn run(boxed) {
                    boxed.unwrap_or(value);
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Box<T>".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "unwrap_or".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(T) -> T".to_owned(),
                            return_type: None,
                            docs: Some("Return the boxed value or a fallback".to_owned()),
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
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn unwrap_or(int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}
