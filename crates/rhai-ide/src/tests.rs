use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, AssistKind, CompletionItemSource, FilePosition, ReferenceKind};

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
fn diagnostics_report_function_access_to_external_scope() {
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
        analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `value`")
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
fn references_and_rename_plan_for_exports_stay_local_to_definition_file() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as shared_tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let references = analysis
        .find_references(FilePosition {
            file_id: provider,
            offset,
        })
        .expect("expected references result");
    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.references.len(), 1);
    assert_eq!(references.references[0].kind, ReferenceKind::Definition);

    let rename = analysis
        .rename_plan(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected rename plan");
    assert_eq!(rename.new_name, "renamed_tools");
    assert_eq!(rename.occurrences.len(), 1);
    assert_eq!(rename.occurrences[0].kind, ReferenceKind::Definition);
}

#[test]
fn completions_merge_visible_project_and_member_results() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    /// helper docs
                    /// @type fun() -> bool
                    fn helper() {}

                    fn run() {
                        let user = #{ name: "Ada" };
                        user.
                        helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: "fn shared_helper() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let main = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let support = analysis
        .db
        .vfs()
        .file_id(Path::new("support.rhai"))
        .expect("expected support.rhai");
    assert_no_syntax_diagnostics(&analysis, main);
    assert_no_syntax_diagnostics(&analysis, support);
    let main_text = analysis.db.file_text(main).expect("expected main text");

    let helper_offset = u32::try_from(main_text.find("helper();").expect("expected helper call"))
        .expect("expected offset to fit");
    let helper_completions = analysis.completions(FilePosition {
        file_id: main,
        offset: helper_offset,
    });
    assert!(
        helper_completions
            .iter()
            .any(|item| { item.label == "helper" && item.source == CompletionItemSource::Visible })
    );
    assert!(helper_completions.iter().any(|item| {
        item.label == "shared_helper" && item.source == CompletionItemSource::Project
    }));

    let member_offset = u32::try_from(main_text.find("user.").expect("expected member access"))
        .expect("expected offset to fit");
    let member_completions = analysis.completions(FilePosition {
        file_id: main,
        offset: member_offset,
    });
    assert!(
        member_completions
            .iter()
            .any(|item| { item.label == "name" && item.source == CompletionItemSource::Member })
    );
}

#[test]
fn completions_fall_back_to_inferred_local_symbol_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                value
            }

            fn run() {
                let result = echo(blob(10));
                res
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
        u32::try_from(text.rfind("res").expect("expected completion target")).expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let result = completions
        .iter()
        .find(|item| item.label == "result" && item.source == CompletionItemSource::Visible)
        .expect("expected result completion");
    let echo = completions
        .iter()
        .find(|item| item.label == "echo" && item.source == CompletionItemSource::Visible)
        .expect("expected echo completion");

    assert_eq!(result.detail.as_deref(), Some("blob"));
    assert_eq!(echo.detail.as_deref(), Some("fun(blob) -> blob"));
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
fn auto_import_actions_are_not_returned_for_workspace_exports() {
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
    let offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected unresolved reference"),
    )
    .expect("expected offset to fit");

    let actions = analysis.auto_import_actions(FilePosition {
        file_id: consumer,
        offset,
    });

    assert!(actions.is_empty());
}

#[test]
fn assists_do_not_offer_auto_import_quick_fixes_for_workspace_exports() {
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
    let offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected unresolved reference"),
    )
    .expect("expected offset to fit");

    let assists = analysis.assists(
        consumer,
        TextRange::new(TextSize::from(offset), TextSize::from(offset)),
    );

    assert!(assists.is_empty());
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
fn rename_materializes_export_alias_source_changes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            let helper = 1;
            export helper as shared_tools;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_edits.len(), 1);
    let provider_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == provider)
        .expect("expected provider edits");

    assert_eq!(provider_edits.edits.len(), 1);
    assert_eq!(provider_edits.edits[0].new_text, "renamed_tools");
}

