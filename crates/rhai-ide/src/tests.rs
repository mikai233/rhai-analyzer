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
            fn outer() {
                fn inner() {}
            }

            const LIMIT = 1;
            export outer as public_outer;
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
        vec!["outer", "LIMIT", "public_outer"]
    );
    assert_eq!(document_symbols[0].children.len(), 1);
    assert_eq!(document_symbols[0].children[0].name, "inner");
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
fn goto_definition_uses_cross_file_database_navigation() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
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
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);
    let offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let targets = analysis.goto_definition(FilePosition {
        file_id: consumer,
        offset,
    });

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].file_id, provider);
}

#[test]
fn references_and_rename_plan_surface_project_level_results() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() { helper(); }
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
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
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);
    let consumer_offset = u32::try_from(
        analysis
            .db
            .file_text(consumer)
            .expect("expected consumer text")
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let references = analysis
        .find_references(FilePosition {
            file_id: consumer,
            offset: consumer_offset,
        })
        .expect("expected references result");
    assert_eq!(references.targets.len(), 1);
    assert!(
        references
            .references
            .iter()
            .any(|reference| reference.kind == ReferenceKind::LinkedImport)
    );

    let rename = analysis
        .rename_plan(
            FilePosition {
                file_id: consumer,
                offset: consumer_offset,
            },
            "renamed_tools",
        )
        .expect("expected rename plan");
    assert_eq!(rename.new_name, "renamed_tools");
    assert!(
        rename
            .occurrences
            .iter()
            .any(|occurrence| occurrence.kind == ReferenceKind::LinkedImport)
    );
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
fn diagnostics_respect_workspace_linked_import_resolution() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;\n\nfn run() { tools(); }".to_owned(),
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
    assert!(analysis.diagnostics(consumer).is_empty());

    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        "fn helper() {} export helper as renamed_tools;",
        DocumentVersion(2),
    ));

    let analysis = host.snapshot();
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);
    let diagnostics = analysis.diagnostics(consumer);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved import module `shared_tools`")
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("import alias no longer resolves")
    }));
}

#[test]
fn auto_import_actions_are_returned_as_source_changes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
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

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].module_name, "shared_tools");
    assert_eq!(actions[0].source_change.file_edits.len(), 1);
    assert_eq!(actions[0].source_change.file_edits[0].file_id, consumer);
    assert_eq!(actions[0].source_change.file_edits[0].edits.len(), 1);
    assert_eq!(
        actions[0].source_change.file_edits[0].edits[0].insertion_offset(),
        Some(0)
    );
    assert_eq!(
        actions[0].source_change.file_edits[0].edits[0].new_text,
        "import shared_tools as shared_tools;\n"
    );
}

#[test]
fn assists_return_auto_import_quick_fixes_for_unresolved_names() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
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

    assert_eq!(assists.len(), 1);
    assert_eq!(assists[0].id.as_str(), "import.auto");
    assert_eq!(assists[0].kind, AssistKind::QuickFix);
    assert_eq!(assists[0].label, "Import `shared_tools`");
}

#[test]
fn diagnostics_with_fixes_attach_auto_import_quick_fixes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
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

    assert_eq!(unresolved.fixes.len(), 1);
    assert_eq!(unresolved.fixes[0].id.as_str(), "import.auto");
    assert_eq!(unresolved.fixes[0].label, "Import `shared_tools`");
}

#[test]
fn diagnostics_with_fixes_attach_remove_import_fix_for_broken_workspace_links() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;\n\nfn run() { tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });
    host.apply_change(ChangeSet::single_file(
        "provider.rhai",
        "fn helper() {} export helper as renamed_tools;",
        DocumentVersion(2),
    ));

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);

    let diagnostics = analysis.diagnostics_with_fixes(consumer);
    let broken = diagnostics
        .iter()
        .find(|entry| {
            entry
                .diagnostic
                .message
                .contains("import alias no longer resolves")
        })
        .expect("expected broken import diagnostic");

    assert_eq!(broken.fixes.len(), 1);
    assert_eq!(broken.fixes[0].id.as_str(), "import.remove_broken");
    assert_eq!(broken.fixes[0].kind, AssistKind::QuickFix);
    assert_eq!(broken.fixes[0].label, "Remove broken import");
    assert_eq!(broken.fixes[0].source_change.file_edits.len(), 1);
    assert_eq!(broken.fixes[0].source_change.file_edits[0].edits.len(), 1);
    assert_eq!(
        broken.fixes[0].source_change.file_edits[0].edits[0].new_text,
        ""
    );
}

#[test]
fn rename_materializes_cross_file_source_changes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
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
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "renamed_tools",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_edits.len(), 2);
    let provider_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == provider)
        .expect("expected provider edits");
    let consumer_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == consumer)
        .expect("expected consumer edits");

    assert_eq!(provider_edits.edits.len(), 1);
    assert_eq!(consumer_edits.edits.len(), 1);
    assert_eq!(provider_edits.edits[0].new_text, "renamed_tools");
    assert_eq!(consumer_edits.edits[0].new_text, "renamed_tools");
}

#[test]
fn rename_with_conflicts_reports_issues_without_materializing_edits() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "existing.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as renamed_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
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
    assert_no_syntax_diagnostics(&analysis, consumer);
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("shared_tools")
            .expect("expected shared_tools"),
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
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
        "import shared_tools as tools;\nfn run() {}",
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
        "import used_tools as used;\nimport unused_tools as unused;\nfn run() { used(); }",
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
        "import zebra as zebra;\nimport alpha as alpha;\nimport zebra as zebra;\nfn run() {}",
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
        "import alpha as alpha;\nimport zebra as zebra;"
    );
}

#[test]
fn assists_include_source_import_cleanup_actions_for_import_blocks() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import zebra as zebra;\nimport alpha as alpha;\nimport zebra as zebra;\nfn run() {}",
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
fn signature_help_returns_cross_file_exported_function_signature() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// helper docs
                    /// @param left int
                    /// @param right string
                    /// @return bool
                    fn helper(left, right) {
                        true
                    }

                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

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
    let provider = analysis
        .db
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    assert_no_syntax_diagnostics(&analysis, provider);
    assert_no_syntax_diagnostics(&analysis, consumer);
    let text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition {
            file_id: consumer,
            offset,
        })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn helper(left: int, right: string) -> bool"
    );
    assert_eq!(help.signatures[0].file_id, Some(provider));
    assert!(
        help.signatures[0]
            .docs
            .as_deref()
            .is_some_and(|docs| docs.contains("helper docs"))
    );
    assert_eq!(help.signatures[0].parameters[0].label, "left");
    assert_eq!(help.signatures[0].parameters[1].label, "right");
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
fn signature_help_returns_host_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;

                fn run() {
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
                /// @type Widget
                let widget = unknown_widget;

                fn run() {
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
