use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_does_not_add_any_when_passing_known_values_to_any_parameters() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let text = "hello";
            print(text);

            let mixed = if true { "hello" } else { 1 };
            print(mixed);
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

    let text = symbol_id_by_name(&hir, "text", SymbolKind::Variable);
    let mixed = symbol_id_by_name(&hir, "mixed", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, text),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mixed),
        Some(&TypeRef::Union(vec![TypeRef::String, TypeRef::Int]))
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
