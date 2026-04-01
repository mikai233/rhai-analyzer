use std::path::Path;
use std::sync::Arc;

use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::{DocumentVersion, normalize_path};

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
