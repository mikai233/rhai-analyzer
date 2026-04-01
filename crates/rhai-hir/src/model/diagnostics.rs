use rhai_syntax::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticDiagnosticKind {
    UnresolvedImport,
    UnresolvedExport,
    InvalidExportTarget,
    InvalidImportModuleType,
    UnresolvedName,
    DuplicateDefinition,
    InconsistentDocType,
    UnusedSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SemanticDiagnosticCode {
    UnresolvedName,
    DuplicateDefinition,
    UnresolvedImportModule,
    UnresolvedExportTarget,
    InvalidExportTarget,
    InvalidImportModuleType,
    UnusedSymbol,
    DuplicateDocParamTag { name: String },
    DuplicateDocReturnTag,
    DocParamDoesNotMatchFunction { name: String, function: String },
    FunctionHasNonFunctionTypeAnnotation { function: String },
    FunctionDocTagsOnNonFunction { symbol: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub kind: SemanticDiagnosticKind,
    pub code: SemanticDiagnosticCode,
    pub range: TextRange,
    pub message: String,
    pub related_range: Option<TextRange>,
}
