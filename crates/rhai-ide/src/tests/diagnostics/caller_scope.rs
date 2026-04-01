use std::path::Path;

use rhai_db::ChangeSet;
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_vfs::DocumentVersion;

use crate::AnalysisHost;
use crate::tests::assert_no_syntax_diagnostics;

#[test]
fn diagnostics_report_function_access_to_external_scope() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            helper();
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
    assert!(
        analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::CallerScopeRequired })
    );
}
#[test]
fn diagnostics_allow_function_access_to_external_scope_for_caller_scope_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }

            call!(helper);
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(!analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
    assert!(
        !analysis
            .diagnostics(file_id)
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::CallerScopeRequired })
    );
}
#[test]
fn diagnostics_allow_function_access_to_external_scope_when_uncalled() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let value = 42;

            fn helper() {
                value
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(!analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
}
#[test]
fn diagnostics_allow_exported_const_access_inside_uncalled_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                DEFAULTS
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);

    assert!(!analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::UnresolvedName)
    }));
}
#[test]
fn diagnostics_report_caller_scope_violations_for_imported_calls() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
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
            rhai_db::FileChange {
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
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);

    assert!(
        analysis
            .diagnostics(consumer)
            .iter()
            .any(|diagnostic| diagnostic.code == ProjectDiagnosticCode::CallerScopeRequired),
        "expected caller-scope diagnostic in importing module, got {:?}",
        analysis.diagnostics(consumer)
    );
}
