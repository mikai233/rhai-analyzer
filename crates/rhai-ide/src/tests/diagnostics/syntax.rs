use std::path::Path;

use rhai_db::ChangeSet;
use rhai_db::ProjectDiagnosticCode;
use rhai_syntax::SyntaxErrorCode;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, DiagnosticSeverity};

#[test]
fn diagnostics_report_nested_function_definitions_as_syntax_errors() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn outer() {
                fn inner() {}
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

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Syntax(SyntaxErrorCode::FunctionsMustBeDefinedAtGlobalLevel)
    }));
}
#[test]
fn diagnostics_report_missing_semicolon_between_statements() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let v = "hello"
            let q = 1.0 + 2;
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");

    assert!(analysis.diagnostics(file_id).iter().any(|diagnostic| {
        diagnostic.code
            == ProjectDiagnosticCode::Syntax(SyntaxErrorCode::ExpectedSemicolonToTerminateStatement)
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
}
