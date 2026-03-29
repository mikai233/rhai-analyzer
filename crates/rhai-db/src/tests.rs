use std::path::Path;
use std::ptr;
use std::sync::Arc;

use rhai_hir::{FunctionTypeRef, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;

use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectReferenceKind};

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
    assert_eq!(snapshot.global_functions().len(), 3);
    assert_eq!(snapshot.global_functions()[0].name, "blob");
    assert_eq!(snapshot.global_functions()[1].name, "timestamp");
    assert_eq!(snapshot.global_functions()[2].name, "Fn");
    assert_eq!(snapshot.global_functions()[0].overloads.len(), 3);
    assert_eq!(snapshot.external_signatures().get("math::parse"), None);
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
        Some(&TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::String),
        ))
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
        Some(&TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::Map(
                Box::new(TypeRef::String),
                Box::new(TypeRef::Union(vec![TypeRef::Int, TypeRef::String])),
            )),
        ))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, age),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, user),
        Some(&TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::Union(vec![TypeRef::Int, TypeRef::String])),
        ))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, indexed),
        Some(&TypeRef::Union(vec![TypeRef::Int, TypeRef::String]))
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

#[test]
fn snapshot_infers_host_method_member_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;
                let opener = widget.open;
                let opened = widget.open("home");
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "open".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(string) -> bool".to_owned(),
                            return_type: None,
                            docs: Some("Open the widget".to_owned()),
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
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let text = snapshot.file_text(file_id).expect("expected text");

    let opener = symbol_id_by_name(&hir, "opener", SymbolKind::Variable);
    let opened = symbol_id_by_name(&hir, "opened", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, opener),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Bool),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, opened),
        Some(&TypeRef::Bool)
    );

    let field_offset = offset_in(&text, "open;") + TextSize::from(1);
    assert_eq!(
        snapshot.inferred_expr_type_at(file_id, field_offset),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Bool),
        }))
    );
}

#[test]
fn snapshot_prefers_host_method_overload_matching_argument_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;
                let opened_by_name = widget.open("home");
                let opened_by_id = widget.open(1);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "open".to_owned(),
                        vec![
                            rhai_project::FunctionSpec {
                                signature: "fun(int) -> int".to_owned(),
                                return_type: None,
                                docs: Some("Open by id".to_owned()),
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by route".to_owned()),
                            },
                        ],
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
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let opened_by_name = symbol_id_by_name(&hir, "opened_by_name", SymbolKind::Variable);
    let opened_by_id = symbol_id_by_name(&hir, "opened_by_id", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, opened_by_name),
        Some(&TypeRef::Bool)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, opened_by_id),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_propagates_exported_types_into_import_aliases() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let shared = blob(10);
                    export shared as shared_value;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_value as value;

                    let copy = value;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_hir = snapshot.hir(provider).expect("expected provider hir");
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");

    let shared = symbol_id_by_name(&provider_hir, "shared", SymbolKind::Variable);
    let value = symbol_id_by_name(&consumer_hir, "value", SymbolKind::ImportAlias);
    let copy = symbol_id_by_name(&consumer_hir, "copy", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(provider, shared),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, copy),
        Some(&TypeRef::Blob)
    );
}

#[test]
fn snapshot_propagates_workspace_call_argument_types_across_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn echo(value) {
                        value
                    }

                    export echo as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

                    let result = tools(blob(10));
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_hir = snapshot.hir(provider).expect("expected provider hir");
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");

    let echo = symbol_id_by_name(&provider_hir, "echo", SymbolKind::Function);
    let value = symbol_id_by_name(&provider_hir, "value", SymbolKind::Parameter);
    let tools = symbol_id_by_name(&consumer_hir, "tools", SymbolKind::ImportAlias);
    let result = symbol_id_by_name(&consumer_hir, "result", SymbolKind::Variable);
    let propagated_signature = TypeRef::Function(FunctionTypeRef {
        params: vec![TypeRef::Blob],
        ret: Box::new(TypeRef::Blob),
    });

    assert_eq!(
        snapshot.inferred_symbol_type(provider, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(provider, echo),
        Some(&propagated_signature)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, tools),
        Some(&propagated_signature)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, result),
        Some(&TypeRef::Blob)
    );
}

