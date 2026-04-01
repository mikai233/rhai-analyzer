use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn snapshot_infers_types_from_mixed_member_and_index_mutations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let root = #{};
            let slot = 0;
            root.items[slot].value = 1;
            root.items[slot].value += 2;

            let items = root.items;
            let entry = root.items[slot];
            let value = root.items[slot].value;
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
    let items = symbol_id_by_name(&hir, "items", SymbolKind::Variable);
    let entry = symbol_id_by_name(&hir, "entry", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, root),
        Some(&TypeRef::Object(BTreeMap::from([(
            "items".to_owned(),
            TypeRef::Array(Box::new(TypeRef::Object(BTreeMap::from([(
                "value".to_owned(),
                TypeRef::Int,
            )])))),
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, items),
        Some(&TypeRef::Array(Box::new(TypeRef::Object(BTreeMap::from(
            [("value".to_owned(), TypeRef::Int,)]
        )))))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, entry),
        Some(&TypeRef::Object(BTreeMap::from([(
            "value".to_owned(),
            TypeRef::Int,
        )])))
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
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
