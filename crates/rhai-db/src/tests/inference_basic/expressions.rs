use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
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
fn snapshot_infers_builtin_index_and_operator_semantics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let text = "Rhai";
            let first = text[0];
            let part = text[1..3];

            let buf = blob(4, 7);
            let byte = buf[0];
            let slice = buf[1..3];

            let flags = 10;
            let bit = flags[1];
            let bits = flags[1..=2];

            let merged = #{ a: 1 } + #{ b: 2.0 };
            let appended = [1, 2] + [3.0];
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
    let part = symbol_id_by_name(&hir, "part", SymbolKind::Variable);
    let byte = symbol_id_by_name(&hir, "byte", SymbolKind::Variable);
    let slice = symbol_id_by_name(&hir, "slice", SymbolKind::Variable);
    let bit = symbol_id_by_name(&hir, "bit", SymbolKind::Variable);
    let bits = symbol_id_by_name(&hir, "bits", SymbolKind::Variable);
    let merged = symbol_id_by_name(&hir, "merged", SymbolKind::Variable);
    let appended = symbol_id_by_name(&hir, "appended", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, first),
        Some(&TypeRef::Char)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, part),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, byte),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, slice),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, bit),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, bits),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, merged),
        Some(&TypeRef::Object(
            [
                ("a".to_owned(), TypeRef::Int),
                ("b".to_owned(), TypeRef::Float),
            ]
            .into_iter()
            .collect(),
        ))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, appended),
        Some(&TypeRef::Array(Box::new(TypeRef::Union(vec![
            TypeRef::Int,
            TypeRef::Float,
        ]))))
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
