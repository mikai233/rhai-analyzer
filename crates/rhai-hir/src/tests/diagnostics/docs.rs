use crate::tests::parse_valid;
use crate::{SemanticDiagnosticCode, SemanticDiagnosticKind, lower_file};

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
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.code
        == SemanticDiagnosticCode::DuplicateDocParamTag {
            name: "first".to_owned()
        }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == SemanticDiagnosticCode::DuplicateDocReturnTag)
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code
            == SemanticDiagnosticCode::DocParamDoesNotMatchFunction {
                name: "missing".to_owned(),
                function: "sample".to_owned(),
            }
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code
            == SemanticDiagnosticCode::FunctionHasNonFunctionTypeAnnotation {
                function: "sample".to_owned(),
            }
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
        diagnostics[0].code,
        SemanticDiagnosticCode::FunctionDocTagsOnNonFunction {
            symbol: "count".to_owned(),
        }
    );
    assert!(diagnostics[0].related_range.is_some());
}
