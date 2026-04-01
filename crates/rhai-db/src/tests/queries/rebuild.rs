use std::path::Path;
use std::sync::Arc;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
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