#[test]
fn workspace_type_inference_refreshes_after_importer_call_changes() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn echo(value) {
                        value
                    }

                    export echo as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

                    let result = tools(blob(10));
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let provider = first_snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_hir = first_snapshot.hir(provider).expect("expected provider hir");
    let consumer_hir = first_snapshot.hir(consumer).expect("expected consumer hir");
    let value = symbol_id_by_name(&provider_hir, "value", SymbolKind::Parameter);
    let result = symbol_id_by_name(&consumer_hir, "result", SymbolKind::Variable);

    assert_eq!(
        first_snapshot.inferred_symbol_type(provider, value),
        Some(&TypeRef::Blob)
    );
    assert_eq!(
        first_snapshot.inferred_symbol_type(consumer, result),
        Some(&TypeRef::Blob)
    );

    db.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        r#"
            import shared_tools as tools;

            let result = tools("home");
        "#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let provider_hir = second_snapshot
        .hir(provider)
        .expect("expected provider hir");
    let consumer_hir = second_snapshot
        .hir(consumer)
        .expect("expected consumer hir");
    let echo = symbol_id_by_name(&provider_hir, "echo", SymbolKind::Function);
    let value = symbol_id_by_name(&provider_hir, "value", SymbolKind::Parameter);
    let tools = symbol_id_by_name(&consumer_hir, "tools", SymbolKind::ImportAlias);
    let result = symbol_id_by_name(&consumer_hir, "result", SymbolKind::Variable);
    let propagated_signature = TypeRef::Function(FunctionTypeRef {
        params: vec![TypeRef::String],
        ret: Box::new(TypeRef::String),
    });

    assert_eq!(
        second_snapshot.inferred_symbol_type(provider, value),
        Some(&TypeRef::String)
    );
    assert_eq!(
        second_snapshot.inferred_symbol_type(provider, echo),
        Some(&propagated_signature)
    );
    assert_eq!(
        second_snapshot.inferred_symbol_type(consumer, tools),
        Some(&propagated_signature)
    );
    assert_eq!(
        second_snapshot.inferred_symbol_type(consumer, result),
        Some(&TypeRef::String)
    );
}

#[test]
fn snapshot_exposes_cached_indexes() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn outer() {
                fn inner() {}
            }

            const LIMIT = 1;
            import "crypto" as secure;
            export outer as public_outer;
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let file_symbol_index = snapshot
        .file_symbol_index(file_id)
        .expect("expected file symbol index");
    let document_symbols = snapshot.document_symbols(file_id);
    let file_workspace_symbols = snapshot.file_workspace_symbols(file_id);
    let module_graph = snapshot
        .module_graph(file_id)
        .expect("expected module graph");

    assert_eq!(
        file_symbol_index.entries.len(),
        file_workspace_symbols.len()
    );
    assert_eq!(
        document_symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["outer", "LIMIT", "secure", "public_outer"]
    );
    assert_eq!(document_symbols[0].children.len(), 1);
    assert_eq!(document_symbols[0].children[0].name, "inner");
    assert_eq!(module_graph.imports.len(), 1);
    assert_eq!(module_graph.exports.len(), 1);
    assert_eq!(
        snapshot
            .workspace_symbols()
            .iter()
            .map(|symbol| (symbol.file_id, symbol.symbol.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (file_id, "LIMIT"),
            (file_id, "inner"),
            (file_id, "outer"),
            (file_id, "public_outer"),
            (file_id, "secure"),
        ]
    );
}

#[test]
fn snapshot_exposes_workspace_module_graphs_and_symbol_locations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn local_module() {} export local_module as public_api;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "import \"crypto\" as secure;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let one = snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");

    assert_eq!(
        snapshot
            .workspace_module_graphs()
            .iter()
            .map(|graph| (
                graph.file_id,
                graph.graph.imports.len(),
                graph.graph.exports.len()
            ))
            .collect::<Vec<_>>(),
        vec![(one, 0, 1), (two, 1, 0)]
    );

    let graph = snapshot.module_graph(one).expect("expected module graph");
    let target = graph.exports[0]
        .target
        .as_ref()
        .expect("expected exported target");
    let alias = graph.exports[0]
        .alias
        .as_ref()
        .expect("expected export alias");

    assert_eq!(snapshot.symbol_owner(target), Some(one));
    assert_eq!(snapshot.locate_symbol(target).len(), 1);
    assert_eq!(
        snapshot.locate_symbol(target)[0].symbol.name,
        "local_module"
    );

    assert_eq!(snapshot.symbol_owner(alias), Some(one));
    assert_eq!(snapshot.locate_symbol(alias).len(), 1);
    assert_eq!(snapshot.locate_symbol(alias)[0].symbol.name, "public_api");
}

#[test]
fn unchanged_files_reuse_cached_analysis_across_snapshots() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_parse = first_snapshot.parse(file_id).expect("expected parse");
    let first_hir = first_snapshot.hir(file_id).expect("expected hir");

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_parse = second_snapshot.parse(file_id).expect("expected parse");
    let second_hir = second_snapshot.hir(file_id).expect("expected hir");

    assert!(Arc::ptr_eq(&first_parse, &second_parse));
    assert!(Arc::ptr_eq(&first_hir, &second_hir));
}

