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
fn snapshot_prefers_specific_array_types_over_array_unknown_in_variable_unions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let a = [1, 2, 3];
            a = "world";

            for i in a {
            }
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

    let a = symbol_id_by_name(&hir, "a", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, a),
        Some(&TypeRef::Union(vec![
            TypeRef::Array(Box::new(TypeRef::Int)),
            TypeRef::String,
        ]))
    );
}

#[test]
fn debug_snapshot_array_unknown_source() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let a = [1, 2, 3];
            a = "world";

            for i in a {
            }
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let a = symbol_id_by_name(&hir, "a", SymbolKind::Variable);

    eprintln!("a type = {:?}", snapshot.inferred_symbol_type(file_id, a));
    for flow in hir.value_flows_into(a) {
        eprintln!(
            "flow {:?} expr {:?} kind {:?} expr_ty {:?}",
            flow.range,
            flow.expr,
            hir.expr(flow.expr).kind,
            snapshot.inferred_expr_type_at(file_id, hir.expr(flow.expr).range.start())
        );
    }
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

#[test]
fn snapshot_narrows_union_values_after_type_of_function_guards() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int | string
            let value = source;

            let picked = if type_of(value) == "string" {
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
fn snapshot_narrows_union_values_after_type_of_method_guards() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int | string
            let value = source;

            let picked = if value.type_of() != "string" {
                value
            } else {
                0
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
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_narrows_union_values_in_switch_type_of_string_arms() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int | string
            let value = source;

            let picked = switch type_of(value) {
                "string" => value,
                _ => "fallback",
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
fn snapshot_narrows_union_values_in_switch_type_of_wildcard_arms() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type int | string
            let value = source;

            let picked = switch type_of(value) {
                "string" => "fallback",
                _ => {
                    let narrowed = value;
                    narrowed
                },
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

    let narrowed = symbol_id_by_name(&hir, "narrowed", SymbolKind::Variable);
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, narrowed),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Union(vec![TypeRef::String, TypeRef::Int]))
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

#[test]
fn snapshot_narrows_member_reads_after_type_of_method_guards() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let user = if flag {
                #{ name: "Ada" }
            } else {
                #{ name: 42 }
            };

            let picked = if user.name.type_of() == "string" {
                user.name
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
fn snapshot_narrows_index_reads_after_type_of_function_guards() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let items = if flag {
                ["Ada"]
            } else {
                [42]
            };

            let picked = if type_of(items[0]) == "string" {
                items[0]
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
fn snapshot_narrows_member_reads_in_switch_type_of_arms() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let user = if flag {
                #{ name: "Ada" }
            } else {
                #{ name: 42 }
            };

            let picked = switch type_of(user.name) {
                "string" => user.name,
                _ => "fallback",
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
fn snapshot_narrows_index_reads_after_not_equal_unit_checks() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let none = loop { break; };
            let items = if flag {
                ["Ada"]
            } else {
                [none]
            };

            let picked = if items[0] != none {
                items[0]
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
