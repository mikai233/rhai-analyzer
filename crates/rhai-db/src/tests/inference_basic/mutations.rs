use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn snapshot_infers_types_from_simple_field_and_index_mutations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let user = #{};
            user.name = "Ada";
            let picked = user.name;

            let items = [];
            items[0] = 1;
            let first = items[0];

            let scores = #{};
            scores["best"] = 42;
            let best = scores["best"];
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let user = symbol_id_by_name(&hir, "user", SymbolKind::Variable);
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);
    let items = symbol_id_by_name(&hir, "items", SymbolKind::Variable);
    let first = symbol_id_by_name(&hir, "first", SymbolKind::Variable);
    let scores = symbol_id_by_name(&hir, "scores", SymbolKind::Variable);
    let best = symbol_id_by_name(&hir, "best", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, user),
        Some(&TypeRef::Object(BTreeMap::from([(
            "name".to_owned(),
            TypeRef::String,
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, items),
        Some(&TypeRef::Array(Box::new(TypeRef::Int)))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, scores),
        Some(&TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::Int),
        ))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, best),
        Some(&TypeRef::Int)
    );
}
#[test]
fn snapshot_infers_types_from_nested_and_compound_mutations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let root = #{};
            root.child.field = 1;
            let child = root.child;
            let field = root.child.field;

            let counter = 1;
            counter += 2;
            let total = counter;

            let obj = #{};
            obj.value = 1;
            obj.value += 2;
            let picked = obj.value;

            let arr = [];
            arr[0] ??= 1;
            arr[0] += 2;
            let first = arr[0];
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let root = symbol_id_by_name(&hir, "root", SymbolKind::Variable);
    let child = symbol_id_by_name(&hir, "child", SymbolKind::Variable);
    let field = symbol_id_by_name(&hir, "field", SymbolKind::Variable);
    let counter = symbol_id_by_name(&hir, "counter", SymbolKind::Variable);
    let total = symbol_id_by_name(&hir, "total", SymbolKind::Variable);
    let obj = symbol_id_by_name(&hir, "obj", SymbolKind::Variable);
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);
    let arr = symbol_id_by_name(&hir, "arr", SymbolKind::Variable);
    let first = symbol_id_by_name(&hir, "first", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, root),
        Some(&TypeRef::Object(BTreeMap::from([(
            "child".to_owned(),
            TypeRef::Object(BTreeMap::from([("field".to_owned(), TypeRef::Int)])),
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, child),
        Some(&TypeRef::Object(BTreeMap::from([(
            "field".to_owned(),
            TypeRef::Int,
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, field),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, counter),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, total),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, obj),
        Some(&TypeRef::Object(BTreeMap::from([(
            "value".to_owned(),
            TypeRef::Int,
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, arr),
        Some(&TypeRef::Array(Box::new(TypeRef::Int)))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Int)
    );
}
#[test]
fn snapshot_infers_nested_field_chains_and_map_style_index_results() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let root = #{ user: #{ age: 1, name: "rhai" } };
            let alias = root;
            let age = alias.user.age;
            let user = alias.user;
            let indexed = user["age"];
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

    let root = symbol_id_by_name(&hir, "root", SymbolKind::Variable);
    let age = symbol_id_by_name(&hir, "age", SymbolKind::Variable);
    let user = symbol_id_by_name(&hir, "user", SymbolKind::Variable);
    let indexed = symbol_id_by_name(&hir, "indexed", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, root),
        Some(&TypeRef::Object(BTreeMap::from([(
            "user".to_owned(),
            TypeRef::Object(BTreeMap::from([
                ("age".to_owned(), TypeRef::Int),
                ("name".to_owned(), TypeRef::String),
            ])),
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, age),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, user),
        Some(&TypeRef::Object(BTreeMap::from([
            ("age".to_owned(), TypeRef::Int),
            ("name".to_owned(), TypeRef::String),
        ])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, indexed),
        Some(&TypeRef::Int)
    );
}
