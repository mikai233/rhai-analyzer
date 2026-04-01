use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_infers_builtin_string_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = "hello";
            let contains_fn = value.contains;
            let matched = value.contains("ell");
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

    let contains_fn = symbol_id_by_name(&hir, "contains_fn", SymbolKind::Variable);
    let matched = symbol_id_by_name(&hir, "matched", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, contains_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Ambiguous(vec![TypeRef::Char, TypeRef::String])],
            ret: Box::new(TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, matched),
        Some(&TypeRef::Bool)
    );
}
#[test]
fn snapshot_infers_builtin_array_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let values = [1, 2, 3];
            let len_fn = values.len;
            let count = values.len();
            let found = values.contains(2);
            let getter = values.get;
            let first = values.get(0);
            let removed = values.remove(1);
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

    let len_fn = symbol_id_by_name(&hir, "len_fn", SymbolKind::Variable);
    let count = symbol_id_by_name(&hir, "count", SymbolKind::Variable);
    let found = symbol_id_by_name(&hir, "found", SymbolKind::Variable);
    let getter = symbol_id_by_name(&hir, "getter", SymbolKind::Variable);
    let first = symbol_id_by_name(&hir, "first", SymbolKind::Variable);
    let removed = symbol_id_by_name(&hir, "removed", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, len_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, count),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, found),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, getter),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Union(vec![TypeRef::Int, TypeRef::Unit])),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Union(vec![TypeRef::Int, TypeRef::Unit]))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, removed),
        Some(&TypeRef::Union(vec![TypeRef::Int, TypeRef::Unit]))
    );
}
#[test]
fn snapshot_infers_builtin_map_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let user = #{ name: "Ada" };
            let names = user.keys();
            let values_fn = user.values;
            let values = user.values();
            let name_fn = user.get;
            let name = user.get("name");
            let shadowed = #{ values: "own" }.values;
            let present = user.contains("name");
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

    let names = symbol_id_by_name(&hir, "names", SymbolKind::Variable);
    let values_fn = symbol_id_by_name(&hir, "values_fn", SymbolKind::Variable);
    let values = symbol_id_by_name(&hir, "values", SymbolKind::Variable);
    let name_fn = symbol_id_by_name(&hir, "name_fn", SymbolKind::Variable);
    let name = symbol_id_by_name(&hir, "name", SymbolKind::Variable);
    let shadowed = symbol_id_by_name(&hir, "shadowed", SymbolKind::Variable);
    let present = symbol_id_by_name(&hir, "present", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, names),
        Some(&TypeRef::Array(Box::new(TypeRef::String)))
    );
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, values_fn),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::String)
                && items.iter().any(|item| matches!(
                    item,
                    TypeRef::Function(FunctionTypeRef { params, ret })
                        if params.is_empty()
                            && ret.as_ref() == &TypeRef::Array(Box::new(TypeRef::String))
                ))
    ));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, values),
        Some(&TypeRef::Array(Box::new(TypeRef::String)))
    );
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, name_fn),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::String)
                && items.iter().any(|item| matches!(
                    item,
                    TypeRef::Function(FunctionTypeRef { params, ret })
                        if params.len() == 1
                            && params[0] == TypeRef::String
                            && ret.as_ref() == &TypeRef::Union(vec![TypeRef::String, TypeRef::Unit])
                ))
    ));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, name),
        Some(&TypeRef::Union(vec![TypeRef::String, TypeRef::Unit]))
    );
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, shadowed),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::String)
                && items.iter().any(|item| matches!(
                    item,
                    TypeRef::Function(FunctionTypeRef { params, ret })
                        if params.is_empty()
                            && ret.as_ref() == &TypeRef::Array(Box::new(TypeRef::String))
                ))
    ));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, present),
        Some(&TypeRef::Bool)
    );
}
#[test]
fn snapshot_infers_builtin_range_and_timestamp_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let span = 1..10;
            let start_fn = span.start;
            let first = span.start();
            let exclusive = span.is_exclusive();

            let now = timestamp();
            let elapsed_fn = now.elapsed;
            let seconds = now.elapsed();
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

    let start_fn = symbol_id_by_name(&hir, "start_fn", SymbolKind::Variable);
    let first = symbol_id_by_name(&hir, "first", SymbolKind::Variable);
    let exclusive = symbol_id_by_name(&hir, "exclusive", SymbolKind::Variable);
    let elapsed_fn = symbol_id_by_name(&hir, "elapsed_fn", SymbolKind::Variable);
    let seconds = symbol_id_by_name(&hir, "seconds", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, start_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, exclusive),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, elapsed_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Float),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, seconds),
        Some(&TypeRef::Float)
    );
}
#[test]
fn snapshot_infers_builtin_primitive_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let count = 41;
            let odd_fn = count.is_odd;
            let odd = count.is_odd();
            let as_float = count.to_float();
            let limited = count.max(50);

            let ratio = 3.5;
            let floor_fn = ratio.floor;
            let rounded_down = ratio.floor();
            let integral = ratio.to_int();
            let clamped = ratio.max(5);

            let initial = 'a';
            let upper_fn = initial.to_upper;
            let upper = initial.to_upper();
            let code = upper.to_int();
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

    let odd_fn = symbol_id_by_name(&hir, "odd_fn", SymbolKind::Variable);
    let odd = symbol_id_by_name(&hir, "odd", SymbolKind::Variable);
    let as_float = symbol_id_by_name(&hir, "as_float", SymbolKind::Variable);
    let floor_fn = symbol_id_by_name(&hir, "floor_fn", SymbolKind::Variable);
    let rounded_down = symbol_id_by_name(&hir, "rounded_down", SymbolKind::Variable);
    let integral = symbol_id_by_name(&hir, "integral", SymbolKind::Variable);
    let limited = symbol_id_by_name(&hir, "limited", SymbolKind::Variable);
    let clamped = symbol_id_by_name(&hir, "clamped", SymbolKind::Variable);
    let upper_fn = symbol_id_by_name(&hir, "upper_fn", SymbolKind::Variable);
    let upper = symbol_id_by_name(&hir, "upper", SymbolKind::Variable);
    let code = symbol_id_by_name(&hir, "code", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, odd_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, odd),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, as_float),
        Some(&TypeRef::Float)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, limited),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, floor_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Float),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, rounded_down),
        Some(&TypeRef::Float)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, integral),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, clamped),
        Some(&TypeRef::Float)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, upper_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Char),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, upper),
        Some(&TypeRef::Char)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, code),
        Some(&TypeRef::Int)
    );
}