#[test]
fn unchanged_files_reuse_cached_indexes_across_snapshots() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn sample() {}
            export sample as public_sample;
        "#,
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_index = first_snapshot
        .file_symbol_index(file_id)
        .expect("expected file symbol index");
    let first_module_graph = first_snapshot
        .module_graph(file_id)
        .expect("expected module graph");

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_index = second_snapshot
        .file_symbol_index(file_id)
        .expect("expected file symbol index");
    let second_module_graph = second_snapshot
        .module_graph(file_id)
        .expect("expected module graph");

    assert!(Arc::ptr_eq(&first_index, &second_index));
    assert!(Arc::ptr_eq(&first_module_graph, &second_module_graph));
}

#[test]
fn file_changes_reuse_project_semantics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: Vec::new(),
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

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let first_signatures = first_snapshot.external_signatures();
    let first_modules = first_snapshot.host_modules();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);

    assert!(ptr::eq(
        first_signatures,
        second_snapshot.external_signatures()
    ));
    assert!(ptr::eq(first_modules, second_snapshot.host_modules()));
}

#[test]
fn text_changes_invalidate_only_affected_file_analysis() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "let first = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "let second = 2;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let one = first_snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = first_snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");
    let first_one_parse = first_snapshot.parse(one).expect("expected parse");
    let first_two_parse = first_snapshot.parse(two).expect("expected parse");

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "let first = 10;",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_parse = second_snapshot.parse(one).expect("expected parse");
    let second_two_parse = second_snapshot.parse(two).expect("expected parse");

    assert!(!Arc::ptr_eq(&first_one_parse, &second_one_parse));
    assert!(Arc::ptr_eq(&first_two_parse, &second_two_parse));
}

#[test]
fn text_changes_invalidate_only_affected_file_indexes_and_refresh_workspace_symbols() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn alpha() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "fn beta() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let one = first_snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = first_snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");
    let first_one_index = first_snapshot
        .file_symbol_index(one)
        .expect("expected file symbol index");
    let first_two_index = first_snapshot
        .file_symbol_index(two)
        .expect("expected file symbol index");

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "fn gamma() {}",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_index = second_snapshot
        .file_symbol_index(one)
        .expect("expected file symbol index");
    let second_two_index = second_snapshot
        .file_symbol_index(two)
        .expect("expected file symbol index");

    assert!(!Arc::ptr_eq(&first_one_index, &second_one_index));
    assert!(Arc::ptr_eq(&first_two_index, &second_two_index));
    assert_eq!(
        second_snapshot
            .workspace_symbols()
            .iter()
            .map(|symbol| (symbol.file_id, symbol.symbol.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(two, "beta"), (one, "gamma")]
    );
}

#[test]
fn text_changes_refresh_workspace_module_graphs_and_symbol_locations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn alpha() {} export alpha as public_alpha;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_graph = first_snapshot
        .module_graph(file_id)
        .expect("expected module graph");
    let first_target = first_graph.exports[0]
        .target
        .as_ref()
        .expect("expected exported target")
        .clone();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "fn beta() {} export beta as public_beta;",
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_graph = second_snapshot
        .module_graph(file_id)
        .expect("expected module graph");
    let second_target = second_graph.exports[0]
        .target
        .as_ref()
        .expect("expected exported target")
        .clone();

    assert_eq!(second_snapshot.workspace_module_graphs().len(), 1);
    assert!(second_snapshot.symbol_owner(&first_target).is_none());
    assert_eq!(second_snapshot.symbol_owner(&second_target), Some(file_id));
    assert_eq!(
        second_snapshot.locate_symbol(&second_target)[0].symbol.name,
        "beta"
    );
}

