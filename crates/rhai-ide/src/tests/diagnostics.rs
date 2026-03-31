use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, DiagnosticSeverity, DiagnosticTag};

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

#[test]
fn diagnostics_report_unresolved_non_string_import_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        "import shared_tools as tools;",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(
        analysis
            .diagnostics(consumer)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved import module `shared_tools`")
    );
}

#[test]
fn diagnostics_report_non_string_import_module_expressions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        r#"
            fn helper() {}
            import helper as tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(analysis.diagnostics(consumer).iter().any(|diagnostic| {
        diagnostic.message
            == "import module expression `helper` must evaluate to string, found function"
    }));
}

#[test]
fn diagnostics_report_nested_function_definitions_as_syntax_errors() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn outer() {
                fn inner() {}
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

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.message == "functions can only be defined at global level"
    }));
}

#[test]
fn diagnostics_report_missing_semicolon_between_statements() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let v = "hello"
            let q = 1.0 + 2;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.message == "expected `;` to terminate statement"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
}

#[test]
fn diagnostics_report_function_access_to_external_scope() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            helper();
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

    assert!(
        analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`")
    );
    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.message
            == "call to `helper` must use caller scope (`call!`) because the function references outer-scope names"
    }));
}

#[test]
fn diagnostics_allow_function_access_to_external_scope_for_caller_scope_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            call!(helper);
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

    assert!(
        !analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`")
    );
    assert!(!analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must use caller scope (`call!`)")
    }));
}

#[test]
fn diagnostics_allow_function_access_to_external_scope_when_uncalled() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
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

    assert!(
        !analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`")
    );
}

#[test]
fn diagnostics_allow_exported_const_access_inside_uncalled_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                DEFAULTS
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

    assert!(
        !analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `DEFAULTS`")
    );
}

#[test]
fn diagnostics_report_caller_scope_violations_for_imported_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let defaults = #{ name: "demo" };
                    fn make_config() {
                        defaults
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::make_config();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(analysis.diagnostics(consumer).iter().any(|diagnostic| {
        diagnostic.message
            == "call to `make_config` must use caller scope (`call!`) because the function references outer-scope names"
    }), "expected caller-scope diagnostic in importing module, got {:?}", analysis.diagnostics(consumer));
}

#[test]
fn diagnostics_report_unresolved_import_members_after_provider_renames() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { 1 }
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                    fn run() {
                        tools::helper();
                        let value = tools::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn renamed_helper() { 1 }
            export const RENAMED = 1;
        "#,
        DocumentVersion(2),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics(consumer);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message == "unresolved import member `tools::helper`" })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message == "unresolved import member `tools::VALUE`" })
    );
}

#[test]
fn diagnostics_report_missing_static_path_import_files() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "./missing_module" as missing;
            fn run() {}
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

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.message
            == "import module `./missing_module` does not resolve to an existing workspace file"
    }));
}

#[test]
fn diagnostics_report_unresolved_static_named_import_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "env" as env;
            fn run() {}
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

    assert!(
        analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| { diagnostic.message == "unresolved import module `env`" })
    );
}

#[test]
fn diagnostics_allow_global_import_aliases_inside_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "hello" as hey;

            fn helper(value) {
                hey::process(value);
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

    assert!(
        !analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| { diagnostic.message == "unresolved name `hey`" })
    );
}

#[test]
fn diagnostics_keep_unresolved_non_string_import_modules_visible() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        "import shared_tools as tools;\n\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(
        analysis
            .diagnostics(consumer)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved import module `shared_tools`")
    );
}

#[test]
fn diagnostics_with_fixes_do_not_attach_workspace_export_auto_imports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics_with_fixes(consumer);
    let unresolved = diagnostics
        .iter()
        .find(|entry| entry.diagnostic.message == "unresolved name `shared_tools`")
        .expect("expected unresolved name diagnostic");

    assert!(unresolved.fixes.is_empty());
}

#[test]
fn diagnostics_with_fixes_attach_remove_unused_import_fix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"shared_tools\" as tools;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let diagnostics = analysis.diagnostics_with_fixes(file_id);
    let unused = diagnostics
        .iter()
        .find(|entry| entry.diagnostic.message == "unused symbol `tools`")
        .expect("expected unused import diagnostic");

    assert_eq!(unused.fixes.len(), 1);
    assert_eq!(unused.fixes[0].id.as_str(), "import.remove_unused");
    assert_eq!(unused.fixes[0].label, "Remove unused import");
    assert_eq!(unused.diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(unused.diagnostic.tags, vec![DiagnosticTag::Unnecessary]);
}

#[test]
fn diagnostics_keep_errors_for_hard_failures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            helper();
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

    let unresolved = analysis
        .diagnostics(file_id)
        .into_iter()
        .find(|diagnostic| diagnostic.message == "unresolved name `value`")
        .expect("expected unresolved name diagnostic");

    assert_eq!(unresolved.severity, DiagnosticSeverity::Error);
    assert!(unresolved.tags.is_empty());
}

#[test]
fn diagnostics_keep_unaliased_import_regular_members_unresolved() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    export const VALUE = 1;

                    fn helper(value) {
                        value
                    }

                    fn int.bump(delta) {
                        this + delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider";

                    fn run() {
                        let _a = helper(1);
                        let _b = VALUE;
                        let _c = 1.bump(2);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics(consumer);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `helper`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `VALUE`")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("bump"))
    );
}
