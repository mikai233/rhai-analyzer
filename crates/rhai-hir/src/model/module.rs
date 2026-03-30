use rhai_syntax::TextRange;

use crate::model::expr::ExprId;
use crate::model::scope::{ReferenceId, ScopeId};
use crate::model::symbol::{SymbolId, SymbolKind};
use crate::ty::TypeRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportLinkageKind {
    StaticText,
    LocalSymbol,
    DynamicExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportExposureKind {
    Bare,
    Aliased,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDirective {
    pub range: TextRange,
    pub scope: ScopeId,
    pub module_expr: Option<ExprId>,
    pub module_range: Option<TextRange>,
    pub module_text: Option<String>,
    pub module_reference: Option<ReferenceId>,
    pub alias: Option<SymbolId>,
    pub is_global: bool,
    pub linkage: ImportLinkageKind,
    pub exposure: ImportExposureKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDirective {
    pub range: TextRange,
    pub scope: ScopeId,
    pub target_range: Option<TextRange>,
    pub target_text: Option<String>,
    pub target_symbol: Option<SymbolId>,
    pub target_reference: Option<ReferenceId>,
    pub alias: Option<SymbolId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationTarget {
    pub symbol: SymbolId,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexableSymbol {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub container: Option<SymbolId>,
    pub exported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileSymbolId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableSymbolKey {
    pub name: String,
    pub kind: SymbolKind,
    pub container_path: Vec<String>,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSymbolIndexEntry {
    pub id: FileSymbolId,
    pub symbol: SymbolId,
    pub stable_key: StableSymbolKey,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub container_name: Option<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSymbolIndex {
    pub entries: Vec<FileSymbolIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub symbol: SymbolId,
    pub stable_key: StableSymbolKey,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub id: FileSymbolId,
    pub stable_key: StableSymbolKey,
    pub symbol: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub full_range: TextRange,
    pub focus_range: TextRange,
    pub container_name: Option<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingHandoff {
    pub file_symbols: FileSymbolIndex,
    pub workspace_symbols: Vec<WorkspaceSymbol>,
    pub module_graph: ModuleGraphIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberCompletionSource {
    DocumentedField,
    ObjectLiteralField,
    HostTypeMember,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCompletion {
    pub name: String,
    pub annotation: Option<TypeRef>,
    pub range: Option<TextRange>,
    pub source: MemberCompletionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenameOccurrenceKind {
    Definition,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameOccurrence {
    pub symbol: SymbolId,
    pub range: TextRange,
    pub kind: RenameOccurrenceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkedAliasKind {
    ImportAlias,
    ExportAlias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedAlias {
    pub kind: LinkedAliasKind,
    pub symbol: FileBackedSymbolIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileBackedSymbolIdentity {
    pub symbol: SymbolId,
    pub stable_key: StableSymbolKey,
    pub name: String,
    pub kind: SymbolKind,
    pub declaration_range: TextRange,
    pub container_path: Vec<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleSpecifier {
    Text(String),
    LocalSymbol(FileBackedSymbolIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModulePath {
    pub import: usize,
    pub alias: SymbolId,
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImportEdge {
    pub import: usize,
    pub module: Option<ModuleSpecifier>,
    pub alias: Option<FileBackedSymbolIdentity>,
    pub linkage: ImportLinkageKind,
    pub exposure: ImportExposureKind,
    pub is_global: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleExportEdge {
    pub export: usize,
    pub target: Option<FileBackedSymbolIdentity>,
    pub exported_name: Option<String>,
    pub alias: Option<FileBackedSymbolIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGraphIndex {
    pub imports: Vec<ModuleImportEdge>,
    pub exports: Vec<ModuleExportEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenamePreflightIssueKind {
    EmptyName,
    DuplicateDefinition,
    ReferenceCollision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePreflightIssue {
    pub kind: RenamePreflightIssueKind,
    pub message: String,
    pub range: TextRange,
    pub related_symbol: Option<FileBackedSymbolIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePlan {
    pub target: FileBackedSymbolIdentity,
    pub new_name: String,
    pub occurrences: Vec<RenameOccurrence>,
    pub linked_aliases: Vec<LinkedAlias>,
    pub issues: Vec<RenamePreflightIssue>,
}
