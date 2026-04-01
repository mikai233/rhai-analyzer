use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, SymbolKind, lower_file};

#[test]
fn semantic_diagnostics_reject_function_access_to_external_scope() {
    let parse = parse_valid(
        r#"
            let value = 42;

            fn helper() {
                value
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let unresolved = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedName)
        .collect::<Vec<_>>();

    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].code, SemanticDiagnosticCode::UnresolvedName);
}
#[test]
fn semantic_diagnostics_allow_global_import_aliases_inside_functions() {
    let parse = parse_valid(
        r#"
            import "hello" as hey;

            fn helper(value) {
                hey::process(value);
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && diagnostic.code == SemanticDiagnosticCode::UnresolvedName
    }));

    let hey_reference = hir
        .references
        .iter()
        .find(|reference| reference.name == "hey")
        .expect("expected `hey` reference");
    let target = hey_reference
        .target
        .expect("expected resolved import alias");
    assert_eq!(hir.symbol(target).kind, SymbolKind::ImportAlias);
}
