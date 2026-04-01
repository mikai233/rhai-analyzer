use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectDiagnosticCode};
use rhai_hir::{FunctionTypeRef, SemanticDiagnosticCode, SymbolKind, TypeRef};
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

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
fn comment_extern_directives_suppress_unresolved_names_and_seed_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            // rhai: extern injected_value: int
            let result = injected_value + 1;
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
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected extern directive to suppress unresolved name, got {diagnostics:?}"
    );

    let hir = snapshot.hir(file_id).expect("expected hir");
    let result = symbol_id_by_name(hir.as_ref(), "result", SymbolKind::Variable);
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}
#[test]
fn comment_module_directives_seed_import_alias_members_and_call_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            // rhai: module env
            // rhai: extern env::test: fun(int) -> int
            // rhai: extern env::DEFAULTS: map<string, int>
            import "env" as env;

            let result = env::test(1);
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
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedImportModule)
        }),
        "expected inline module directive to suppress unresolved import, got {diagnostics:?}"
    );

    let completions = snapshot.imported_module_completions(file_id, &[String::from("env")]);
    let test_completion = completions
        .iter()
        .find(|completion| completion.name == "test")
        .expect("expected inline module completion for test");
    assert_eq!(
        test_completion.annotation,
        Some(TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );

    let hir = snapshot.hir(file_id).expect("expected hir");
    let result = symbol_id_by_name(hir.as_ref(), "result", SymbolKind::Variable);
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}
#[test]
fn project_host_modules_seed_import_alias_member_completions() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![FileChange {
            path: "main.rhai".into(),
            text: r#"
                import "env" as env;

                let result = env::test(1);
                let defaults = env::DEFAULTS;
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "env".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Environment helpers".to_owned()),
                    functions: [(
                        "test".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int) -> int".to_owned(),
                            return_type: None,
                            docs: Some("Run the environment test".to_owned()),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: [(
                        "DEFAULTS".to_owned(),
                        rhai_project::ConstantSpec {
                            type_name: "map<string, int>".to_owned(),
                            docs: Some("Default environment values".to_owned()),
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
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    let completions = snapshot.imported_module_completions(file_id, &[String::from("env")]);
    let test_completion = completions
        .iter()
        .find(|completion| completion.name == "test")
        .expect("expected host module function completion");
    assert_eq!(
        test_completion.annotation,
        Some(TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }))
    );
    assert_eq!(
        test_completion.docs.as_deref(),
        Some("Run the environment test")
    );

    let defaults_completion = completions
        .iter()
        .find(|completion| completion.name == "DEFAULTS")
        .expect("expected host module constant completion");
    assert_eq!(
        defaults_completion.annotation,
        Some(TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::Int)
        ))
    );
    assert_eq!(
        defaults_completion.docs.as_deref(),
        Some("Default environment values")
    );
}
