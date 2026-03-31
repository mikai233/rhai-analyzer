use std::path::Path;

use rhai_db::ChangeSet;
use rhai_syntax::TextSize;
use rhai_vfs::{DocumentVersion, normalize_path};

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition, ReferenceKind};

#[test]
fn navigation_and_references_cover_object_field_value_usages() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
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

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected main text");

    let mode_decl = u32::try_from(text.find("root, mode").expect("expected mode param") + 6)
        .expect("expected mode decl offset");
    let mode_usage = u32::try_from(text.find("mode: mode").expect("expected mode usage") + 7)
        .expect("expected mode usage offset");
    let workspace_decl = u32::try_from(
        text.find("workspace_name =")
            .expect("expected workspace declaration"),
    )
    .expect("expected workspace decl offset");
    let workspace_usage = u32::try_from(
        text.find("workspace: workspace_name")
            .expect("expected workspace usage")
            + 11,
    )
    .expect("expected workspace usage offset");

    let mode_definition = analysis.goto_definition(FilePosition {
        file_id,
        offset: mode_usage,
    });
    assert_eq!(mode_definition.len(), 1);
    assert!(
        mode_definition[0]
            .full_range
            .contains(TextSize::from(mode_decl))
    );

    let workspace_definition = analysis.goto_definition(FilePosition {
        file_id,
        offset: workspace_usage,
    });
    assert_eq!(workspace_definition.len(), 1);
    assert!(
        workspace_definition[0]
            .full_range
            .contains(TextSize::from(workspace_decl))
    );

    let mode_references = analysis
        .find_references(FilePosition {
            file_id,
            offset: mode_decl,
        })
        .expect("expected mode references");
    assert!(mode_references.references.iter().any(|reference| {
        reference.kind == ReferenceKind::Reference
            && reference.range.contains(TextSize::from(mode_usage))
    }));

    let workspace_references = analysis
        .find_references(FilePosition {
            file_id,
            offset: workspace_decl,
        })
        .expect("expected workspace references");
    assert!(workspace_references.references.iter().any(|reference| {
        reference.kind == ReferenceKind::Reference
            && reference.range.contains(TextSize::from(workspace_usage))
    }));
}

#[test]
fn rename_updates_object_field_declarations_and_usages_in_same_file() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
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

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected main text");
    let offset = u32::try_from(text.find("DEFAULTS.name").expect("expected field usage") + 9)
        .expect("offset");

    let prepared = analysis
        .rename(FilePosition { file_id, offset }, "title".to_owned())
        .expect("expected rename");
    assert!(
        prepared.plan.issues.is_empty(),
        "{:?}",
        prepared.plan.issues
    );

    let source_change = prepared
        .source_change
        .expect("expected object field rename source change");
    assert_eq!(source_change.file_edits.len(), 1);
    assert!(
        source_change.file_edits[0].edits.len() >= 2,
        "expected declaration and usage edits, got {:?}",
        source_change.file_edits[0].edits
    );
    assert!(
        source_change.file_edits[0]
            .edits
            .iter()
            .all(|edit| edit.new_text == "title")
    );
}

#[test]
fn rename_updates_object_field_usages_across_files() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
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
            rhai_db::FileChange {
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
        project: Some(rhai_project::ProjectConfig::default()),
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
    let provider_text = analysis
        .db
        .file_text(provider)
        .expect("expected provider text");
    let offset = u32::try_from(
        provider_text
            .find("name: \"demo\"")
            .expect("expected field declaration"),
    )
    .expect("offset");

    let prepared = analysis
        .rename(
            FilePosition {
                file_id: provider,
                offset,
            },
            "title".to_owned(),
        )
        .expect("expected rename");
    assert!(
        prepared.plan.issues.is_empty(),
        "{:?}",
        prepared.plan.issues
    );

    let source_change = prepared
        .source_change
        .expect("expected object field rename source change");
    assert!(
        source_change
            .file_edits
            .iter()
            .any(|edit| edit.file_id == provider),
        "expected provider edits, got {:?}",
        source_change.file_edits
    );
    assert!(
        source_change
            .file_edits
            .iter()
            .any(|edit| edit.file_id == consumer),
        "expected consumer edits, got {:?}",
        source_change.file_edits
    );
    assert!(
        source_change
            .file_edits
            .iter()
            .all(|file_edit| file_edit.edits.iter().all(|edit| edit.new_text == "title"))
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
fn renaming_static_import_module_reference_renames_file_and_updates_imports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "other.rhai".into(),
                text: "import \"demo\" as tools;\n".to_owned(),
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
    let demo = analysis
        .db
        .vfs()
        .file_id(Path::new("demo.rhai"))
        .expect("expected demo.rhai");
    let other = analysis
        .db
        .vfs()
        .file_id(Path::new("other.rhai"))
        .expect("expected other.rhai");
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"demo\"")
            .expect("expected import literal")
            + 1,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "renamed_demo",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_renames.len(), 1);
    assert_eq!(source_change.file_renames[0].file_id, demo);
    assert_eq!(
        source_change.file_renames[0].new_path,
        normalize_path(Path::new("renamed_demo.rhai"))
    );

    let consumer_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == consumer)
        .expect("expected consumer edits");
    assert_eq!(consumer_edits.edits.len(), 1);
    assert_eq!(consumer_edits.edits[0].new_text, "\"renamed_demo\"");

    let other_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == other)
        .expect("expected other edits");
    assert_eq!(other_edits.edits.len(), 1);
    assert_eq!(other_edits.edits[0].new_text, "\"renamed_demo\"");
}

#[test]
fn renaming_static_import_module_reference_preserves_path_prefixes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "shared/demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"shared/demo\" as d;\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "nested.rhai".into(),
                text: "import \"a/b/c\" as d;\n".to_owned(),
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
        .file_id(Path::new("shared/demo.rhai"))
        .expect("expected provider file");
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"shared/demo\"")
            .expect("expected import literal")
            + 8,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "renamed",
        )
        .expect("expected prepared rename");

    assert!(rename.plan.issues.is_empty());
    let source_change = rename
        .source_change
        .expect("expected concrete source change");
    assert_eq!(source_change.file_renames.len(), 1);
    assert_eq!(source_change.file_renames[0].file_id, provider);
    assert_eq!(
        source_change.file_renames[0].new_path,
        normalize_path(Path::new("shared/renamed.rhai"))
    );
    let consumer_edits = source_change
        .file_edits
        .iter()
        .find(|edit| edit.file_id == consumer)
        .expect("expected consumer edits");
    assert_eq!(consumer_edits.edits.len(), 1);
    assert_eq!(consumer_edits.edits[0].new_text, "\"shared/renamed\"");
}

#[test]
fn renaming_static_import_module_reference_rejects_new_path_segments() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "demo.rhai".into(),
                text: "fn hello() {}\n".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "import \"demo\" as d;\n".to_owned(),
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
    let consumer_text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(
        consumer_text
            .find("\"demo\"")
            .expect("expected import literal")
            + 1,
    )
    .expect("expected offset to fit");

    let rename = analysis
        .rename(
            FilePosition {
                file_id: consumer,
                offset,
            },
            "other/path",
        )
        .expect("expected prepared rename");

    assert!(rename.source_change.is_none());
    assert!(
        rename.plan.issues.iter().any(|issue| issue
            .message
            .contains("only supports changing the file name")),
        "expected path-segment rename issue, got {:?}",
        rename.plan.issues
    );
}
