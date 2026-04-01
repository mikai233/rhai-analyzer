use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_project::ProjectConfig;
use rhai_vfs::{DocumentVersion, normalize_path};
use std::path::Path;

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
