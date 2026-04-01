use crate::tests::{
    assert_workspace_files_have_no_syntax_diagnostics, offset_in, symbol_id_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectReferenceKind};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::{DocumentVersion, normalize_path};
use std::path::Path;
use std::sync::Arc;

#[test]
fn stale_or_identical_file_changes_do_not_rebuild_analysis() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(2),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_parse = first_snapshot.parse(file_id).expect("expected parse");

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(2),
    ));
    let identical_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&identical_snapshot);
    let identical_parse = identical_snapshot.parse(file_id).expect("expected parse");
    assert!(Arc::ptr_eq(&first_parse, &identical_parse));

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 0;",
        DocumentVersion(1),
    ));
    let stale_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&stale_snapshot);
    let stale_parse = stale_snapshot.parse(file_id).expect("expected parse");
    assert!(Arc::ptr_eq(&first_parse, &stale_parse));
}

#[test]
fn workspace_symbol_search_supports_project_wide_queries() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "alpha.rhai".into(),
                text: r#"
                    fn helper() {}
                    let api_value = 1;
                    export api_value as public_helper;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "beta.rhai".into(),
                text: r#"
                    fn helper_tool() {}
                    fn Worker() { helper_tool(); }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let helper_matches = snapshot.workspace_symbols_matching("helper");
    assert_eq!(
        helper_matches
            .iter()
            .map(|symbol| (
                symbol.file_id,
                symbol.symbol.name.as_str(),
                symbol.symbol.exported
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "helper",
                true,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("beta.rhai"))
                    .expect("expected beta.rhai"),
                "helper_tool",
                true,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "public_helper",
                true,
            ),
        ]
    );

    let worker_matches = snapshot.workspace_symbols_matching("worker");
    assert_eq!(worker_matches.len(), 1);
    assert_eq!(worker_matches[0].symbol.name, "Worker");
}

#[test]
fn completion_inputs_collect_visible_member_and_project_symbols() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn run() {
                        let user = #{ name: "Ada", id: 42 };
                        let text = "Ada";
                        let local_value = 1;
                        user.
                        text.
                        helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn shared_helper() {}
                    fn project_only() {}
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let main = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let main_text = snapshot.file_text(main).expect("expected main text");

    let helper_offset = offset_in(&main_text, "helper();");
    let helper_inputs = snapshot
        .completion_inputs(main, helper_offset)
        .expect("expected completion inputs");
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "local_value")
    );
    assert!(
        helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "shared_helper")
    );
    assert!(
        !helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "helper")
    );

    let member_offset = offset_in(&main_text, "user.");
    let member_inputs = snapshot
        .completion_inputs(main, member_offset)
        .expect("expected member completion inputs");
    assert!(
        !member_inputs.member_symbols.is_empty(),
        "expected member completions for object literal fields"
    );
    assert!(
        member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "name")
            || member_inputs
                .member_symbols
                .iter()
                .any(|member| member.name == "id")
    );

    let string_member_offset =
        offset_in(&main_text, "text.") + rhai_syntax::TextSize::from("text.".len() as u32);
    let string_member_inputs = snapshot
        .completion_inputs(main, string_member_offset)
        .expect("expected string member completion inputs");
    assert!(
        string_member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "contains")
    );
    assert!(
        string_member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "len")
    );
}

#[test]
fn query_support_can_be_warmed_for_completion_and_navigation_queries() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}

            fn run() {
                let user = #{ name: "Ada", id: 42 };
                user.
                helper();
            }
        "#,
        DocumentVersion(1),
    ));

    let cold_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&cold_snapshot);
    let file_id = cold_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    assert!(cold_snapshot.query_support(file_id).is_none());

    assert_eq!(db.warm_query_support(&[file_id]), 1);
    let warm_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warm_snapshot);
    let query_support = warm_snapshot
        .query_support(file_id)
        .expect("expected warmed query support");
    assert!(
        query_support
            .completion_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );
    assert!(
        query_support
            .navigation_targets
            .iter()
            .any(|target| target.symbol.name == "helper")
    );
    assert!(
        query_support
            .member_completion_sets
            .iter()
            .any(|set| set.symbol.name == "user"
                && set.members.iter().any(|member| member.name == "name"))
    );
    assert_eq!(warm_snapshot.stats().query_support_rebuilds, 1);

    let main_text = warm_snapshot
        .file_text(file_id)
        .expect("expected main text");
    let completion_inputs = warm_snapshot
        .completion_inputs(file_id, offset_in(&main_text, "helper();"))
        .expect("expected completion inputs");
    assert!(
        completion_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );

    assert_eq!(db.warm_workspace_queries(), 0);
}

