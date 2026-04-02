use std::path::Path;

use rhai_db::ChangeSet;
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, AssistKind, FilePosition};
#[test]
fn auto_import_actions_insert_module_imports_and_qualify_workspace_exports() {
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

    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(action.module_name, "provider");
    assert_eq!(action.source_change.file_edits.len(), 1);
    assert_eq!(action.source_change.file_edits[0].edits.len(), 2);
    assert_eq!(
        action.source_change.file_edits[0].edits[0].new_text,
        "provider::shared_tools"
    );
    assert_eq!(
        action.source_change.file_edits[0].edits[1].new_text,
        "import \"provider\" as provider;\n\n"
    );
}

#[test]
fn assists_offer_auto_import_quick_fixes_for_workspace_exports() {
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

    let assist = assists
        .iter()
        .find(|assist| assist.id.as_str() == "import.auto")
        .expect("expected auto import assist");
    assert_eq!(assist.label, "Import `provider`");
    assert_eq!(assist.source_change.file_edits.len(), 1);
    assert_eq!(assist.source_change.file_edits[0].edits.len(), 2);
    assert_eq!(
        assist.source_change.file_edits[0].edits[0].new_text,
        "provider::shared_tools"
    );
}

#[test]
fn auto_import_actions_reuse_existing_import_aliases_without_duplicate_imports() {
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
                text: r#"
                    import "provider" as tools;

                    fn run() { shared_tools(); }
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
    assert_eq!(actions[0].label, "Qualify with `tools`");
    let edits = &actions[0].source_change.file_edits[0].edits;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "tools::shared_tools");
}

#[test]
fn auto_import_actions_allocate_non_conflicting_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "tools.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "tools" as provider;

                    fn run() { shared_tools(); }
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
    let edits = &actions[0].source_change.file_edits[0].edits;
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].new_text, "provider_1::shared_tools");
    assert_eq!(edits[1].new_text, "\nimport \"provider\" as provider_1;");
}

#[test]
fn auto_import_actions_support_unresolved_module_path_members() {
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
                text: "fn run() { provider::shared_tools(); }".to_owned(),
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
    let edits = &actions[0].source_change.file_edits[0].edits;
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].new_text, "shared_tools");
    assert_eq!(edits[1].new_text, "import \"provider\" as provider;\n\n");
}

#[test]
fn assists_offer_auto_import_quick_fixes_for_unresolved_module_path_members() {
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
                text: "fn run() { provider::shared_tools(); }".to_owned(),
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
    let file_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        file_text
            .find("shared_tools")
            .expect("expected unresolved member"),
    )
    .expect("expected offset to fit");

    let assists = analysis.assists(
        consumer,
        TextRange::new(TextSize::from(offset), TextSize::from(offset)),
    );

    let assist = assists
        .iter()
        .find(|assist| assist.id.as_str() == "import.auto")
        .expect("expected auto import assist");
    let edits = &assist.source_change.file_edits[0].edits;
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0].new_text, "shared_tools");
    assert_eq!(edits[1].new_text, "import \"provider\" as provider;\n\n");
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
