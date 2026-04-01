use crate::tests::{
    assert_workspace_files_have_no_syntax_diagnostics, offset_in, symbol_id_by_name,
};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectDiagnosticCode};
use rhai_hir::{SemanticDiagnosticCode, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;
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
fn snapshot_tracks_ambiguous_imported_typed_method_results() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider_a.rhai".into(),
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
                path: "provider_b.rhai".into(),
                text: r#"
                    /// @param delta string
                    /// @return string
                    fn int.bump(delta) {
                        delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider_a";
                    import "provider_b";

                    fn run() {
                        let value = 1;
                        let seed = if flag { 1 } else { "ok" };
                        let result = value.bump(seed);
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
    let consumer_hir = snapshot.hir(consumer).expect("expected consumer hir");
    let result = symbol_id_by_name(&consumer_hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(consumer, result),
        Some(&TypeRef::Ambiguous(vec![TypeRef::Int, TypeRef::String]))
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

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
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

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
    }));
}
