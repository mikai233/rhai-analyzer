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
fn snapshot_tracks_flow_sensitive_reads_through_if_else_overwrites() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 0;
            if true {
                value = 1;
            } else {
                value = 2;
            }
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_tracks_flow_sensitive_reads_after_conditional_loop_updates() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 1;
            while false {
                value = "loop";
            }
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Union(vec![TypeRef::Int, TypeRef::String]))
    );
}

#[test]
fn snapshot_narrows_nullable_values_in_truthy_if_branches() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;

            let picked = if value {
                value
            } else {
                "fallback"
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_narrows_nullable_values_in_negated_else_branches() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;

            let picked = if !value {
                "fallback"
            } else {
                value
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_narrows_nullable_values_after_not_equal_unit_checks() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if value != none {
                value
            } else {
                "fallback"
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_narrows_nullable_values_after_equal_unit_else_branches() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if value == none {
                "fallback"
            } else {
                value
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_narrows_nullable_values_to_unit_in_equal_unit_branches() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if !(value != none) {
                value
            } else {
                none
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Unit)
    );
}

#[test]
fn snapshot_narrows_nullable_values_through_conjunctive_null_checks() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };
            let ready = true;

            let picked = if value != none && ready {
                value
            } else {
                "fallback"
            };
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

    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
}