#[test]
fn workspace_index_invalidation_updates_only_changed_file_contributions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn alpha() {} export alpha as public_alpha;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "fn beta() {} export beta as public_beta;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let one = first_snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = first_snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");

    let first_one_symbols = Arc::clone(
        db.workspace_indexes
            .symbols_by_file
            .get(&one)
            .expect("expected one.rhai symbols"),
    );
    let first_two_symbols = Arc::clone(
        db.workspace_indexes
            .symbols_by_file
            .get(&two)
            .expect("expected two.rhai symbols"),
    );
    let first_one_graph = Arc::clone(
        db.workspace_indexes
            .module_graphs_by_file
            .get(&one)
            .expect("expected one.rhai module graph"),
    );
    let first_two_graph = Arc::clone(
        db.workspace_indexes
            .module_graphs_by_file
            .get(&two)
            .expect("expected two.rhai module graph"),
    );
    let first_one_locations = Arc::clone(
        db.workspace_indexes
            .symbol_locations_by_file
            .get(&one)
            .expect("expected one.rhai symbol locations"),
    );
    let first_two_locations = Arc::clone(
        db.workspace_indexes
            .symbol_locations_by_file
            .get(&two)
            .expect("expected two.rhai symbol locations"),
    );

    db.apply_change(ChangeSet::single_file(
        "one.rhai",
        "fn gamma() {} export gamma as public_gamma;",
        DocumentVersion(2),
    ));

    let second_one_symbols = db
        .workspace_indexes
        .symbols_by_file
        .get(&one)
        .expect("expected one.rhai symbols");
    let second_two_symbols = db
        .workspace_indexes
        .symbols_by_file
        .get(&two)
        .expect("expected two.rhai symbols");
    let second_one_graph = db
        .workspace_indexes
        .module_graphs_by_file
        .get(&one)
        .expect("expected one.rhai module graph");
    let second_two_graph = db
        .workspace_indexes
        .module_graphs_by_file
        .get(&two)
        .expect("expected two.rhai module graph");
    let second_one_locations = db
        .workspace_indexes
        .symbol_locations_by_file
        .get(&one)
        .expect("expected one.rhai symbol locations");
    let second_two_locations = db
        .workspace_indexes
        .symbol_locations_by_file
        .get(&two)
        .expect("expected two.rhai symbol locations");

    assert!(!Arc::ptr_eq(&first_one_symbols, second_one_symbols));
    assert!(Arc::ptr_eq(&first_two_symbols, second_two_symbols));
    assert!(!Arc::ptr_eq(&first_one_graph, second_one_graph));
    assert!(Arc::ptr_eq(&first_two_graph, second_two_graph));
    assert!(!Arc::ptr_eq(&first_one_locations, second_one_locations));
    assert!(Arc::ptr_eq(&first_two_locations, second_two_locations));
}

#[test]
fn project_changes_refresh_project_semantics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: Vec::new(),
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

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let first_signatures = first_snapshot.external_signatures();
    assert!(first_signatures.get("math::add").is_some());

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            engine: rhai_project::EngineOptions {
                disabled_symbols: vec!["spawn".to_owned()],
                custom_syntaxes: vec!["unless".to_owned()],
            },
            modules: [(
                "io".to_owned(),
                rhai_project::ModuleSpec {
                    docs: None,
                    functions: [(
                        "read".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(string) -> string".to_owned(),
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

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(!ptr::eq(
        first_signatures,
        second_snapshot.external_signatures()
    ));
    assert_eq!(second_snapshot.disabled_symbols(), ["spawn"]);
    assert_eq!(second_snapshot.custom_syntaxes(), ["unless"]);
    assert!(
        second_snapshot
            .external_signatures()
            .get("math::add")
            .is_none()
    );
    assert!(
        second_snapshot
            .external_signatures()
            .get("io::read")
            .is_some()
    );
}

#[test]
fn project_changes_invalidate_all_cached_file_analysis() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "let first = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "let second = 2;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let one = first_snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = first_snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");
    let first_one_hir = first_snapshot.hir(one).expect("expected hir");
    let first_two_hir = first_snapshot.hir(two).expect("expected hir");

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            ..ProjectConfig::default()
        }),
    });

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_one_hir = second_snapshot.hir(one).expect("expected hir");
    let second_two_hir = second_snapshot.hir(two).expect("expected hir");

    assert!(!Arc::ptr_eq(&first_one_hir, &second_one_hir));
    assert!(!Arc::ptr_eq(&first_two_hir, &second_two_hir));
}

#[test]
fn stale_or_identical_file_changes_do_not_rebuild_analysis() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(2),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    let first_parse = first_snapshot.parse(file_id).expect("expected parse");

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(2),
    ));
    let identical_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&identical_snapshot);
    let identical_parse = identical_snapshot.parse(file_id).expect("expected parse");
    assert!(Arc::ptr_eq(&first_parse, &identical_parse));

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 0;",
        DocumentVersion(1),
    ));
    let stale_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&stale_snapshot);
    let stale_parse = stale_snapshot.parse(file_id).expect("expected parse");
    assert!(Arc::ptr_eq(&first_parse, &stale_parse));
}

#[test]
fn workspace_symbol_search_supports_project_wide_queries() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "alpha.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as public_helper;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "beta.rhai".into(),
                text: r#"
                    fn helper_tool() {}
                    fn Worker() { helper_tool(); }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let helper_matches = snapshot.workspace_symbols_matching("helper");
    assert_eq!(
        helper_matches
            .iter()
            .map(|symbol| (
                symbol.file_id,
                symbol.symbol.name.as_str(),
                symbol.symbol.exported
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "helper",
                true,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("beta.rhai"))
                    .expect("expected beta.rhai"),
                "helper_tool",
                false,
            ),
            (
                snapshot
                    .vfs()
                    .file_id(Path::new("alpha.rhai"))
                    .expect("expected alpha.rhai"),
                "public_helper",
                true,
            ),
        ]
    );

    let worker_matches = snapshot.workspace_symbols_matching("worker");
    assert_eq!(worker_matches.len(), 1);
    assert_eq!(worker_matches[0].symbol.name, "Worker");
}

