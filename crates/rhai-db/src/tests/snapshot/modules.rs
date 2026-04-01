use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_infers_builtin_introspection_function_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn check(value) { value }
            fn int.bump(delta) { this + delta }

            let answer = 42;
            let has_var = is_def_var("answer");
            let has_fn = is_def_fn("check", 1);
            let has_method = is_def_fn("int", "bump", 1);
            let kind_fn = type_of(answer);
            let kind_method = answer.type_of();
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

    for name in ["has_var", "has_fn", "has_method"] {
        let symbol = symbol_id_by_name(&hir, name, SymbolKind::Variable);
        assert_eq!(
            snapshot.inferred_symbol_type(file_id, symbol),
            Some(&TypeRef::Bool),
            "expected `{name}` to infer as bool"
        );
    }

    for name in ["kind_fn", "kind_method"] {
        let symbol = symbol_id_by_name(&hir, name, SymbolKind::Variable);
        assert_eq!(
            snapshot.inferred_symbol_type(file_id, symbol),
            Some(&TypeRef::String),
            "expected `{name}` to infer as string"
        );
    }
}
#[test]
fn snapshot_specializes_generic_module_function_returns_from_argument_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let picked = tools::pick([1, 2, 3], 0);
                let text = tools::pick(["a", "b"], 0);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "tools".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "pick".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(array<T>, int) -> T".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let picked = symbol_id_by_name(&hir, "picked", SymbolKind::Variable);
    let text = symbol_id_by_name(&hir, "text", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, picked),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, text),
        Some(&TypeRef::String)
    );
}
#[test]
fn snapshot_specializes_generic_module_function_parameter_expectations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let result = tools::map_one(1, |value| value.to_float());
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "tools".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "map_one".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(T, fun(T) -> U) -> U".to_owned(),
                            return_type: None,
                            docs: None,
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Float)
    );
}
#[test]
fn snapshot_specializes_applied_generic_module_abstractions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let boxed = tools::box_value(1);
                let value = tools::unbox(boxed);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "tools".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [
                        (
                            "box_value".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun(T) -> Box<T>".to_owned(),
                                return_type: None,
                                docs: None,
                            }],
                        ),
                        (
                            "unbox".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun(Box<T>) -> T".to_owned(),
                                return_type: None,
                                docs: None,
                            }],
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let boxed = symbol_id_by_name(&hir, "boxed", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, boxed),
        Some(&TypeRef::Applied {
            name: "Box".to_owned(),
            args: vec![TypeRef::Int],
        })
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
}
