use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, lower_file};

#[test]
fn semantic_diagnostics_report_constant_if_and_while_conditions() {
    let parse = parse_valid(
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
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::ConstantCondition)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == SemanticDiagnosticCode::ConstantCondition)
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