#[test]
fn completion_inputs_collect_visible_member_and_project_symbols() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn run() {
                        let user = #{ name: "Ada", id: 42 };
                        let local_value = 1;
                        user.
                        helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn shared_helper() {}
                    fn project_only() {}
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let main = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let main_text = snapshot.file_text(main).expect("expected main text");

    let helper_offset = offset_in(&main_text, "helper();");
    let helper_inputs = snapshot
        .completion_inputs(main, helper_offset)
        .expect("expected completion inputs");
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );
    assert!(
        helper_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "local_value")
    );
    assert!(
        helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "shared_helper")
    );
    assert!(
        !helper_inputs
            .project_symbols
            .iter()
            .any(|symbol| symbol.symbol.name == "helper")
    );

    let member_offset = offset_in(&main_text, "user.");
    let member_inputs = snapshot
        .completion_inputs(main, member_offset)
        .expect("expected member completion inputs");
    assert!(
        !member_inputs.member_symbols.is_empty(),
        "expected member completions for object literal fields"
    );
    assert!(
        member_inputs
            .member_symbols
            .iter()
            .any(|member| member.name == "name")
            || member_inputs
                .member_symbols
                .iter()
                .any(|member| member.name == "id")
    );
}

#[test]
fn query_support_can_be_warmed_for_completion_and_navigation_queries() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}

            fn run() {
                let user = #{ name: "Ada", id: 42 };
                user.
                helper();
            }
        "#,
        DocumentVersion(1),
    ));

    let cold_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&cold_snapshot);
    let file_id = cold_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    assert!(cold_snapshot.query_support(file_id).is_none());

    assert_eq!(db.warm_query_support(&[file_id]), 1);
    let warm_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warm_snapshot);
    let query_support = warm_snapshot
        .query_support(file_id)
        .expect("expected warmed query support");
    assert!(
        query_support
            .completion_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );
    assert!(
        query_support
            .navigation_targets
            .iter()
            .any(|target| target.symbol.name == "helper")
    );
    assert!(
        query_support
            .member_completion_sets
            .iter()
            .any(|set| set.symbol.name == "user"
                && set.members.iter().any(|member| member.name == "name"))
    );
    assert_eq!(warm_snapshot.stats().query_support_rebuilds, 1);

    let main_text = warm_snapshot
        .file_text(file_id)
        .expect("expected main text");
    let completion_inputs = warm_snapshot
        .completion_inputs(file_id, offset_in(&main_text, "helper();"))
        .expect("expected completion inputs");
    assert!(
        completion_inputs
            .visible_symbols
            .iter()
            .any(|symbol| symbol.name == "helper")
    );

    assert_eq!(db.warm_workspace_queries(), 0);
}

#[test]
fn builtin_global_functions_suppress_unresolved_name_diagnostics() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}
            let _bytes = blob(10);
            let _now = timestamp();
            let _callback = Fn("helper");
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin blob call to avoid unresolved-name diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn import_export_linkage_supports_cross_file_navigation() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

                    fn run() {
                        tools();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    assert_eq!(snapshot.workspace_exports().len(), 1);
    assert_eq!(snapshot.exports_named("shared_tools").len(), 1);
    assert_eq!(snapshot.exports_named("shared_tools")[0].file_id, provider);

    let linked_imports = snapshot.linked_imports(consumer);
    assert_eq!(linked_imports.len(), 1);
    assert_eq!(linked_imports[0].module_name, "shared_tools");
    assert_eq!(linked_imports[0].exports.len(), 1);
    assert_eq!(linked_imports[0].exports[0].file_id, provider);

    let definition_targets =
        snapshot.goto_definition(consumer, offset_in(&consumer_text, "shared_tools"));
    assert_eq!(definition_targets.len(), 1);
    assert_eq!(definition_targets[0].file_id, provider);
    assert_eq!(
        slice_range(
            &snapshot
                .file_text(provider)
                .expect("expected provider text"),
            definition_targets[0].target.focus_range,
        ),
        "shared_tools"
    );
}

#[test]
fn changing_exports_refreshes_import_linkage() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import shared_tools as tools;"#.to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);
    assert_eq!(first_snapshot.exports_named("shared_tools").len(), 1);

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        r#"
            fn helper() {}
            export helper as renamed_tools;
        "#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(second_snapshot.linked_imports(consumer).is_empty());
    assert!(second_snapshot.exports_named("shared_tools").is_empty());
    assert_eq!(second_snapshot.exports_named("renamed_tools").len(), 1);
}

#[test]
fn workspace_dependency_graph_tracks_importers_and_exporters() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(
        snapshot
            .workspace_dependency_graph()
            .edges
            .iter()
            .map(|edge| (
                edge.importer_file_id,
                edge.exporter_file_id,
                edge.module_name.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(consumer, provider, "shared_tools")]
    );
    assert_eq!(snapshot.dependency_files(consumer), [provider]);
    assert_eq!(snapshot.dependent_files(provider), [consumer]);
}

