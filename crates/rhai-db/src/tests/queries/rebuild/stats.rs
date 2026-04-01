use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_vfs::{DocumentVersion, normalize_path};
use std::path::Path;

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
