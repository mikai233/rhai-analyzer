use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectReferenceKind};
use rhai_vfs::DocumentVersion;

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
fn project_rename_plan_for_automatic_global_constant_includes_global_usages() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            const ANSWER = 42;

            fn run() {
                global::ANSWER
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

    let plan = snapshot
        .rename_plan(file_id, offset_in(&text, "ANSWER ="), "RESULT")
        .expect("expected project rename plan");

    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].symbol.name, "ANSWER");
    assert_eq!(plan.occurrences.len(), 2);
    assert!(plan.occurrences.iter().any(|occurrence| {
        occurrence.kind == ProjectReferenceKind::Reference
            && occurrence
                .range
                .contains(offset_in(&text, "global::ANSWER") + rhai_syntax::TextSize::from(8))
    }));
}
