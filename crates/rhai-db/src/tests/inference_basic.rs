use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn snapshot_infers_literal_and_operator_expression_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let count = 1 + 2;
            let mixed = 1 + 2.0;
            let text = "a" + "b";
            let flag = !false;
            let window = 1..10;
            let fallback = "a" ?? "b";
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

    let count = symbol_id_by_name(&hir, "count", SymbolKind::Variable);
    let mixed = symbol_id_by_name(&hir, "mixed", SymbolKind::Variable);
    let text = symbol_id_by_name(&hir, "text", SymbolKind::Variable);
    let flag = symbol_id_by_name(&hir, "flag", SymbolKind::Variable);
    let window = symbol_id_by_name(&hir, "window", SymbolKind::Variable);
    let fallback = symbol_id_by_name(&hir, "fallback", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, count),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mixed),
        Some(&TypeRef::Float)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, text),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, flag),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, window),
        Some(&TypeRef::Range)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, fallback),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_infers_assignment_paren_path_and_interpolated_string_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let value = 1;
                let assigned = (value = 2);
                let grouped = (assigned);
                let pi = global::math::PI;
                let add = global::math::add;
                let sum = add(grouped, 3);
                let message = `sum=${sum}`;
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "add".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int, int) -> int".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: [(
                        "PI".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "float".to_owned(),
                            docs: None,
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let assigned = symbol_id_by_name(&hir, "assigned", SymbolKind::Variable);
    let grouped = symbol_id_by_name(&hir, "grouped", SymbolKind::Variable);
    let pi = symbol_id_by_name(&hir, "pi", SymbolKind::Variable);
    let add = symbol_id_by_name(&hir, "add", SymbolKind::Variable);
    let sum = symbol_id_by_name(&hir, "sum", SymbolKind::Variable);
    let message = symbol_id_by_name(&hir, "message", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, assigned),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, grouped),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, pi),
        Some(&TypeRef::Float)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, add),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int, TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, sum),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, message),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_infers_tail_return_types_for_functions_and_closures() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn choose(flag) {
                if flag { 1 } else { 2.0 }
            }

            let mapper = || 1;
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

    let choose = symbol_id_by_name(&hir, "choose", SymbolKind::Function);
    let mapper = symbol_id_by_name(&hir, "mapper", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, choose),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Unknown],
            ret: Box::new(TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mapper),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Int),
        }))
    );
}

#[test]
fn snapshot_propagates_declared_function_types_into_closure_parameters() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @type fun(int) -> int
            let mapper = |value| value + 1;
            let result = mapper(1);
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

    let mapper = symbol_id_by_name(&hir, "mapper", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mapper),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_propagates_parameter_annotations_into_closure_arguments() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param callback fun(int) -> int
            /// @return int
            fn apply(callback) {
                callback(1)
            }

            let mapper = |value| value + 1;
            let result = apply(mapper);
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

    let mapper = symbol_id_by_name(&hir, "mapper", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mapper),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_propagates_return_annotations_into_returned_closures() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @return fun(int) -> int
            fn make_mapper() {
                |value| value + 1
            }

            let mapper = make_mapper();
            let result = mapper(1);
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

    let mapper = symbol_id_by_name(&hir, "mapper", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, mapper),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_infers_loop_expression_result_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let spun = loop { break; };
            let counted = loop { break 1; };
            let checked = while false { break; };
            let iterated = for item in [1, 2, 3] { if item > 1 { break; } };
            let retried = do { break; } while false;
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

    let spun = symbol_id_by_name(&hir, "spun", SymbolKind::Variable);
    let counted = symbol_id_by_name(&hir, "counted", SymbolKind::Variable);
    let checked = symbol_id_by_name(&hir, "checked", SymbolKind::Variable);
    let iterated = symbol_id_by_name(&hir, "iterated", SymbolKind::Variable);
    let retried = symbol_id_by_name(&hir, "retried", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, spun),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, counted),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, checked),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, iterated),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, retried),
        Some(&TypeRef::Unit)
    );
}

#[test]
fn snapshot_infers_for_loop_binding_types_for_common_iterables() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let array_item = for item in [1, 2, 3] { break item; };
            let array_index = for (item, index) in [1, 2, 3] { break index; };
            let char_item = for ch in "abc" { break ch; };
            let ranged = for value in 0..10 { break value; };
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

    let array_item = symbol_id_by_name(&hir, "array_item", SymbolKind::Variable);
    let array_index = symbol_id_by_name(&hir, "array_index", SymbolKind::Variable);
    let char_item = symbol_id_by_name(&hir, "char_item", SymbolKind::Variable);
    let ranged = symbol_id_by_name(&hir, "ranged", SymbolKind::Variable);
    let item = symbol_id_by_name(&hir, "item", SymbolKind::Variable);
    let index = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(symbol_index, symbol)| {
            (symbol.name == "index" && symbol.kind == SymbolKind::Variable)
                .then_some(rhai_hir::SymbolId(symbol_index as u32))
        })
        .expect("expected `index` loop binding");
    let ch = symbol_id_by_name(&hir, "ch", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);

    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, array_item),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, array_index),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, char_item),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Char)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, ranged),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, item),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, index),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, ch),
        Some(&TypeRef::Char)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_infers_block_index_and_field_expression_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let arr = [1, 2, 3];
            let picked = arr[0];
            let obj = #{ value: picked, label: "ok" };
            let field = obj.value;
            let tail = { let alias = field; alias };
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

    let arr = symbol_id_by_name(&hir, "arr", SymbolKind::Variable);
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);
    let field = symbol_id_by_name(&hir, "field", SymbolKind::Variable);
    let tail = symbol_id_by_name(&hir, "tail", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, arr),
        Some(&TypeRef::Array(Box::new(TypeRef::Int)))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, field),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, tail),
        Some(&TypeRef::Int)
    );
}

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

#[test]
fn snapshot_uses_only_fallthrough_branch_values_for_if_and_switch_exprs() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn choose(flag, mode) {
                let from_if = if flag { return 1; } else { "ok" };
                let from_switch = switch mode {
                    0 => { return 2; },
                    _ => "done"
                };
                let tail = { return 3; };
                from_if + from_switch
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

    let from_if = symbol_id_by_name(&hir, "from_if", SymbolKind::Variable);
    let from_switch = symbol_id_by_name(&hir, "from_switch", SymbolKind::Variable);
    let tail = symbol_id_by_name(&hir, "tail", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, from_if),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, from_switch),
        Some(&TypeRef::String)
    );
    assert_eq!(snapshot.inferred_symbol_type(file_id, tail), None);
}
