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
    let docs = hover.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "print");
}
#[test]
fn hover_supports_host_module_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                import "env" as env;

                fn run() {
                    env::test(1);
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "env".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Environment helpers".to_owned()),
                    functions: [(
                        "test".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int) -> int".to_owned(),
                            return_type: None,
                            docs: Some("Run the environment test".to_owned()),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
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
        u32::try_from(text.find("test").expect("expected module function") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected host module function hover");

    assert_eq!(hover.signature, "fn test(int) -> int");
    assert_eq!(hover.docs.as_deref(), Some("Run the environment test"));
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}
#[test]
fn hover_supports_host_module_constants() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                import "env" as env;

                let value = env::DEFAULTS;
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "env".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Environment helpers".to_owned()),
                    functions: Default::default(),
                    constants: [(
                        "DEFAULTS".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "map<string, int>".to_owned(),
                            docs: Some("Default environment values".to_owned()),
                        },
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
    let offset = u32::try_from(text.find("DEFAULTS").expect("expected module constant") + 1)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected host module constant hover");

    assert_eq!(hover.signature, "const DEFAULTS: map<string, int>");
    assert_eq!(hover.docs.as_deref(), Some("Default environment values"));
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}

#[test]
fn hover_supports_global_module_constants() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// Answer docs
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("ANSWER").expect("expected global constant") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected global constant hover");

    assert_eq!(hover.signature, "const ANSWER: int");
    assert_eq!(hover.docs.as_deref(), Some("Answer docs"));
    assert_eq!(hover.source, HoverSignatureSource::Inferred);
}

#[test]
fn hover_supports_global_module_host_members() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn run() {
                    global::math::PI
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Math helpers".to_owned()),
                    functions: Default::default(),
                    constants: [(
                        "PI".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "float".to_owned(),
                            docs: Some("Circle ratio".to_owned()),
                        },
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
        u32::try_from(text.find("PI").expect("expected global host constant") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected global module host hover");

    assert_eq!(hover.signature, "const PI: float");
    assert_eq!(hover.docs.as_deref(), Some("Circle ratio"));
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}

#[test]
fn explicit_global_import_alias_overrides_automatic_global_module_hover() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    import "env" as global;

                    const ANSWER = 42;

                    fn run() {
                        global::DEFAULTS
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "env.rhai".into(),
                text: r#"
                    /// Imported defaults
                    export const DEFAULTS = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("DEFAULTS")
            .expect("expected imported global alias member")
            + 1,
    )
    .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected overridden global hover");

    assert_eq!(hover.signature, "const DEFAULTS: int");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
}
