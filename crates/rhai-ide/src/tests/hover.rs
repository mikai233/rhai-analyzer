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

#[test]
fn hover_reports_inferred_types_for_object_field_member_accesses() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let defaults = #{
                name: "demo",
                watch: true,
            };

            let value = defaults.name;
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
    let offset = u32::try_from(text.find("defaults.name").expect("expected field usage") + 9)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.source, HoverSignatureSource::Inferred);
    assert!(hover.declared_signature.is_none());
    assert_eq!(
        hover.inferred_signature.as_deref(),
        Some("field name: string")
    );
    assert!(hover.notes.iter().any(|note| {
        note == "Field type is inferred from structural object flows and object literal analysis."
    }));
}

#[test]
fn hover_prefers_documented_object_field_annotations() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @field name string
            let user = #{
                name: "Ada",
            };

            let value = user.name;
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
        u32::try_from(text.find("user.name").expect("expected field usage") + 5).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected documented field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.source, HoverSignatureSource::Declared);
    assert_eq!(
        hover.declared_signature.as_deref(),
        Some("field name: string")
    );
    assert_eq!(
        hover.inferred_signature.as_deref(),
        Some("field name: string")
    );
}

#[test]
fn hover_surfaces_documented_object_field_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @field name string Primary display name
            let user = #{
                name: "Ada",
            };

            let value = user.name;
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
        u32::try_from(text.find("user.name").expect("expected field usage") + 5).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected documented field hover");

    assert_eq!(hover.signature, "field name: string");
    assert_eq!(hover.docs.as_deref(), Some("Primary display name"));
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
fn hover_supports_host_module_import_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                import "env" as env;

                env::test(1);
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
    let offset =
        u32::try_from(text.find("env;").expect("expected import alias") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected import alias hover");

    assert_eq!(hover.signature, r#"import "env" as env"#);
    assert_eq!(hover.docs.as_deref(), Some("Environment helpers"));
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Resolved from host module metadata.")
    );
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Module exposes 2 members.")
    );
}

#[test]
fn hover_supports_workspace_import_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper(value) { value }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    tools::helper(1);
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("tools;").expect("expected import alias") + 1).expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected workspace import alias hover");

    assert_eq!(hover.signature, r#"import "provider" as tools"#);
    assert!(hover.docs.is_none());
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note.contains("provider.rhai"))
    );
}

#[test]
fn hover_supports_export_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let exported_value = 1;

            /// Public API alias
            export exported_value as public_value;
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
    let offset = u32::try_from(text.find("public_value").expect("expected export alias") + 1)
        .expect("offset");

    let hover = analysis
        .hover(FilePosition { file_id, offset })
        .expect("expected export alias hover");

    assert_eq!(hover.signature, "export exported_value as public_value");
    assert_eq!(hover.docs.as_deref(), Some("Public API alias"));
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Re-exports: let exported_value")
    );
    assert!(
        hover
            .notes
            .iter()
            .any(|note| note == "Export alias is visible to importing modules.")
    );
}