#[test]
fn builtin_global_functions_suppress_unresolved_name_diagnostics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}
            let _bytes = blob(10);
            let _now = timestamp();
            let _callback = Fn("helper");
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin blob call to avoid unresolved-name diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn comment_extern_directives_suppress_unresolved_names_and_seed_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            // rhai: extern injected_value: int
            let result = injected_value + 1;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `injected_value`"),
        "expected extern directive to suppress unresolved name, got {diagnostics:?}"
    );

    let hir = snapshot.hir(file_id).expect("expected hir");
    let result = symbol_id_by_name(hir.as_ref(), "result", SymbolKind::Variable);
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn comment_module_directives_seed_import_alias_members_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            // rhai: module env
            // rhai: extern env::test: fun(int) -> int
            // rhai: extern env::DEFAULTS: map<string, int>
            import "env" as env;

            let result = env::test(1);
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved import module `env`"),
        "expected inline module directive to suppress unresolved import, got {diagnostics:?}"
    );

    let completions = snapshot.imported_module_completions(file_id, &[String::from("env")]);
    let test_completion = completions
        .iter()
        .find(|completion| completion.name == "test")
        .expect("expected inline module completion for test");
    assert_eq!(
        test_completion.annotation,
        Some(TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );

    let hir = snapshot.hir(file_id).expect("expected hir");
    let result = symbol_id_by_name(hir.as_ref(), "result", SymbolKind::Variable);
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn caller_scope_captures_are_suppressed_when_function_has_no_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`"),
        "expected unresolved capture to stay hidden until normal calls exist, got {diagnostics:?}"
    );
}

#[test]
fn caller_scope_captures_are_suppressed_for_caller_scope_invocations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            let result = call!(helper);
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`"),
        "expected caller-scope call to suppress unresolved capture, got {diagnostics:?}"
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must use caller scope (`call!`)")
    }));
}

#[test]
fn caller_scope_exported_consts_are_suppressed_when_function_has_no_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                DEFAULTS
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `DEFAULTS`"),
        "expected uncalled function capture of exported const to stay hidden, got {diagnostics:?}"
    );
}

