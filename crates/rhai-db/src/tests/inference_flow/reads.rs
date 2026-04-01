use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_tracks_flow_sensitive_reads_after_sequential_reassignments() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 1;
            value = "done";
            let picked = value;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Union(vec![TypeRef::Int, TypeRef::String]))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}
#[test]
fn snapshot_prefers_dominating_member_writes_at_read_positions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let user = #{ name: "Ada" };
            let before = user.name;
            user.name = 42;
            let after = user.name;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let before = symbol_id_by_name(&hir, "before", SymbolKind::Variable);
    let after = symbol_id_by_name(&hir, "after", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, before),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, after),
        Some(&TypeRef::Int)
    );
}
#[test]
fn snapshot_prefers_dominating_index_writes_at_read_positions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let items = [];
            items[0] = 1;
            let first = items[0];
            items[0] = "x";
            let second = items[0];
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let first = symbol_id_by_name(&hir, "first", SymbolKind::Variable);
    let second = symbol_id_by_name(&hir, "second", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, second),
        Some(&TypeRef::String)
    );
}
