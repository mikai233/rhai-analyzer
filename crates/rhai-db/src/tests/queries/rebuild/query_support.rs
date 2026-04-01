use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, offset_in};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_vfs::DocumentVersion;
use std::path::Path;

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
