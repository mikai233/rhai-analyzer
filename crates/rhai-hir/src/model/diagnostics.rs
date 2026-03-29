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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub kind: SemanticDiagnosticKind,
    pub range: TextRange,
    pub message: String,
    pub related_range: Option<TextRange>,
}
