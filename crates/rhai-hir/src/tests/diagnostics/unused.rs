use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, lower_file};

#[test]
fn semantic_diagnostics_report_unused_symbols() {
    let parse = parse_valid(
        r#"
            import "crypto" as secure;
            const KEPT = 1;

            fn sample(arg, _ignored) {
                let local = 1;
                let kept = arg + KEPT;
                kept;
            }

            sample(KEPT);
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnusedSymbol)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == SemanticDiagnosticCode::UnusedSymbol)
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == SemanticDiagnosticCode::UnusedSymbol)
    );
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
}
#[test]
fn semantic_diagnostics_do_not_report_caller_scope_captures_as_unused_symbols() {
    let parse = parse_valid(
        r#"
            export const DEFAULTS = #{ name: "demo" };

            fn make_config() {
                DEFAULTS
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnusedSymbol)
        .collect::<Vec<_>>();

    assert!(
        diagnostics.is_empty(),
        "expected caller-scope capture to count as usage, got {diagnostics:?}"
    );
}