#[test]
fn rename_with_export_conflicts_reports_issues_without_materializing_edits() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "existing.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper as renamed_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let existing = analysis
        .db
        .vfs()
        .file_id(Path::new("existing.rhai"))
        .expect("expected existing.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, existing);
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: provider,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected prepared rename");

    assert!(!rename.plan.issues.is_empty());
    assert!(rename.source_change.is_none());
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
}

#[test]
fn remove_unused_imports_plans_deletions_for_unused_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"used_tools\" as used;\nimport \"unused_tools\" as unused;\nused;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .remove_unused_imports(file_id)
        .expect("expected unused import cleanup");

    assert_eq!(change.file_edits.len(), 1);
    assert_eq!(change.file_edits[0].file_id, file_id);
    assert_eq!(change.file_edits[0].edits.len(), 1);
    assert_eq!(change.file_edits[0].edits[0].new_text, "");
}

#[test]
fn organize_imports_sorts_deduplicates_and_normalizes_import_block() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"zebra\" as zebra;\nimport \"alpha\" as alpha;\nimport \"zebra\" as zebra;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let change = analysis
        .organize_imports(file_id)
        .expect("expected organize imports change");

    assert_eq!(change.file_edits.len(), 1);
    assert_eq!(change.file_edits[0].edits.len(), 1);
    assert_eq!(
        change.file_edits[0].edits[0].new_text,
        "import \"alpha\" as alpha;\nimport \"zebra\" as zebra;"
    );
}

#[test]
fn assists_include_source_import_cleanup_actions_for_import_blocks() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"zebra\" as zebra;\nimport \"alpha\" as alpha;\nimport \"zebra\" as zebra;\nfn run() {}",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    let assists = analysis.assists(
        file_id,
        TextRange::new(TextSize::from(0), TextSize::from(0)),
    );

    assert!(assists.iter().any(|assist| {
        assist.id.as_str() == "import.organize"
            && assist.kind == AssistKind::Source
            && assist.label == "Organize imports"
    }));
}

#[test]
fn signature_help_returns_local_function_signature() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param left int
            /// @param right string
            /// @return bool
            fn check(left, right) {
                true
            }

            fn run() {
                check(1, value);
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
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn check(left: int, right: string) -> bool"
    );
    assert_eq!(help.signatures[0].file_id, Some(file_id));
    assert_eq!(help.signatures[0].parameters.len(), 2);
    assert_eq!(help.signatures[0].parameters[0].label, "left");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
    assert_eq!(help.signatures[0].parameters[1].label, "right");
    assert_eq!(
        help.signatures[0].parameters[1].annotation.as_deref(),
        Some("string")
    );
}

#[test]
fn signature_help_prefers_typed_script_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            /// @param delta string
            /// @return string
            fn bump(delta) {
                delta
            }

            fn run() {
                let value = 1;
                value.bump(amount);
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
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected method signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn bump(delta: int) -> int");
    assert_eq!(help.signatures[0].file_id, Some(file_id));
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "delta");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}

#[test]
fn signature_help_supports_imported_global_typed_methods() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @param delta int
                    /// @return int
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
                        let value = 1;
                        value.bump(amount);
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
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected imported method signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn bump(delta: int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "delta");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
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

#[test]
fn signature_help_supports_module_qualified_imported_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @param value int
                    /// @return int
                    fn helper(value) {
                        value
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
                        tools::helper(amount);
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
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected imported module function signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "value");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}

#[test]
fn signature_help_supports_nested_module_qualified_imported_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "sub.rhai".into(),
                text: r#"
                    /// @param value int
                    /// @return int
                    fn helper(value) {
                        value
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    import "sub" as sub;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::sub::helper(amount);
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
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected nested imported module function signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: int) -> int");
}

