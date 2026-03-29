use crate::tests::{
    assert_workspace_files_have_no_syntax_diagnostics, offset_in, symbol_id_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_exposes_cached_parse_hir_and_diagnostics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = ;",
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let parse = snapshot.parse(file_id).expect("expected parse");
    let hir = snapshot.hir(file_id).expect("expected hir");

    assert_eq!(parse.errors().len(), 1);
    assert_eq!(snapshot.syntax_diagnostics(file_id).len(), 1);
    assert_eq!(
        snapshot.semantic_diagnostics(file_id),
        hir.diagnostics().as_slice()
    );
    assert_eq!(hir.root_range, parse.root().range());
}

#[test]
fn snapshot_exposes_project_semantics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            engine: rhai_project::EngineOptions {
                disabled_symbols: vec!["eval".to_owned()],
                custom_syntaxes: vec!["unless".to_owned()],
            },
            modules: [(
                "math".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Math helpers".to_owned()),
                    functions: [
                        (
                            "add".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun(int, int) -> int".to_owned(),
                                return_type: None,
                                docs: Some("Adds two numbers".to_owned()),
                            }],
                        ),
                        (
                            "parse".to_owned(),
                            vec![
                                rhai_project::FunctionSpec {
                                    signature: "fun(string) -> int".to_owned(),
                                    return_type: None,
                                    docs: None,
                                },
                                rhai_project::FunctionSpec {
                                    signature: "fun(float) -> int".to_owned(),
                                    return_type: None,
                                    docs: None,
                                },
                            ],
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    constants: [(
                        "PI".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "float".to_owned(),
                            docs: Some("Circle ratio".to_owned()),
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            )]
            .into_iter()
            .collect(),
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: Some("A UI widget".to_owned()),
                    methods: [(
                        "open".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(string) -> bool".to_owned(),
                            return_type: None,
                            docs: Some("Opens the widget".to_owned()),
                        }],
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
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);

    assert_eq!(snapshot.disabled_symbols(), ["eval"]);
    assert_eq!(snapshot.custom_syntaxes(), ["unless"]);
    assert_eq!(snapshot.host_modules().len(), 1);
    assert_eq!(snapshot.host_modules()[0].name, "math");
    assert_eq!(snapshot.host_modules()[0].functions[0].name, "add");
    assert_eq!(snapshot.host_modules()[0].constants[0].name, "PI");
    assert_eq!(snapshot.host_types().len(), 1);
    assert_eq!(snapshot.host_types()[0].name, "Widget");
    assert_eq!(
        snapshot.external_signatures().get("math::add"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::Int, rhai_hir::TypeRef::Int],
            ret: Box::new(rhai_hir::TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("math::PI"),
        Some(&rhai_hir::TypeRef::Float)
    );
    assert_eq!(
        snapshot.external_signatures().get("Widget::open"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("blob"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::Int],
            ret: Box::new(rhai_hir::TypeRef::Blob),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("timestamp"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![],
            ret: Box::new(rhai_hir::TypeRef::Timestamp),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("Fn"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::FnPtr),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("is_def_var"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("is_def_fn"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String, rhai_hir::TypeRef::Int],
            ret: Box::new(rhai_hir::TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("type_of"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::Any],
            ret: Box::new(rhai_hir::TypeRef::String),
        }))
    );
    assert_eq!(snapshot.global_functions().len(), 6);
    assert_eq!(snapshot.global_functions()[0].name, "blob");
    assert_eq!(snapshot.global_functions()[1].name, "timestamp");
    assert_eq!(snapshot.global_functions()[2].name, "Fn");
    assert_eq!(snapshot.global_functions()[3].name, "is_def_var");
    assert_eq!(snapshot.global_functions()[4].name, "is_def_fn");
    assert_eq!(snapshot.global_functions()[5].name, "type_of");
    assert_eq!(snapshot.global_functions()[0].overloads.len(), 3);
    assert_eq!(snapshot.external_signatures().get("math::parse"), None);
}

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
fn snapshot_infers_local_function_pointers_with_indirect_call_signatures() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                value
            }

            /// @type Fn
            let ptr = Fn("echo");
            let result = ptr(blob(10));
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
    let ptr = symbol_id_by_name(&hir, "ptr", SymbolKind::Variable);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);
    let blob_fn = TypeRef::Function(FunctionTypeRef {
        params: vec![TypeRef::Blob],
        ret: Box::new(TypeRef::Blob),
    });

    assert_eq!(snapshot.inferred_symbol_type(file_id, echo), Some(&blob_fn));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(snapshot.inferred_symbol_type(file_id, ptr), Some(&blob_fn));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Blob)
    );
}

#[test]
fn snapshot_infers_caller_scope_call_targets_and_return_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                value
            }

            let direct = call!(echo, blob(10));
            let indirect = call!(Fn("echo"), blob(11));
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

    let value = symbol_id_by_name(&hir, "value", SymbolKind::Parameter);
    let direct = symbol_id_by_name(&hir, "direct", SymbolKind::Variable);
    let indirect = symbol_id_by_name(&hir, "indirect", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, direct),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, indirect),
        Some(&TypeRef::Blob)
    );
}

#[test]
fn snapshot_infers_builtin_function_pointers_from_fn_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let ptr = Fn("timestamp");
            let result = ptr();
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

    let ptr = symbol_id_by_name(&hir, "ptr", SymbolKind::Variable);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, ptr),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Timestamp),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Timestamp)
    );
}

#[test]
fn snapshot_infers_external_function_pointers_from_fn_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let ptr = Fn("math::add");
                let result = ptr(1, 2);
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

    let ptr = symbol_id_by_name(&hir, "ptr", SymbolKind::Variable);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, ptr),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int, TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_infers_path_qualified_external_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                let result = math::add(1, 2);
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

    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}
