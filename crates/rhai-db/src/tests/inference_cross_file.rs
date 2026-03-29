use crate::tests::{
    assert_workspace_files_have_no_syntax_diagnostics, offset_in, symbol_id_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet, FileChange};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn snapshot_resolves_imported_typed_methods_as_global_method_targets() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @param delta int
                    /// @return int
                    fn int.bump(delta) {
                        this + delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider";

                    fn run() {
                        let value = 1;
                        let result = value.bump(2);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");
    let imported = snapshot.imported_global_method_symbols(consumer, &TypeRef::Int, "bump");
    let result = symbol_id_by_name(&consumer_hir, "result", SymbolKind::Variable);

    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].file_id, provider);
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, result),
        Some(&TypeRef::Int)
    );

    let text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let method_offset = offset_in(&text, "bump(2)") + TextSize::from(1);
    let definitions = snapshot.goto_definition(consumer, method_offset);

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].file_id, provider);
    let provider_hir = snapshot.hir(provider).expect("expected provider hir");
    assert_eq!(
        provider_hir.symbol(definitions[0].target.symbol).name,
        "bump"
    );
}

#[test]
fn snapshot_keeps_unaliased_imports_from_exposing_regular_module_members() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    export const VALUE = 1;

                    fn helper(value) {
                        value
                    }

                    fn int.bump(delta) {
                        this + delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider";

                    fn run() {
                        let value = 1;
                        let direct = helper(1);
                        let constant = VALUE;
                        let method = value.bump(2);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let diagnostics = snapshot.project_diagnostics(consumer);
    let imported = snapshot.imported_global_method_symbols(consumer, &TypeRef::Int, "bump");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `helper`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved name `VALUE`")
    );
    assert_eq!(imported.len(), 1);
}

#[test]
fn snapshot_infers_module_qualified_import_member_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @return int
                    fn helper() {
                        1
                    }

                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        let fn_result = tools::helper();
                        let value = tools::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");

    let fn_result = symbol_id_by_name(&consumer_hir, "fn_result", SymbolKind::Variable);
    let value = symbol_id_by_name(&consumer_hir, "value", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(consumer, fn_result),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, value),
        Some(&TypeRef::Int)
    );

    let text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let helper_offset = offset_in(&text, "helper()");
    let value_offset = offset_in(&text, "VALUE");

    let helper_definitions = snapshot.goto_definition(consumer, helper_offset);
    let value_definitions = snapshot.goto_definition(consumer, value_offset);

    assert_eq!(helper_definitions.len(), 1);
    assert_eq!(helper_definitions[0].file_id, provider);
    assert_eq!(value_definitions.len(), 1);
    assert_eq!(value_definitions[0].file_id, provider);
}

#[test]
fn snapshot_infers_nested_module_qualified_import_member_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "sub.rhai".into(),
                text: r#"
                    /// @param value int
                    /// @return int
                    fn helper(value) {
                        value
                    }

                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    import "sub" as sub;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        let fn_result = tools::sub::helper(1);
                        let value = tools::sub::VALUE;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(ProjectConfig::default()),
    });

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let sub = snapshot
        .vfs()
        .file_id(Path::new("sub.rhai"))
        .expect("expected sub.rhai");
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");

    let fn_result = symbol_id_by_name(&consumer_hir, "fn_result", SymbolKind::Variable);
    let value = symbol_id_by_name(&consumer_hir, "value", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(consumer, fn_result),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(consumer, value),
        Some(&TypeRef::Int)
    );

    let text = snapshot
        .file_text(consumer)
        .expect("expected consumer text");
    let helper_offset = offset_in(&text, "helper(1)");
    let value_offset = offset_in(&text, "VALUE");

    let helper_definitions = snapshot.goto_definition(consumer, helper_offset);
    let value_definitions = snapshot.goto_definition(consumer, value_offset);

    assert_eq!(helper_definitions.len(), 1);
    assert_eq!(helper_definitions[0].file_id, sub);
    assert_eq!(value_definitions.len(), 1);
    assert_eq!(value_definitions[0].file_id, sub);
}

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

#[test]
fn snapshot_reports_unresolved_bare_import_module_names() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        r#"
            import shared_tools as tools;
        "#,
        DocumentVersion(1),
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
            .any(|diagnostic| diagnostic.message == "unresolved import module `shared_tools`")
    );
}

#[test]
fn workspace_dependency_graph_tracks_static_string_imports() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;
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

    let linked_imports = snapshot.linked_imports(consumer);
    assert_eq!(linked_imports.len(), 1);
    assert_eq!(linked_imports[0].module_name, "provider");
    assert!(
        linked_imports[0]
            .exports
            .iter()
            .all(|export| export.file_id == provider)
    );

    let dependency_edges = snapshot
        .workspace_dependency_graph()
        .edges
        .iter()
        .map(|edge| {
            (
                edge.importer_file_id,
                edge.exporter_file_id,
                edge.module_name.as_str(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(dependency_edges, [(consumer, provider, "provider")].into());
    assert_eq!(snapshot.dependency_files(consumer), [provider]);
    assert_eq!(snapshot.dependent_files(provider), [consumer]);
}

#[test]
fn changing_consumer_import_paths_refreshes_workspace_linkage() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"import "provider" as tools;"#.to_owned(),
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

    db.apply_change(ChangeSet::single_file(
        "consumer.rhai",
        r#"import "missing" as tools;"#,
        DocumentVersion(2),
    ));

    let second_snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&second_snapshot);
    assert!(second_snapshot.linked_imports(consumer).is_empty());
}

#[test]
fn snapshot_exposes_cached_indexes() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn outer() {}
            fn helper() {}

            const LIMIT = 1;
            let exported_limit = LIMIT;
            import "crypto" as secure;
            export exported_limit as public_outer;
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
        vec![
            "outer",
            "helper",
            "LIMIT",
            "exported_limit",
            "secure",
            "public_outer"
        ]
    );
    assert!(document_symbols[0].children.is_empty());
    assert!(document_symbols[1].children.is_empty());
    assert_eq!(module_graph.imports.len(), 1);
    assert_eq!(module_graph.exports.len(), 3);
    assert_eq!(
        snapshot
            .workspace_symbols()
            .iter()
            .map(|symbol| (symbol.file_id, symbol.symbol.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (file_id, "LIMIT"),
            (file_id, "exported_limit"),
            (file_id, "helper"),
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
                text: "let local_module = 1; export local_module as public_api;".to_owned(),
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