#[test]
fn signature_help_uses_caller_scope_call_targets() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value string
            fn helper(value) {
                value
            }

            fn run() {
                call!(helper, "home");
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
    let helper_offset =
        u32::try_from(text.find("helper,").expect("expected dispatch target")).expect("offset");
    let arg_offset = u32::try_from(
        text.find("\"home\"")
            .expect("expected caller-scope argument"),
    )
    .expect("offset");

    assert!(
        analysis
            .signature_help(FilePosition {
                file_id,
                offset: helper_offset,
            })
            .is_none()
    );

    let help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: arg_offset,
        })
        .expect("expected caller-scope signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: string)");
}

#[test]
fn signature_help_is_not_returned_for_imported_module_alias_invocations() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools(1, value);
                    }
                "#
                .to_owned(),
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
    let text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    assert!(
        analysis
            .signature_help(FilePosition {
                file_id: consumer,
                offset,
            })
            .is_none()
    );
}

#[test]
fn signature_help_returns_builtin_blob_overloads() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let _empty = blob();
                let _sized = blob(10);
                let _filled = blob(50, 42);
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin blob call to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("42").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.active_signature, 2);
    assert_eq!(help.signatures.len(), 3);
    assert_eq!(help.signatures[0].label, "fn blob() -> blob");
    assert_eq!(help.signatures[1].label, "fn blob(int) -> blob");
    assert_eq!(help.signatures[2].label, "fn blob(int, int) -> blob");
}

#[test]
fn signature_help_returns_builtin_timestamp_and_fn_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}

            fn run() {
                let _now = timestamp();
                let _callback = Fn("helper");
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin timestamp/Fn calls to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");

    let timestamp_offset =
        u32::try_from(text.find("timestamp").expect("expected timestamp call")).expect("offset");
    let timestamp_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: timestamp_offset,
        })
        .expect("expected timestamp signature help");
    assert_eq!(timestamp_help.signatures.len(), 1);
    assert_eq!(
        timestamp_help.signatures[0].label,
        "fn timestamp() -> timestamp"
    );

    let fn_offset =
        u32::try_from(text.find("\"helper\"").expect("expected Fn argument")).expect("offset");
    let fn_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: fn_offset,
        })
        .expect("expected Fn signature help");
    assert_eq!(fn_help.active_parameter, 0);
    assert_eq!(fn_help.signatures.len(), 1);
    assert_eq!(fn_help.signatures[0].label, "fn Fn(string) -> Fn");
}

#[test]
fn signature_help_returns_builtin_introspection_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn int.bump(delta) { this + delta }

            fn run(value) {
                let _a = is_def_var("value");
                let _b = is_def_fn("int", "bump", 1);
                let _c = value.type_of();
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin introspection calls to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");

    let is_def_fn_offset =
        u32::try_from(text.find("\"bump\", 1").expect("expected is_def_fn arg") + 8)
            .expect("offset");
    let is_def_fn_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: is_def_fn_offset,
        })
        .expect("expected is_def_fn signature help");
    assert_eq!(is_def_fn_help.active_parameter, 2);
    assert_eq!(is_def_fn_help.signatures.len(), 2);
    assert_eq!(
        is_def_fn_help.signatures[1].label,
        "fn is_def_fn(string, string, int) -> bool"
    );

    let type_of_offset =
        u32::try_from(text.find("type_of").expect("expected type_of call")).expect("offset");
    let type_of_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: type_of_offset,
        })
        .expect("expected type_of signature help");
    assert_eq!(type_of_help.signatures.len(), 1);
    assert_eq!(type_of_help.signatures[0].label, "fn type_of() -> string");
}

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

fn assert_no_syntax_diagnostics(analysis: &crate::Analysis, file_id: rhai_vfs::FileId) {
    let diagnostics = analysis.db.syntax_diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected no syntax diagnostics for {:?}, got {:?}",
        file_id,
        diagnostics
    );
}
