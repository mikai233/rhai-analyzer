use lsp_types::{self, Diagnostic};
use rhai_db::ProjectDiagnosticCode;
use rhai_hir::SemanticDiagnosticCode;
use rhai_ide::{
    Diagnostic as IdeDiagnostic, DiagnosticSeverity as IdeDiagnosticSeverity,
    DiagnosticTag as IdeDiagnosticTag,
};
use rhai_syntax::SyntaxErrorCode;

use crate::protocol::text_range_to_lsp_range;

pub(crate) fn diagnostic_to_lsp(text: &str, diagnostic: &IdeDiagnostic) -> Option<Diagnostic> {
    Some(Diagnostic {
        range: text_range_to_lsp_range(text, diagnostic.range)?,
        severity: Some(match diagnostic.severity {
            IdeDiagnosticSeverity::Error => lsp_types::DiagnosticSeverity::ERROR,
            IdeDiagnosticSeverity::Warning => lsp_types::DiagnosticSeverity::WARNING,
        }),
        code: Some(lsp_types::NumberOrString::String(diagnostic_code_label(
            &diagnostic.code,
        ))),
        code_description: None,
        source: Some("rhai-analyzer".to_owned()),
        message: diagnostic.message.clone(),
        related_information: None,
        tags: if diagnostic.tags.is_empty() {
            None
        } else {
            Some(
                diagnostic
                    .tags
                    .iter()
                    .map(|tag| match tag {
                        IdeDiagnosticTag::Unnecessary => lsp_types::DiagnosticTag::UNNECESSARY,
                    })
                    .collect(),
            )
        },
        data: None,
    })
}

fn diagnostic_code_label(code: &ProjectDiagnosticCode) -> String {
    match code {
        ProjectDiagnosticCode::Syntax(code) => format!("syntax/{}", syntax_error_code_label(code)),
        ProjectDiagnosticCode::Semantic(code) => {
            format!("semantic/{}", semantic_diagnostic_code_label(code))
        }
        ProjectDiagnosticCode::BrokenLinkedImport => "linked-import/broken".to_owned(),
        ProjectDiagnosticCode::AmbiguousLinkedImport => "linked-import/ambiguous".to_owned(),
        ProjectDiagnosticCode::UnresolvedImportMember => "import-member/unresolved".to_owned(),
        ProjectDiagnosticCode::CallerScopeRequired => "call/caller-scope-required".to_owned(),
    }
}