#[test]
fn project_diagnostics_suppress_false_unresolved_imports_when_workspace_export_exists() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;\n\nfn run() { tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert!(snapshot.project_diagnostics(consumer).is_empty());
}

#[test]
fn project_diagnostics_surface_broken_linked_import_usage_when_export_disappears() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;\n\nfn run() { tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    db.apply_change(ChangeSet::single_file(
        "provider.rhai",
        "fn helper() {} export helper as renamed_tools;",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let diagnostics = snapshot.project_diagnostics(consumer);

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message == "unresolved import module `shared_tools`" })
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("import alias no longer resolves")
    }));
}

#[test]
fn change_report_surfaces_dependency_affected_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let impact = db.apply_change_report(ChangeSet::single_file(
        "provider.rhai",
        "fn helper() {} export helper as renamed_tools;",
        DocumentVersion(2),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");

    assert_eq!(impact.changed_files, vec![provider]);
    assert_eq!(impact.rebuilt_files, vec![provider]);
    assert_eq!(impact.dependency_affected_files, vec![consumer]);
}

#[test]
fn project_find_references_include_linked_imports_for_exported_names() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

                    fn run() {
                        tools();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let references = snapshot
        .find_references(consumer, offset_in(&consumer_text, "shared_tools"))
        .expect("expected project references");

    assert_eq!(references.targets.len(), 1);
    assert_eq!(references.targets[0].symbol.name, "shared_tools");
    let mut reference_kinds = references
        .references
        .iter()
        .map(|reference| (reference.file_id, reference.kind))
        .collect::<Vec<_>>();
    reference_kinds.sort_by_key(|(file_id, kind)| (file_id.0, *kind as u8));
    assert_eq!(
        reference_kinds,
        vec![
            (consumer, ProjectReferenceKind::LinkedImport),
            (
                references.targets[0].file_id,
                ProjectReferenceKind::Definition
            ),
        ]
    );
}

#[test]
fn auto_import_candidates_plan_imports_for_unresolved_workspace_exports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_tools(); }".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let candidates =
        snapshot.auto_import_candidates(consumer, offset_in(&consumer_text, "shared_tools"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].module_name, "shared_tools");
    assert_eq!(candidates[0].alias, "shared_tools");
    assert_eq!(candidates[0].insertion_offset, TextSize::from(0));
    assert_eq!(
        candidates[0].insert_text,
        "import shared_tools as shared_tools;\n"
    );
}

#[test]
fn project_rename_plan_tracks_cross_file_import_occurrences_without_renaming_internal_target() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {
                        helper();
                    }

                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import shared_tools as tools;

                    fn run() {
                        tools();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider_text = snapshot
        .file_text(provider)
        .expect("expected provider text");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let export_plan = snapshot
        .rename_plan(
            consumer,
            offset_in(&consumer_text, "shared_tools"),
            "renamed_tools",
        )
        .expect("expected project rename plan");
    assert_eq!(export_plan.targets.len(), 1);
    assert_eq!(export_plan.targets[0].symbol.name, "shared_tools");
    assert_eq!(export_plan.occurrences.len(), 2);
    assert!(export_plan.occurrences.iter().any(|occurrence| {
        occurrence.file_id == provider && occurrence.kind == ProjectReferenceKind::Definition
    }));
    assert!(export_plan.occurrences.iter().any(|occurrence| {
        occurrence.file_id == consumer && occurrence.kind == ProjectReferenceKind::LinkedImport
    }));

    let helper_plan = snapshot
        .rename_plan(
            provider,
            offset_in(&provider_text, "helper"),
            "renamed_helper",
        )
        .expect("expected helper rename plan");
    assert_eq!(helper_plan.targets.len(), 1);
    assert_eq!(helper_plan.targets[0].symbol.name, "helper");
    assert!(helper_plan.occurrences.iter().any(|occurrence| {
        occurrence.file_id == provider && occurrence.kind == ProjectReferenceKind::Definition
    }));
    assert!(helper_plan.occurrences.iter().any(|occurrence| {
        occurrence.file_id == provider && occurrence.kind == ProjectReferenceKind::Reference
    }));
    assert!(!helper_plan.occurrences.iter().any(|occurrence| {
        occurrence.file_id == consumer && occurrence.kind == ProjectReferenceKind::LinkedImport
    }));
}

