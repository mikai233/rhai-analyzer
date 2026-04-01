use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectReferenceKind};
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

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
