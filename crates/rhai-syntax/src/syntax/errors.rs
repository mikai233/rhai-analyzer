use thiserror::Error;

use crate::syntax::TextRange;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct SyntaxError {
    code: SyntaxErrorCode,
    message: String,
    range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SyntaxErrorCode {
    UnterminatedStringLiteral,
    UnterminatedCharacterLiteral,
    UnterminatedBacktickStringLiteral,
    UnterminatedStringInterpolation,
    UnterminatedBlockComment,
    UnterminatedRawStringLiteral,
    ExpectedExpression,
    ExpectedPropertyValue,
    ExpectedExpressionAfterOperator,
    ExpectedCommaBetweenArguments,
    ExpectedClosingArgumentList,
    ExpectedInForExpression,
    ExpectedSwitchArmArrow,
    ExpectedConstantValue,
    ExpectedAliasAfterAs,
    ExpectedCommaBetweenParameters,
    ExpectedParameterName,
    ExpectedClosingParameters,
    ExpectedCommaBetweenArrayItems,
    ExpectedCommaBetweenObjectFields,
    ExpectedClosureParameter,
    ExpectedCommaBetweenClosureParameters,
    ExpectedClosingClosureParameters,
    ExpectedOpenParenAfterFunctionName,
    ExpectedFunctionNameAfterFn,
    ExpectedMethodNameAfterTypedMethodDefinition,
    ExpectedModulePathAfterImport,
    ExpectedEqInConstStatement,
    ExpectedExportTargetAfterExport,
    ExpectedCatchClauseAfterTry,
    ExpectedWhileOrUntilAfterDoBlock,
    ExpectedClosureBody,
    ExpectedBindingInForExpression,
    ExpectedIndexExpression,
    ExpectedClosingIndexExpression,
    ExpectedPropertyNameAfterFieldAccess,
    ExpectedExpressionInsideParentheses,
    ExpectedSemicolonToTerminateStatement,
    FunctionsMustBeDefinedAtGlobalLevel,
    CallerScopeMethodStyle,
    CallerScopeNamespacePath,
    ExpectedDotAfterTypedMethodReceiver,
    InvalidExportPlacement,
    InvalidExportTargetShape,
    Other(String),
}

impl SyntaxError {
    pub fn new(message: impl Into<String>, range: TextRange) -> Self {
        let message = message.into();
        Self {
            code: SyntaxErrorCode::from_message(message.as_str()),
            message,
            range,
        }
    }

    pub fn code(&self) -> &SyntaxErrorCode {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl SyntaxErrorCode {
    fn from_message(message: &str) -> Self {
        match message {
            "unterminated string literal" => Self::UnterminatedStringLiteral,
            "unterminated character literal" => Self::UnterminatedCharacterLiteral,
            "unterminated back-tick string literal" => Self::UnterminatedBacktickStringLiteral,
            "unterminated string interpolation" => Self::UnterminatedStringInterpolation,
            "unterminated block comment" => Self::UnterminatedBlockComment,
            "unterminated raw string literal" => Self::UnterminatedRawStringLiteral,
            "expected expression" => Self::ExpectedExpression,
            "expected property value" => Self::ExpectedPropertyValue,
            "expected expression after operator" => Self::ExpectedExpressionAfterOperator,
            "expected `,` between arguments" => Self::ExpectedCommaBetweenArguments,
            "expected `)` to close argument list" => Self::ExpectedClosingArgumentList,
            "expected `in` in `for` expression" => Self::ExpectedInForExpression,
            "expected `=>` in `switch` arm" => Self::ExpectedSwitchArmArrow,
            "expected constant value" => Self::ExpectedConstantValue,
            "expected alias after `as`" => Self::ExpectedAliasAfterAs,
            "expected `,` between parameters" => Self::ExpectedCommaBetweenParameters,
            "expected parameter name" => Self::ExpectedParameterName,
            "expected `)` after parameters" => Self::ExpectedClosingParameters,
            "expected `,` between array items" => Self::ExpectedCommaBetweenArrayItems,
            "expected `,` between object fields" => Self::ExpectedCommaBetweenObjectFields,
            "expected closure parameter" => Self::ExpectedClosureParameter,
            "expected `,` between closure parameters" => {
                Self::ExpectedCommaBetweenClosureParameters
            }
            "expected closing `|` for closure parameters" => Self::ExpectedClosingClosureParameters,
            "expected `(` after function name" => Self::ExpectedOpenParenAfterFunctionName,
            "expected function name after `fn`" => Self::ExpectedFunctionNameAfterFn,
            "expected method name after `.` in typed method definition" => {
                Self::ExpectedMethodNameAfterTypedMethodDefinition
            }
            "expected module path after `import`" => Self::ExpectedModulePathAfterImport,
            "expected `=` in `const` statement" => Self::ExpectedEqInConstStatement,
            "expected export target after `export`" => Self::ExpectedExportTargetAfterExport,
            "expected `catch` clause after `try`" => Self::ExpectedCatchClauseAfterTry,
            "expected `while` or `until` after `do` block" => {
                Self::ExpectedWhileOrUntilAfterDoBlock
            }
            "expected closure body" => Self::ExpectedClosureBody,
            "expected binding in `for` expression" => Self::ExpectedBindingInForExpression,
            "expected index expression" => Self::ExpectedIndexExpression,
            "expected `]` to close index expression" => Self::ExpectedClosingIndexExpression,
            "expected property name after field access" => {
                Self::ExpectedPropertyNameAfterFieldAccess
            }
            "expected expression inside parentheses" => Self::ExpectedExpressionInsideParentheses,
            "expected `;` to terminate statement" => Self::ExpectedSemicolonToTerminateStatement,
            "functions can only be defined at global level" => {
                Self::FunctionsMustBeDefinedAtGlobalLevel
            }
            "caller-scope function calls cannot use method-call style" => {
                Self::CallerScopeMethodStyle
            }
            "caller-scope function calls cannot use namespace-qualified paths" => {
                Self::CallerScopeNamespacePath
            }
            "expected `.` after typed method receiver" => Self::ExpectedDotAfterTypedMethodReceiver,
            "the `export` statement can only be used at global level" => {
                Self::InvalidExportPlacement
            }
            "expected exported variable name or `let`/`const` declaration after `export`" => {
                Self::InvalidExportTargetShape
            }
            other => Self::Other(other.to_owned()),
        }
    }
}
