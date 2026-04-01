use std::path::Path;

use crate::tests::assert_workspace_files_have_no_syntax_diagnostics;
use crate::{AnalyzerDatabase, ChangeSet, FileChange, ProjectDiagnosticCode};
use rhai_hir::SemanticDiagnosticCode;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

#[test]
fn caller_scope_captures_are_suppressed_when_function_has_no_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
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

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected unresolved capture to stay hidden until normal calls exist, got {diagnostics:?}"
    );
}
#[test]
fn caller_scope_captures_are_suppressed_for_caller_scope_invocations() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            let result = call!(helper);
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
        "expected caller-scope call to suppress unresolved capture, got {diagnostics:?}"
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must use caller scope (`call!`)")
    }));
}
#[test]
fn caller_scope_exported_consts_are_suppressed_when_function_has_no_calls() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                DEFAULTS
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

    let diagnostics = snapshot.project_diagnostics(file_id);
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected uncalled function capture of exported const to stay hidden, got {diagnostics:?}"
    );
}
#[test]
fn caller_scope_captures_ignore_unrelated_same_name_calls_in_other_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "main.rhai".into(),
                text: r#"
                    export const DEFAULTS = #{ name: "demo" };

                    fn make_config(root) {
                        #{
                            root: root,
                            defaults: DEFAULTS,
                        }
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "other.rhai".into(),
                text: r#"
                    fn make_config() {
                        1
                    }

                    make_config();
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
    let main = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    let diagnostics = snapshot.project_diagnostics(main);
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected unrelated same-name call sites to not trigger caller-scope diagnostics, got {diagnostics:?}"
    );
}
#[test]
fn caller_scope_captures_report_function_and_regular_call_sites() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            let a = call!(helper);
            let b = helper();
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
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected function-body unresolved capture diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::CallerScopeRequired)
    );
}
#[test]
fn caller_scope_captures_report_regular_imported_calls_across_files() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet {
        files: vec![
            FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let defaults = #{ name: "demo" };
                    fn make_config() {
                        defaults
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::make_config();
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
    let provider = snapshot
        .vfs()
        .file_id(Path::new("provider.rhai"))
        .expect("expected provider.rhai");
    let consumer = snapshot
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let imported_exports = snapshot
        .linked_imports(consumer)
        .iter()
        .flat_map(|linked| linked.exports.iter())
        .filter_map(|export| export.export.exported_name.clone())
        .collect::<Vec<_>>();
    assert!(
        imported_exports.iter().any(|name| name == "make_config"),
        "expected linked import to expose make_config, got {imported_exports:?}"
    );

    let provider_diagnostics = snapshot.project_diagnostics(provider);
    assert!(
        provider_diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
        }),
        "expected provider unresolved capture to stay visible once regular calls exist, got {provider_diagnostics:?}"
    );

    let consumer_diagnostics = snapshot.project_diagnostics(consumer);
    assert!(
        consumer_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::CallerScopeRequired),
        "expected consumer caller-scope diagnostic, got {consumer_diagnostics:?}"
    );
}
