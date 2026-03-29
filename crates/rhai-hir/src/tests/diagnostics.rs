use crate::tests::parse_valid;
use crate::{SemanticDiagnosticKind, SymbolKind, lower_file};

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
    assert_eq!(diagnostics[0].message, "unresolved name `missing_name`");
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

    assert_eq!(diagnostics.len(), 3);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `value`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `arg`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `local`")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
}

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
        import_diagnostics[0].message,
        "unresolved import module `missing_module`"
    );
    assert!(import_diagnostics[0].related_range.is_some());

    assert_eq!(export_diagnostics.len(), 1);
    assert_eq!(
        export_diagnostics[0].message,
        "unresolved export target `missing_value`"
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
        invalid_export_diagnostics[0].message,
        "export target `helper` must refer to a global variable or constant"
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
    assert!(invalid_import_diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "import module expression `helper` must evaluate to string, found function"
    }));
    assert!(invalid_import_diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "import module expression `module_name` must evaluate to string, found int"
    }));
    assert!(
        invalid_import_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("prefix + suffix")
            || diagnostic.message.contains("block_module")
            || diagnostic.message.contains("conditional_module")
    }));
}

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
    assert_eq!(unresolved[0].message, "unresolved name `value`");
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
            && diagnostic.message == "unresolved name `hey`"
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
            .any(|diagnostic| diagnostic.message == "unused symbol `secure`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unused symbol `local`")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("_ignored"))
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("KEPT"))
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("arg"))
    );
}

#[test]
fn semantic_diagnostics_report_inconsistent_function_doc_types() {
    let parse = parse_valid(
        r#"
            /// @type int
            /// @param first int
            /// @param first string
            /// @param missing bool
            /// @return int
            /// @return string
            fn sample(first) {
                first
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InconsistentDocType)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 4);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate `@param` tag for `first`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate `@return` tags")
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "doc tag `@param missing` does not match any parameter of `sample`"
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "function `sample` has a non-function type annotation"
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
}

#[test]
fn semantic_diagnostics_report_function_doc_tags_on_non_functions() {
    let parse = parse_valid(
        r#"
            /// @param value int
            /// @return int
            let count = 1;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InconsistentDocType)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "function doc tags cannot be attached to `count`"
    );
    assert!(diagnostics[0].related_range.is_some());
}
