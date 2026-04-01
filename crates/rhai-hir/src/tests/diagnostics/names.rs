use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, lower_file};

#[test]
fn semantic_diagnostics_report_unresolved_names() {
    let parse = parse_valid(
        r#"
            fn sample() {
                missing_name;
                this;
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, SemanticDiagnosticKind::UnresolvedName);
    assert_eq!(diagnostics[0].code, SemanticDiagnosticCode::UnresolvedName);
    assert!(diagnostics[0].related_range.is_none());
}
#[test]
fn semantic_diagnostics_report_duplicate_definitions_in_same_scope() {
    let parse = parse_valid(
        r#"
            let value = 1;
            let value = 2;

            fn sample(arg, arg) {
                let local = 1;
                let local = 2;
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::DuplicateDefinition)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == SemanticDiagnosticCode::DuplicateDefinition)
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
}
#[test]
fn semantic_diagnostics_allow_variable_and_constant_shadowing() {
    let parse = parse_valid(
        r#"
            let value = 1;
            let value = "hello";

            const LIMIT = 1;
            const LIMIT = 2;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::DuplicateDefinition)
        .collect::<Vec<_>>();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