#[test]
fn project_rename_plan_reports_cross_file_export_collisions_before_renaming() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as shared_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "existing.rhai".into(),
                text: r#"
                    fn helper() {}
                    export helper as renamed_tools;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let consumer_text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");

    let plan = snapshot
        .rename_plan(
            consumer,
            offset_in(&consumer_text, "shared_tools"),
            "renamed_tools",
        )
        .expect("expected rename plan");

    assert!(plan.issues.iter().any(|issue| {
        issue.file_id == provider
            && issue
                .issue
                .message
                .contains("collide with another workspace export")
    }));
    assert!(plan.issues.iter().any(|issue| {
        issue.file_id == consumer && issue.issue.message.contains("linked import ambiguous")
    }));
}

#[test]
fn snapshot_tracks_source_roots_workspace_membership_and_normalized_paths() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "workspace/src/./main.rhai".into(),
                text: "fn main() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/scripts/../scripts/tool.rhai".into(),
                text: "fn tool() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/tests/test.rhai".into(),
                text: "fn test_case() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            source_roots: vec!["src".into(), "scripts/../scripts".into()],
            ..ProjectConfig::default()
        }),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let main = snapshot
        .vfs()
        .file_id(Path::new("workspace/src/main.rhai"))
        .expect("expected main.rhai");
    let tool = snapshot
        .vfs()
        .file_id(Path::new("workspace/scripts/tool.rhai"))
        .expect("expected tool.rhai");
    let test = snapshot
        .vfs()
        .file_id(Path::new("workspace/tests/test.rhai"))
        .expect("expected test.rhai");

    assert_eq!(
        snapshot.source_root_paths(),
        vec![
            Path::new("workspace/scripts").to_path_buf(),
            Path::new("workspace/src").to_path_buf(),
        ]
    );
    assert_eq!(
        snapshot.normalized_path(main),
        Some(Path::new("workspace/src/main.rhai"))
    );
    assert_eq!(
        snapshot.normalized_path(tool),
        Some(Path::new("workspace/scripts/tool.rhai"))
    );
    assert_eq!(
        snapshot.normalized_path(test),
        Some(Path::new("workspace/tests/test.rhai"))
    );
    assert!(snapshot.is_workspace_file(main));
    assert!(snapshot.is_workspace_file(tool));
    assert!(!snapshot.is_workspace_file(test));

    let workspace_files = snapshot.workspace_files();
    assert_eq!(workspace_files.len(), 3);
    assert!(workspace_files.iter().any(|file| {
        file.file_id == main
            && file.source_root == snapshot.source_root_index(main)
            && file.is_workspace_file
    }));
    assert!(workspace_files.iter().any(|file| {
        file.file_id == test && file.source_root.is_none() && !file.is_workspace_file
    }));
}

#[test]
fn analysis_dependencies_track_text_and_project_inputs() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "src/./main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let file_id = first_snapshot
        .vfs()
        .file_id(Path::new("src/main.rhai"))
        .expect("expected file id");
    let first_dependencies = first_snapshot
        .analysis_dependencies(file_id)
        .expect("expected analysis dependencies");

    assert_eq!(
        first_dependencies.parse.normalized_path,
        Path::new("src/main.rhai")
    );
    assert_eq!(
        first_dependencies.parse.document_version,
        DocumentVersion(1)
    );
    assert_eq!(first_dependencies.hir.project_revision, 0);
    assert_eq!(
        first_dependencies.last_invalidation,
        crate::InvalidationReason::InitialLoad
    );

    db.apply_change(ChangeSet {
        files: Vec::new(),
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            root: "workspace".into(),
            ..ProjectConfig::default()
        }),
    });

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    let second_dependencies = second_snapshot
        .analysis_dependencies(file_id)
        .expect("expected analysis dependencies");

    assert_eq!(second_snapshot.project_revision(), 1);
    assert_eq!(second_dependencies.hir.project_revision, 1);
    assert_eq!(second_dependencies.index.project_revision, 1);
    assert_eq!(
        second_dependencies.last_invalidation,
        crate::InvalidationReason::ProjectChanged
    );
}

#[test]
fn removing_files_unloads_cached_analysis_and_updates_workspace_links() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {} export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: "import shared_tools as tools;".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    let provider = first_snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = first_snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_eq!(first_snapshot.exports_named("shared_tools").len(), 1);
    assert_eq!(first_snapshot.linked_imports(consumer).len(), 1);

    db.apply_change(ChangeSet::remove_file("provider.rhai"));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(second_snapshot.file_text(provider).is_none());
    assert!(second_snapshot.parse(provider).is_none());
    assert!(second_snapshot.hir(provider).is_none());
    assert!(second_snapshot.module_graph(provider).is_none());
    assert!(second_snapshot.exports_named("shared_tools").is_empty());
    assert!(second_snapshot.linked_imports(consumer).is_empty());
}

