use std::path::Path;

use rhai_db::ChangeSet;
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, DiagnosticSeverity};

#[test]
fn diagnostics_report_constant_conditions_as_warnings() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                if true {
                    1;
                } else {
                    2;
                }

                while false {
                    3;
                }

                do {
                    4;
                } while false;

                do {
                    5;
                } until true;

                while true {
                    break;
                }

                do {
                    break;
                } while true;

                do {
                    break;
                } until false;

                while true {
                    loop {
                        break;
                    }
                }
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

    let diagnostics = analysis
        .diagnostics(file_id)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.code
                == ProjectDiagnosticCode::Semantic(SemanticDiagnosticCode::ConstantCondition)
        })
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "if condition is always true")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "while condition is always false")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "do-while condition is always false")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "do-until condition is always true")
    );
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message == "while condition is always true")
            .count()
            == 1
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "do-while condition is always true")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "do-until condition is always false")
    );
}