fn syntax_error_code_label(code: &SyntaxErrorCode) -> String {
    match code {
        SyntaxErrorCode::UnterminatedStringLiteral => "unterminated-string-literal".to_owned(),
        SyntaxErrorCode::UnterminatedCharacterLiteral => {
            "unterminated-character-literal".to_owned()
        }
        SyntaxErrorCode::UnterminatedBacktickStringLiteral => {
            "unterminated-backtick-string-literal".to_owned()
        }
        SyntaxErrorCode::UnterminatedStringInterpolation => {
            "unterminated-string-interpolation".to_owned()
        }
        SyntaxErrorCode::UnterminatedBlockComment => "unterminated-block-comment".to_owned(),
        SyntaxErrorCode::UnterminatedRawStringLiteral => {
            "unterminated-raw-string-literal".to_owned()
        }
        SyntaxErrorCode::ExpectedExpression => "expected-expression".to_owned(),
        SyntaxErrorCode::ExpectedPropertyValue => "expected-property-value".to_owned(),
        SyntaxErrorCode::ExpectedExpressionAfterOperator => {
            "expected-expression-after-operator".to_owned()
        }
        SyntaxErrorCode::ExpectedCommaBetweenArguments => {
            "expected-comma-between-arguments".to_owned()
        }
        SyntaxErrorCode::ExpectedClosingArgumentList => "expected-closing-argument-list".to_owned(),
        SyntaxErrorCode::ExpectedInForExpression => "expected-in-for-expression".to_owned(),
        SyntaxErrorCode::ExpectedSwitchArmArrow => "expected-switch-arm-arrow".to_owned(),
        SyntaxErrorCode::ExpectedConstantValue => "expected-constant-value".to_owned(),
        SyntaxErrorCode::ExpectedAliasAfterAs => "expected-alias-after-as".to_owned(),
        SyntaxErrorCode::ExpectedCommaBetweenParameters => {
            "expected-comma-between-parameters".to_owned()
        }
        SyntaxErrorCode::ExpectedParameterName => "expected-parameter-name".to_owned(),
        SyntaxErrorCode::ExpectedClosingParameters => "expected-closing-parameters".to_owned(),
        SyntaxErrorCode::ExpectedCommaBetweenArrayItems => {
            "expected-comma-between-array-items".to_owned()
        }
        SyntaxErrorCode::ExpectedCommaBetweenObjectFields => {
            "expected-comma-between-object-fields".to_owned()
        }
        SyntaxErrorCode::ExpectedClosureParameter => "expected-closure-parameter".to_owned(),
        SyntaxErrorCode::ExpectedCommaBetweenClosureParameters => {
            "expected-comma-between-closure-parameters".to_owned()
        }
        SyntaxErrorCode::ExpectedClosingClosureParameters => {
            "expected-closing-closure-parameters".to_owned()
        }
        SyntaxErrorCode::ExpectedOpenParenAfterFunctionName => {
            "expected-open-paren-after-function-name".to_owned()
        }
        SyntaxErrorCode::ExpectedFunctionNameAfterFn => {
            "expected-function-name-after-fn".to_owned()
        }
        SyntaxErrorCode::ExpectedMethodNameAfterTypedMethodDefinition => {
            "expected-method-name-after-typed-method-definition".to_owned()
        }
        SyntaxErrorCode::ExpectedModulePathAfterImport => {
            "expected-module-path-after-import".to_owned()
        }
        SyntaxErrorCode::ExpectedEqInConstStatement => "expected-eq-in-const-statement".to_owned(),
        SyntaxErrorCode::ExpectedExportTargetAfterExport => {
            "expected-export-target-after-export".to_owned()
        }
        SyntaxErrorCode::ExpectedCatchClauseAfterTry => {
            "expected-catch-clause-after-try".to_owned()
        }
        SyntaxErrorCode::ExpectedWhileOrUntilAfterDoBlock => {
            "expected-while-or-until-after-do-block".to_owned()
        }
        SyntaxErrorCode::ExpectedClosureBody => "expected-closure-body".to_owned(),
        SyntaxErrorCode::ExpectedBindingInForExpression => {
            "expected-binding-in-for-expression".to_owned()
        }
        SyntaxErrorCode::ExpectedIndexExpression => "expected-index-expression".to_owned(),
        SyntaxErrorCode::ExpectedClosingIndexExpression => {
            "expected-closing-index-expression".to_owned()
        }
        SyntaxErrorCode::ExpectedPropertyNameAfterFieldAccess => {
            "expected-property-name-after-field-access".to_owned()
        }
        SyntaxErrorCode::ExpectedExpressionInsideParentheses => {
            "expected-expression-inside-parentheses".to_owned()
        }
        SyntaxErrorCode::ExpectedSemicolonToTerminateStatement => {
            "expected-semicolon-to-terminate-statement".to_owned()
        }
        SyntaxErrorCode::FunctionsMustBeDefinedAtGlobalLevel => {
            "functions-must-be-defined-at-global-level".to_owned()
        }
        SyntaxErrorCode::CallerScopeMethodStyle => "caller-scope-method-style".to_owned(),
        SyntaxErrorCode::CallerScopeNamespacePath => "caller-scope-namespace-path".to_owned(),
        SyntaxErrorCode::ExpectedDotAfterTypedMethodReceiver => {
            "expected-dot-after-typed-method-receiver".to_owned()
        }
        SyntaxErrorCode::InvalidExportPlacement => "invalid-export-placement".to_owned(),
        SyntaxErrorCode::InvalidExportTargetShape => "invalid-export-target-shape".to_owned(),
        SyntaxErrorCode::Other(message) => format!("other:{message}"),
    }
}

fn semantic_diagnostic_code_label(code: &SemanticDiagnosticCode) -> &'static str {
    match code {
        SemanticDiagnosticCode::ConstantCondition => "constant-condition",
        SemanticDiagnosticCode::UnresolvedName => "unresolved-name",
        SemanticDiagnosticCode::DuplicateDefinition => "duplicate-definition",
        SemanticDiagnosticCode::UnresolvedImportModule => "unresolved-import-module",
        SemanticDiagnosticCode::UnresolvedExportTarget => "unresolved-export-target",
        SemanticDiagnosticCode::InvalidExportTarget => "invalid-export-target",
        SemanticDiagnosticCode::InvalidImportModuleType => "invalid-import-module-type",
        SemanticDiagnosticCode::UnusedSymbol => "unused-symbol",
        SemanticDiagnosticCode::DuplicateDocParamTag { .. } => "duplicate-doc-param-tag",
        SemanticDiagnosticCode::DuplicateDocReturnTag => "duplicate-doc-return-tag",
        SemanticDiagnosticCode::DocParamDoesNotMatchFunction { .. } => {
            "doc-param-does-not-match-function"
        }
        SemanticDiagnosticCode::FunctionHasNonFunctionTypeAnnotation { .. } => {
            "function-has-non-function-type-annotation"
        }
        SemanticDiagnosticCode::FunctionDocTagsOnNonFunction { .. } => {
            "function-doc-tags-on-non-function"
        }
    }
}
