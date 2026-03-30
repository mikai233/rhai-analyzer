use rhai_hir::SymbolKind;
use rhai_syntax::TextRange;
use rhai_vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilePosition {
    pub file_id: FileId,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub tags: Vec<DiagnosticTag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticTag {
    Unnecessary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverResult {
    pub signature: String,
    pub docs: Option<String>,
    pub source: HoverSignatureSource,
    pub declared_signature: Option<String>,
    pub inferred_signature: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HoverSignatureSource {
    Declared,
    Inferred,
    Structural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InlayHintKind {
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InlayHintSource {
    Variable,
    Parameter,
    ReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    pub offset: u32,
    pub label: String,
    pub kind: InlayHintKind,
    pub source: InlayHintSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: usize,
    pub active_parameter: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub docs: Option<String>,
    pub parameters: Vec<SignatureParameter>,
    pub file_id: Option<FileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParameter {
    pub label: String,
    pub annotation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentHighlightKind {
    Read,
    Write,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub range: TextRange,
    pub kind: DocumentHighlightKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItem {
    pub file_id: FileId,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingCall {
    pub to: CallHierarchyItem,
    pub from_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoldingRangeKind {
    Comment,
    Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRange {
    pub range: TextRange,
    pub kind: FoldingRangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenKind {
    Keyword,
    Comment,
    String,
    Number,
    Type,
    Function,
    Method,
    Parameter,
    Variable,
    Property,
    Namespace,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenModifier {
    Declaration,
    Readonly,
    DefaultLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    pub range: TextRange,
    pub kind: SemanticTokenKind,
    pub modifiers: Vec<SemanticTokenModifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub file_id: FileId,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub container_name: Option<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTarget {
    pub file_id: FileId,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    Definition,
    Reference,
    LinkedImport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceLocation {
    pub file_id: FileId,
    pub range: TextRange,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencesResult {
    pub targets: Vec<NavigationTarget>,
    pub references: Vec<ReferenceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameIssue {
    pub file_id: FileId,
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePlan {
    pub new_name: String,
    pub targets: Vec<NavigationTarget>,
    pub occurrences: Vec<ReferenceLocation>,
    pub issues: Vec<RenameIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionItemKind {
    Symbol(SymbolKind),
    Member,
    Keyword,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionItemSource {
    Visible,
    Project,
    Member,
    Builtin,
    Postfix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub source: CompletionItemSource,
    pub sort_text: String,
    pub detail: Option<String>,
    pub docs: Option<String>,
    pub filter_text: Option<String>,
    pub text_edit: Option<CompletionTextEdit>,
    pub insert_format: CompletionInsertFormat,
    pub file_id: Option<FileId>,
    pub exported: bool,
    pub resolve_data: Option<CompletionResolveData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionInsertFormat {
    PlainText,
    Snippet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTextEdit {
    pub range: TextRange,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionResolveData {
    pub file_id: FileId,
    pub offset: u32,
}
