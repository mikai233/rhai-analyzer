use crate::tests::{
    assert_global_function_has_signature, assert_global_functions_include,
    assert_workspace_files_have_no_syntax_diagnostics, global_function_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_project::ProjectConfig;
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
    assert_eq!(hir.root_range, parse.root().text_range());
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
    assert!(
        snapshot
            .host_types()
            .iter()
            .any(|host_type| host_type.name == "string"),
        "expected builtin string host type"
    );
    assert!(
        snapshot
            .host_types()
            .iter()
            .any(|host_type| host_type.name == "Widget"),
        "expected project Widget host type"
    );
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
    assert_eq!(
        snapshot.external_signatures().get("print"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::Any],
            ret: Box::new(rhai_hir::TypeRef::Unit),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("debug"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::Any],
            ret: Box::new(rhai_hir::TypeRef::Unit),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("parse_int"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("parse_float"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::Float),
        }))
    );
    assert_eq!(
        snapshot.external_signatures().get("eval"),
        Some(&rhai_hir::TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String],
            ret: Box::new(rhai_hir::TypeRef::Dynamic),
        }))
    );
    assert_global_functions_include(
        &snapshot,
        &[
            "blob",
            "timestamp",
            "Fn",
            "is_def_var",
            "is_def_fn",
            "type_of",
            "print",
            "debug",
            "parse_int",
            "parse_float",
            "eval",
        ],
    );
    assert_eq!(
        global_function_by_name(&snapshot, "blob").overloads.len(),
        3
    );
    assert_global_function_has_signature(
        &snapshot,
        "is_def_fn",
        &rhai_hir::FunctionTypeRef {
            params: vec![
                rhai_hir::TypeRef::String,
                rhai_hir::TypeRef::String,
                rhai_hir::TypeRef::Int,
            ],
            ret: Box::new(rhai_hir::TypeRef::Bool),
        },
    );
    assert_global_function_has_signature(
        &snapshot,
        "parse_int",
        &rhai_hir::FunctionTypeRef {
            params: vec![rhai_hir::TypeRef::String, rhai_hir::TypeRef::Int],
            ret: Box::new(rhai_hir::TypeRef::Int),
        },
    );
    assert_eq!(snapshot.external_signatures().get("math::parse"), None);
}