#[test]
fn revision_stats_and_debug_view_surface_cache_activity() {
    let mut db = AnalyzerDatabase::default();
    let initial_revision = db.snapshot().revision();

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));
    let first_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&first_snapshot);
    assert_eq!(first_snapshot.revision(), initial_revision + 1);
    assert_eq!(first_snapshot.stats().parse_rebuilds, 1);
    assert_eq!(first_snapshot.stats().lower_rebuilds, 1);
    assert_eq!(first_snapshot.stats().index_rebuilds, 1);

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 1;",
        DocumentVersion(1),
    ));
    let no_op_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&no_op_snapshot);
    assert_eq!(no_op_snapshot.revision(), first_snapshot.revision());

    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        "let value = 2;",
        DocumentVersion(2),
    ));
    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert_eq!(second_snapshot.revision(), first_snapshot.revision() + 1);

    let file_id = second_snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");
    assert_eq!(db.warm_query_support(&[file_id]), 1);
    let warmed_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warmed_snapshot);
    assert_eq!(warmed_snapshot.stats().query_support_rebuilds, 1);

    let debug_view = warmed_snapshot.debug_view();
    assert_eq!(debug_view.revision, warmed_snapshot.revision());
    assert_eq!(debug_view.files.len(), 1);
    assert_eq!(debug_view.files[0].normalized_path, Path::new("main.rhai"));
    assert_eq!(debug_view.files[0].document_version, DocumentVersion(2));
    assert_eq!(debug_view.files[0].stats.file_id, file_id);
    assert_eq!(debug_view.files[0].stats.query_support_rebuilds, 1);
    assert!(debug_view.stats.total_parse_time >= std::time::Duration::ZERO);
    assert!(debug_view.stats.total_query_support_time >= std::time::Duration::ZERO);
}

#[test]
fn batched_high_frequency_updates_rebuild_each_file_once() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "workspace/src/./main.rhai".into(),
                text: "let value = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "workspace/src/main.rhai".into(),
                text: "let value = 2;".to_owned(),
                version: DocumentVersion(2),
            },
            FileChange {
                path: "workspace/src/main.rhai".into(),
                text: "let value = 3;".to_owned(),
                version: DocumentVersion(3),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("workspace/src/main.rhai"))
        .expect("expected file id");
    assert_eq!(
        snapshot.file_text(file_id).as_deref(),
        Some("let value = 3;")
    );
    assert_eq!(snapshot.stats().parse_rebuilds, 1);
    assert_eq!(snapshot.stats().lower_rebuilds, 1);
    assert_eq!(snapshot.stats().index_rebuilds, 1);
}

#[test]
fn query_support_budget_evicts_cold_cached_files_and_updates_file_stats() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "one.rhai".into(),
                text: "fn one() {}".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "two.rhai".into(),
                text: "fn two() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let one = snapshot
        .vfs()
        .file_id(Path::new("one.rhai"))
        .expect("expected one.rhai");
    let two = snapshot
        .vfs()
        .file_id(Path::new("two.rhai"))
        .expect("expected two.rhai");

    assert_eq!(
        db.set_query_support_budget(Some(1)),
        Vec::<rhai_vfs::FileId>::new()
    );
    assert_eq!(db.warm_query_support(&[one]), 1);
    assert_eq!(db.warm_query_support(&[two]), 1);

    let warmed_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&warmed_snapshot);
    assert!(warmed_snapshot.query_support(one).is_none());
    assert!(warmed_snapshot.query_support(two).is_some());
    assert_eq!(warmed_snapshot.stats().query_support_evictions, 1);
    assert_eq!(
        warmed_snapshot
            .file_stats(one)
            .expect("expected one.rhai stats")
            .query_support_evictions,
        1
    );
    assert!(
        !warmed_snapshot
            .file_stats(one)
            .expect("expected one.rhai stats")
            .query_support_cached
    );
    assert!(
        warmed_snapshot
            .file_stats(two)
            .expect("expected two.rhai stats")
            .query_support_cached
    );
}

fn offset_in(text: &str, needle: &str) -> TextSize {
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("expected to find `{needle}` in:\n{text}"));
    TextSize::from(u32::try_from(offset).expect("expected offset to fit into u32"))
}

fn slice_range(text: &str, range: rhai_syntax::TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn assert_workspace_files_have_no_syntax_diagnostics(snapshot: &crate::DatabaseSnapshot) {
    for file in snapshot.workspace_files() {
        let diagnostics = snapshot.syntax_diagnostics(file.file_id);
        assert!(
            diagnostics.is_empty(),
            "expected valid Rhai syntax for {:?}, got diagnostics: {:?}",
            snapshot.normalized_path(file.file_id),
            diagnostics
        );
    }
}

fn symbol_id_by_name(hir: &rhai_hir::FileHir, name: &str, kind: SymbolKind) -> rhai_hir::SymbolId {
    let index = hir
        .symbols
        .iter()
        .position(|symbol| symbol.name == name && symbol.kind == kind)
        .unwrap_or_else(|| panic!("expected symbol `{name}` with kind {kind:?}"));
    rhai_hir::SymbolId(index as u32)
}
