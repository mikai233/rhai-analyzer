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
