use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, lower_file};

#[test]
fn semantic_diagnostics_report_unresolved_imports_and_exports() {
    let parse = parse_valid(
        r#"
            import missing_module as secure;
            export missing_value as exposed;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let import_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedImport)
        .collect::<Vec<_>>();
    let export_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedExport)
        .collect::<Vec<_>>();
    let unresolved_name_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedName)
        .collect::<Vec<_>>();

    assert_eq!(import_diagnostics.len(), 1);
    assert_eq!(
        import_diagnostics[0].code,
        SemanticDiagnosticCode::UnresolvedImportModule
    );
    assert!(import_diagnostics[0].related_range.is_some());

    assert_eq!(export_diagnostics.len(), 1);
    assert_eq!(
        export_diagnostics[0].code,
        SemanticDiagnosticCode::UnresolvedExportTarget
    );
    assert!(export_diagnostics[0].related_range.is_some());

    assert!(unresolved_name_diagnostics.is_empty());
    assert_eq!(hir.imports.len(), 1);
    assert_eq!(hir.exports.len(), 1);
}
#[test]
fn semantic_diagnostics_reject_explicit_function_exports() {
    let parse = parse_valid(
        r#"
            fn helper() {}
            export helper as public_helper;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let invalid_export_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InvalidExportTarget)
        .collect::<Vec<_>>();

    assert_eq!(invalid_export_diagnostics.len(), 1);
    assert_eq!(
        invalid_export_diagnostics[0].code,
        SemanticDiagnosticCode::InvalidExportTarget
    );
    assert!(invalid_export_diagnostics[0].related_range.is_some());
}
#[test]
fn semantic_diagnostics_reject_non_string_import_expressions() {
    let parse = parse_valid(
        r#"
            fn helper() {}
            let module_name = 1;
            const valid_module = "crypto";
            const prefix = "crypt";
            const suffix = "o";
            const block_module = { "crypto" };
            const conditional_module = if true { "crypto" } else { "hash" };

            import helper as bad_helper;
            import module_name as bad_value;
            import valid_module as ok_module;
            import prefix + suffix as ok_concat;
            import block_module as ok_block;
            import conditional_module as ok_conditional;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let invalid_import_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InvalidImportModuleType)
        .collect::<Vec<_>>();

    assert_eq!(invalid_import_diagnostics.len(), 2);
    assert!(
        invalid_import_diagnostics.iter().all(|diagnostic| {
            diagnostic.code == SemanticDiagnosticCode::InvalidImportModuleType
        })
    );
    assert!(
        invalid_import_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
    assert_eq!(invalid_import_diagnostics.len(), 2, "{diagnostics:?}");
}