#[test]
fn caller_scope_captures_ignore_unrelated_same_name_calls_in_other_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    export const DEFAULTS = #{ name: "demo" };

                    fn make_config(root) {
                        #{
                            root: root,
                            defaults: DEFAULTS,
                        }
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "other.rhai".into(),
                text: r#"
                    fn make_config() {
                        1
                    }

                    make_config();
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let main = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(main);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `DEFAULTS`"),
        "expected unrelated same-name call sites to not trigger caller-scope diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn caller_scope_captures_report_function_and_regular_call_sites() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            let a = call!(helper);
            let b = helper();
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`"),
        "expected function-body unresolved capture diagnostic, got {diagnostics:?}"
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "call to `helper` must use caller scope (`call!`) because the function references outer-scope names"
    }));
}

#[test]
fn caller_scope_captures_report_regular_imported_calls_across_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
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
            FileChange {
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
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let imported_exports = snapshot
        .linked_imports(consumer)
        .iter()
        .flat_map(|linked| linked.exports.iter())
        .filter_map(|export| export.export.exported_name.clone())
        .collect::<Vec<_>>();
    assert!(
        imported_exports.iter().any(|name| name == "make_config"),
        "expected linked import to expose make_config, got {imported_exports:?}"
    );

    let provider_diagnostics = snapshot.project_diagnostics(provider);
    assert!(
        provider_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `defaults`"),
        "expected provider unresolved capture to stay visible once regular calls exist, got {provider_diagnostics:?}"
    );

    let consumer_diagnostics = snapshot.project_diagnostics(consumer);
    assert!(consumer_diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "call to `make_config` must use caller scope (`call!`) because the function references outer-scope names"
    }), "expected consumer caller-scope diagnostic, got {consumer_diagnostics:?}");
}

#[test]
fn imported_member_references_report_unresolved_after_provider_renames() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { 1 }
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
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
        project: Some(ProjectConfig::default()),
    });

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn renamed_helper() { 1 }
            export const RENAMED = 1;
        "#,
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    let diagnostics = snapshot.project_diagnostics(consumer);
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
fn static_path_imports_report_missing_workspace_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "./missing_module" as missing;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "import module `./missing_module` does not resolve to an existing workspace file"
    }));
}

#[test]
fn static_named_imports_report_unresolved_modules() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            import "env" as env;
            fn run() {}
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved import module `env`")
    );
}

#[test]
fn changing_exports_does_not_break_static_import_linkage() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as renamed_tools;
        "#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert_eq!(second_snapshot.linked_imports(consumer).len(), 1);
    assert!(second_snapshot.exports_named("shared_tools").is_empty());
    assert_eq!(second_snapshot.exports_named("renamed_tools").len(), 1);
}

#[test]
fn change_report_surfaces_dependency_affected_files_for_static_imports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "provider.rhai",
        "export const VALUE = 2;",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(impact.changed_files, vec![provider]);
    assert_eq!(impact.rebuilt_files, vec![provider]);
    assert_eq!(impact.dependency_affected_files, vec![consumer]);
}

#[test]
fn change_report_marks_dependencies_affected_when_importers_change() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "consumer.rhai",
        "import \"provider\" as tools;\nfn run() {}",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(impact.changed_files, vec![consumer]);
    assert_eq!(impact.rebuilt_files, vec![consumer]);
    assert_eq!(impact.dependency_affected_files, vec![provider]);
}

#[test]
fn project_find_references_for_exports_stay_local_to_the_exporting_file() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");

    let references = snapshot
        .find_references(provider, offset_in(&provider_text, "shared_tools"))
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "shared_tools");
    assert_eq!(
        references
            .references
            .iter()
            .map(|reference| (reference.file_id, reference.kind))
            .collect::<Vec<_>>(),
        vec![(provider, ProjectReferenceKind::Definition)]
    );
}

#[test]
fn auto_import_candidates_do_not_plan_symbol_imports_from_workspace_exports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let candidates =
        snapshot.auto_import_candidates(consumer, offset_in(&consumer_text, "shared_tools"));
    assert!(candidates.is_empty());
}

#[test]
fn goto_definition_on_import_module_reference_targets_provider_file() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        1
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let module_offset = offset_in(&consumer_text, "\"provider\"") + TextSize::from(1);

    let definitions = snapshot.goto_definition(consumer, module_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, provider);
}

#[test]
fn goto_definition_resolves_outer_scope_captures_inside_functions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                let config = #{
                    defaults: DEFAULTS,
                };
                config
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "defaults: DEFAULTS") + TextSize::from(10);
    let declaration_offset = offset_in(&text, "DEFAULTS =");
    let definitions = snapshot.goto_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0]
            .target
            .full_range
            .contains(declaration_offset),
        "expected goto definition to target outer captured const, got {definitions:?}"
    );
}

#[test]
fn find_references_on_import_alias_reports_current_file_usages() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        1
                    }

                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
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
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let alias_offset = offset_in(&consumer_text, "tools");

    let references = snapshot
        .find_references(consumer, alias_offset)
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "tools");
    assert_eq!(references.targets[0].file_id, consumer);
    assert_eq!(references.references.len(), 3);
    assert_eq!(
        references
            .references
            .iter()
            .map(|reference| (reference.file_id, reference.kind))
            .collect::<Vec<_>>(),
        vec![
            (consumer, ProjectReferenceKind::Definition),
            (consumer, ProjectReferenceKind::Reference),
            (consumer, ProjectReferenceKind::Reference),
        ]
    );
}

#[test]
fn find_references_on_imported_path_member_reaches_provider_symbol() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        1
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");

    let references = snapshot
        .find_references(provider, offset_in(&provider_text, "helper"))
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "helper");
    assert!(
        references
            .references
            .iter()
            .any(|reference| reference.file_id == consumer
                && reference.kind == ProjectReferenceKind::Reference),
        "expected imported path reference, got {:?}",
        references.references
    );
}

#[test]
fn find_references_include_outer_scope_captures_inside_functions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                let config = #{
                    defaults: DEFAULTS,
                };
                config
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let declaration_offset = offset_in(&text, "DEFAULTS =");
    let usage_offset = offset_in(&text, "defaults: DEFAULTS") + TextSize::from(10);
    let references = snapshot
        .find_references(file_id, declaration_offset)
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert!(references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(usage_offset)
    }));
}

#[test]
fn navigation_resolves_param_and_local_refs_inside_object_field_values() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn make_config(root, mode) {
                let workspace_name = workspace::name(root);
                let config = #{
                    mode: mode,
                    workspace: workspace_name,
                };
                config
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let mode_decl_offset = offset_in(&text, "root, mode") + TextSize::from(6);
    let mode_usage_offset = offset_in(&text, "mode: mode") + TextSize::from(7);
    let mode_definitions = snapshot.goto_definition(file_id, mode_usage_offset);
    assert_eq!(mode_definitions.len(), 1);
    assert!(
        mode_definitions[0]
            .target
            .full_range
            .contains(mode_decl_offset),
        "expected mode usage to target parameter definition, got {mode_definitions:?}"
    );

    let workspace_decl_offset = offset_in(&text, "workspace_name =");
    let workspace_usage_offset = offset_in(&text, "workspace: workspace_name") + TextSize::from(11);
    let workspace_definitions = snapshot.goto_definition(file_id, workspace_usage_offset);
    assert_eq!(workspace_definitions.len(), 1);
    assert!(
        workspace_definitions[0]
            .target
            .full_range
            .contains(workspace_decl_offset),
        "expected workspace_name usage to target local definition, got {workspace_definitions:?}"
    );

    let mode_references = snapshot
        .find_references(file_id, mode_decl_offset)
        .expect("expected mode references");
    assert!(mode_references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(mode_usage_offset)
    }));

    let workspace_references = snapshot
        .find_references(file_id, workspace_decl_offset)
        .expect("expected workspace_name references");
    assert!(workspace_references.references.iter().any(|reference| {
        reference.file_id == file_id
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(workspace_usage_offset)
    }));
}

#[test]
fn goto_definition_resolves_object_field_member_access_to_field_declaration() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{
                name: "demo",
                watch: true,
            };

            let value = DEFAULTS.name;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let declaration_offset = offset_in(&text, "name: \"demo\"");
    let usage_offset = offset_in(&text, "DEFAULTS.name") + TextSize::from(9);
    let definitions = snapshot.goto_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0]
            .target
            .full_range
            .contains(declaration_offset),
        "expected field member access to resolve to object field declaration, got {definitions:?}"
    );
}

#[test]
fn goto_type_definition_traces_structural_object_sources_through_symbol_flows() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let original = #{
                name: "demo",
                watch: true,
            };
            let alias = original;
            let current = alias;
            current.name
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "current.name") + TextSize::from(1);
    let literal_offset = offset_in(&text, "#{") + TextSize::from(1);
    let definitions = snapshot.goto_type_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, file_id);
    assert!(
        definitions[0].target.full_range.contains(literal_offset),
        "expected type definition to target structural object source, got {definitions:?}"
    );
}

#[test]
fn goto_type_definition_can_target_documented_symbol_annotations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int
            let answer = 1;
            answer
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let text = snapshot.file_text(file_id).expect("expected file text");

    let usage_offset = offset_in(&text, "answer\n") + TextSize::from(1);
    let docs_offset = offset_in(&text, "@type int") + TextSize::from(1);
    let definitions = snapshot.goto_type_definition(file_id, usage_offset);

    assert_eq!(definitions.len(), 1);
    assert!(
        definitions[0].target.full_range.contains(docs_offset),
        "expected type definition to target doc annotation block, got {definitions:?}"
    );
}

#[test]
fn find_references_for_object_field_declaration_include_cross_file_member_accesses() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    export const DEFAULTS = #{
                        name: "demo",
                        watch: true,
                    };
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    let value = tools::DEFAULTS.name;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let declaration_offset = offset_in(&provider_text, "name: \"demo\"");
    let usage_offset = offset_in(&consumer_text, "DEFAULTS.name") + TextSize::from(9);
    let references = snapshot
        .find_references(provider, declaration_offset)
        .expect("expected object field references");

    assert!(references.references.iter().any(|reference| {
        reference.file_id == provider && reference.kind == ProjectReferenceKind::Definition
    }));
    assert!(references.references.iter().any(|reference| {
        reference.file_id == consumer
            && reference.kind == ProjectReferenceKind::Reference
            && reference.range.contains(usage_offset)
    }));
}

#[test]
fn project_rename_plan_for_exports_does_not_include_module_import_paths() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");

    let export_plan = snapshot
        .rename_plan(
            provider,
            offset_in(&provider_text, "shared_tools"),
            "renamed_tools",
        )
        .expect("expected project rename plan");
    assert_eq!(export_plan.targets.len(), 1);
    assert_eq!(export_plan.targets[0].symbol.name, "shared_tools");
    assert_eq!(export_plan.occurrences.len(), 1);
    assert_eq!(export_plan.occurrences[0].file_id, provider);
    assert_eq!(
        export_plan.occurrences[0].kind,
        ProjectReferenceKind::Definition
    );
}

#[test]
fn snapshot_tracks_source_roots_workspace_membership_and_normalized_paths() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "workspace/src/./main.rhai".into(),
                text: "fn main() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/scripts/../scripts/tool.rhai".into(),
                text: "fn tool() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/tests/test.rhai".into(),
                text: "fn test_case() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            source_roots: vec!["src".into(), "scripts/../scripts".into()],
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let main = snapshot
        .vfs()
        .file_id(Path::new("workspace/src/main.rhai"))
        .expect("expected main.rhai");
    let tool = snapshot
        .vfs()
        .file_id(Path::new("workspace/scripts/tool.rhai"))
        .expect("expected tool.rhai");
    let test = snapshot
        .vfs()
        .file_id(Path::new("workspace/tests/test.rhai"))
        .expect("expected test.rhai");

    assert_eq!(
        snapshot.source_root_paths(),
        vec![
            normalize_path(Path::new("workspace/scripts")),
            normalize_path(Path::new("workspace/src")),
        ]
    );
    assert_eq!(
        snapshot.normalized_path(main),
        Some(normalize_path(Path::new("workspace/src/main.rhai")).as_path())
    );
    assert_eq!(
        snapshot.normalized_path(tool),
        Some(normalize_path(Path::new("workspace/scripts/tool.rhai")).as_path())
    );
    assert_eq!(
        snapshot.normalized_path(test),
        Some(normalize_path(Path::new("workspace/tests/test.rhai")).as_path())
    );
    assert!(snapshot.is_workspace_file(main));
    assert!(snapshot.is_workspace_file(tool));
    assert!(!snapshot.is_workspace_file(test));

    let workspace_files = snapshot.workspace_files();
    assert_eq!(workspace_files.len(), 3);
    assert!(workspace_files.iter().any(|file| {
        file.file_id == main
            && file.source_root == snapshot.source_root_index(main)
            && file.is_workspace_file
    }));
    assert!(workspace_files.iter().any(|file| {
        file.file_id == test && file.source_root.is_none() && !file.is_workspace_file
    }));
}

#[test]
fn analysis_dependencies_track_text_and_project_inputs() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "src/./main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("src/main.rhai"))
        .expect("expected file id");
    let first_dependencies = first_snapshot
        .analysis_dependencies(file_id)
        .expect("expected analysis dependencies");

    assert_eq!(
        first_dependencies.parse.normalized_path,
        normalize_path(Path::new("src/main.rhai"))
    );
    assert_eq!(
        first_dependencies.parse.document_version,
        DocumentVersion(1)
    );
    assert_eq!(first_dependencies.hir.project_revision, 0);
    assert_eq!(
        first_dependencies.last_invalidation,
        crate::InvalidationReason::InitialLoad
    );

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            ..ProjectConfig::default()
        }),
    });

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_dependencies = second_snapshot
        .analysis_dependencies(file_id)
        .expect("expected analysis dependencies");

    assert_eq!(second_snapshot.project_revision(), 1);
    assert_eq!(second_dependencies.hir.project_revision, 1);
    assert_eq!(second_dependencies.index.project_revision, 1);
    assert_eq!(
        second_dependencies.last_invalidation,
        crate::InvalidationReason::ProjectChanged
    );
}

#[test]
fn removing_files_unloads_cached_analysis_and_updates_workspace_links() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let provider = first_snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);

    db.apply_change(ChangeSet::remove_file("provider.rhai"));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(second_snapshot.file_text(provider).is_none());
    assert!(second_snapshot.parse(provider).is_none());
    assert!(second_snapshot.hir(provider).is_none());
    assert!(second_snapshot.module_graph(provider).is_none());
    assert!(second_snapshot.linked_imports(consumer).is_empty());
}

#[test]
fn revision_stats_and_debug_view_surface_cache_activity() {
    let mut db = AnalyzerDatabase::default();
    let initial_revision = db.snapshot().revision();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));
    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    assert_eq!(first_snapshot.revision(), initial_revision + 1);
    assert_eq!(first_snapshot.stats().parse_rebuilds, 1);
    assert_eq!(first_snapshot.stats().lower_rebuilds, 1);
    assert_eq!(first_snapshot.stats().index_rebuilds, 1);

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));
    let no_op_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&no_op_snapshot);
    assert_eq!(no_op_snapshot.revision(), first_snapshot.revision());

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 2;",
        DocumentVersion(2),
    ));
    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert_eq!(second_snapshot.revision(), first_snapshot.revision() + 1);

    let file_id = second_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    assert_eq!(db.warm_query_support(&[file_id]), 1);
    let warmed_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warmed_snapshot);
    assert_eq!(warmed_snapshot.stats().query_support_rebuilds, 1);

    let debug_view = warmed_snapshot.debug_view();
    assert_eq!(debug_view.revision, warmed_snapshot.revision());
    assert_eq!(debug_view.files.len(), 1);
    assert_eq!(
        debug_view.files[0].normalized_path,
        normalize_path(Path::new("main.rhai"))
    );
    assert_eq!(debug_view.files[0].document_version, DocumentVersion(2));
    assert_eq!(debug_view.files[0].stats.file_id, file_id);
    assert_eq!(debug_view.files[0].stats.query_support_rebuilds, 1);
    assert!(debug_view.stats.total_parse_time >= std::time::Duration::ZERO);
    assert!(debug_view.stats.total_query_support_time >= std::time::Duration::ZERO);
}

#[test]
fn batched_high_frequency_updates_rebuild_each_file_once() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "workspace/src/./main.rhai".into(),
                text: "let value = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/src/main.rhai".into(),
                text: "let value = 2;".to_owned(),
                version: DocumentVersion(2),
            },
            FileChange {
                path: "workspace/src/main.rhai".into(),
                text: "let value = 3;".to_owned(),
                version: DocumentVersion(3),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("workspace/src/main.rhai"))
        .expect("expected file id");
    assert_eq!(
        snapshot.file_text(file_id).as_deref(),
        Some("let value = 3;")
    );
    assert_eq!(snapshot.stats().parse_rebuilds, 1);
    assert_eq!(snapshot.stats().lower_rebuilds, 1);
    assert_eq!(snapshot.stats().index_rebuilds, 1);
}

#[test]
fn query_support_budget_evicts_cold_cached_files_and_updates_file_stats() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn one() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "fn two() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let one = snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");

    assert_eq!(
        db.set_query_support_budget(Some(1)),
        Vec::<rhai_vfs::FileId>::new()
    );
    assert_eq!(db.warm_query_support(&[one]), 1);
    assert_eq!(db.warm_query_support(&[two]), 1);

    let warmed_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warmed_snapshot);
    assert!(warmed_snapshot.query_support(one).is_none());
    assert!(warmed_snapshot.query_support(two).is_some());
    assert_eq!(warmed_snapshot.stats().query_support_evictions, 1);
    assert_eq!(
        warmed_snapshot
            .file_stats(one)
            .expect("expected one.rhai stats")
            .query_support_evictions,
        1
    );
    assert!(
        !warmed_snapshot
            .file_stats(one)
            .expect("expected one.rhai stats")
            .query_support_cached
    );
    assert!(
        warmed_snapshot
            .file_stats(two)
            .expect("expected two.rhai stats")
            .query_support_cached
    );
}
