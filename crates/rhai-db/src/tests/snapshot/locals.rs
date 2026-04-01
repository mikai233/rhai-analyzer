use crate::tests::{
    assert_workspace_files_have_no_syntax_diagnostics, offset_in, symbol_id_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_infers_local_function_return_and_variable_flow_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn make_blob() {
                return blob(10);
            }

            fn run() {
                let value = make_blob();
                let alias = value;
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
    let text = snapshot.file_text(file_id).expect("expected text");

    let make_blob = symbol_id_by_name(&hir, "make_blob", SymbolKind::Function);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let alias = symbol_id_by_name(&hir, "alias", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, make_blob),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Blob),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, alias),
        Some(&TypeRef::Blob)
    );

    let call_offset = offset_in(&text, "make_blob();") + TextSize::from(9);
    assert_eq!(
        snapshot.inferred_expr_type_at(file_id, call_offset),
        Some(&TypeRef::Blob)
    );
}
#[test]
fn snapshot_propagates_argument_types_into_local_function_parameters() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                return value;
            }

            fn run() {
                let result = echo(blob(10));
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

    let echo = symbol_id_by_name(&hir, "echo", SymbolKind::Function);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, echo),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Blob],
            ret: Box::new(TypeRef::Blob),
        }))
    );
}
#[test]
fn snapshot_tracks_ambiguous_local_callable_results() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value int
            /// @return int
            fn parse_int(value) {
                1
            }

            /// @param value string
            /// @return string
            fn parse_text(value) {
                "value"
            }

            let target = parse_int;
            target = parse_text;
            let seed = if flag { 1 } else { "value" };
            let result = target(seed);
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
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(TypeRef::Ambiguous(items))
            if items.len() == 2
                && items.iter().all(|item| matches!(
                    item,
                    TypeRef::Union(union_items)
                        if union_items.contains(&TypeRef::Int)
                            && union_items.contains(&TypeRef::String)
                ))
    ));
}
#[test]
fn snapshot_resolves_local_function_overloads_by_arity() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @return int
            fn do_something() {
                1
            }

            /// @param value int
            /// @return string
            fn do_something(value) {
                value.to_string()
            }

            let first = do_something();
            let second = do_something(1);
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
