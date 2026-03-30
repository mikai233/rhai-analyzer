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
fn snapshot_infers_receiver_specialized_generic_host_method_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Box<int>
                let boxed = unknown_box;
                let peek_fn = boxed.peek;
                let value = boxed.peek();
                let unwrap_or_fn = boxed.unwrap_or;
                let fallback = boxed.unwrap_or(1);
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Box<T>".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [
                        (
                            "peek".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun() -> T".to_owned(),
                                return_type: None,
                                docs: Some("Peek at the boxed value".to_owned()),
                            }],
                        ),
                        (
                            "unwrap_or".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun(T) -> T".to_owned(),
                                return_type: None,
                                docs: Some("Return the boxed value or a fallback".to_owned()),
                            }],
                        ),
                    ]
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

    let peek_fn = symbol_id_by_name(&hir, "peek_fn", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let unwrap_or_fn = symbol_id_by_name(&hir, "unwrap_or_fn", SymbolKind::Variable);
    let fallback = symbol_id_by_name(&hir, "fallback", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, peek_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, unwrap_or_fn),
        Some(&TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, fallback),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_infers_local_script_method_calls_and_this_types() {
    let mut db = AnalyzerDatabase::default();
    let source = r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            let value = 40;
            let result = value.bump(2);
        "#;
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        source,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);
    let this_offset =
        TextSize::from(u32::try_from(source.find("this +").expect("expected this")).unwrap());

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_expr_type_at(file_id, this_offset),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

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

#[test]
fn snapshot_prefers_typed_script_methods_over_blanket_methods() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            /// @param delta string
            /// @return string
            fn bump(delta) {
                delta
            }

            let value = 40;
            let result = value.bump(2);
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
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}

#[test]
fn snapshot_tracks_ambiguous_host_method_overload_results() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @type Widget
                let widget = unknown_widget;
                let seed = if flag { 1 } else { "home" };
                let result = widget.open(seed);
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
                                docs: None,
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: None,
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
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Ambiguous(vec![TypeRef::Int, TypeRef::Bool]))
    );
}
