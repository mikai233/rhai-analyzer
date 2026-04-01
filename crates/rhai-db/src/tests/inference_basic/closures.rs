use crate::infer::infer_file_types;
use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, SymbolKind, TypeRef, lower_file};
use rhai_vfs::DocumentVersion;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

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
fn infer_file_types_propagates_object_shape_expectations_into_nested_closures() {
    let parse = rhai_syntax::parse_text(
        r#"
            let config = #{
                nested: #{
                    callback: |value| value
                }
            };
        "#,
    );
    assert!(
        parse.errors().is_empty(),
        "expected valid Rhai syntax, got errors: {:?}",
        parse.errors()
    );
    let hir = lower_file(&parse);

    let config = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "config" && symbol.kind == SymbolKind::Variable)
                .then_some(rhai_hir::SymbolId(index as u32))
        })
        .expect("expected `config` symbol");
    let callback_param = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Parameter)
                .then_some(rhai_hir::SymbolId(index as u32))
        })
        .expect("expected closure parameter");

    let mut seeds = HashMap::new();
    seeds.insert(
        config,
        TypeRef::Object(BTreeMap::from([(
            "nested".to_owned(),
            TypeRef::Object(BTreeMap::from([(
                "callback".to_owned(),
                TypeRef::Function(FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }),
            )])),
        )])),
    );

    let inference = infer_file_types(
        &hir,
        &ExternalSignatureIndex::default(),
        &[],
        &[],
        &[],
        &[],
        &seeds,
    );

    assert_eq!(
        inference.symbol_types.get(&callback_param),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        inference.symbol_types.get(&config),
        Some(&TypeRef::Object(BTreeMap::from([(
            "nested".to_owned(),
            TypeRef::Object(BTreeMap::from([(
                "callback".to_owned(),
                TypeRef::Function(FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }),
            )])),
        )])))
    );
}
