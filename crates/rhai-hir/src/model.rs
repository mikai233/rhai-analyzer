use rhai_syntax::TextRange;

use crate::{DocBlock, DocBlockId, TypeRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeSlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileSymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    File,
    Function,
    Block,
    Catch,
    SwitchArm,
    Closure,
    Loop,
    Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Constant,
    Function,
    ImportAlias,
    ExportAlias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    Name,
    This,
    PathSegment,
    Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyKind {
    Function,
    Closure,
    Block,
    Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlFlowKind {
    Return,
    Throw,
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergePointKind {
    IfElse,
    Switch,
    LoopIteration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExprKind {
    Name,
    Literal,
    Array,
    Object,
    If,
    Switch,
    While,
    Loop,
    For,
    Do,
    Path,
    Closure,
    InterpolatedString,
    Unary,
    Binary,
    Assign,
    Paren,
    Call,
    Index,
    Field,
    Block,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlFlowEvent {
    pub kind: ControlFlowKind,
    pub range: TextRange,
    pub value_range: Option<TextRange>,
    pub target_loop: Option<ScopeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlFlowMergePoint {
    pub kind: MergePointKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub range: TextRange,
    pub parent: Option<ScopeId>,
    pub children: Vec<ScopeId>,
    pub symbols: Vec<SymbolId>,
    pub references: Vec<ReferenceId>,
    pub bodies: Vec<BodyId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub docs: Option<DocBlockId>,
    pub annotation: Option<TypeRef>,
    pub references: Vec<ReferenceId>,
    pub shadowed: Option<SymbolId>,
    pub duplicate_of: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reference {
    pub name: String,
    pub kind: ReferenceKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub target: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    pub kind: BodyKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub owner: Option<SymbolId>,
    pub control_flow: Vec<ControlFlowEvent>,
    pub return_values: Vec<ExprId>,
    pub throw_values: Vec<ExprId>,
    pub merge_points: Vec<ControlFlowMergePoint>,
    pub may_fall_through: bool,
    pub unreachable_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprNode {
    pub kind: ExprKind,
    pub range: TextRange,
    pub scope: ScopeId,
    pub result_slot: TypeSlotId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeSlot {
    pub range: TextRange,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeSlotAssignments {
    values: Vec<Option<TypeRef>>,
}

impl TypeSlotAssignments {
    pub fn with_slot_count(slot_count: usize) -> Self {
        Self {
            values: vec![None; slot_count],
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn set(&mut self, slot: TypeSlotId, ty: TypeRef) {
        let index = slot.0 as usize;
        if index >= self.values.len() {
            self.values.resize(index + 1, None);
        }
        self.values[index] = Some(ty);
    }

    pub fn get(&self, slot: TypeSlotId) -> Option<&TypeRef> {
        self.values
            .get(slot.0 as usize)
            .and_then(|value| value.as_ref())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalSignatureIndex {
    symbols: std::collections::BTreeMap<String, TypeRef>,
}

impl ExternalSignatureIndex {
    pub fn insert(&mut self, name: impl Into<String>, ty: TypeRef) -> Option<TypeRef> {
        self.symbols.insert(name.into(), ty)
    }

    pub fn get(&self, name: &str) -> Option<&TypeRef> {
        self.symbols.get(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueFlowKind {
    Initializer,
    Assignment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolValueFlow {
    pub symbol: SymbolId,
    pub expr: ExprId,
    pub kind: ValueFlowKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub range: TextRange,
    pub scope: ScopeId,
    pub callee_range: Option<TextRange>,
    pub callee_reference: Option<ReferenceId>,
    pub resolved_callee: Option<SymbolId>,
    pub arg_ranges: Vec<TextRange>,
    pub parameter_bindings: Vec<Option<SymbolId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFieldInfo {
    pub owner: ExprId,
    pub name: String,
    pub range: TextRange,
    pub value: Option<ExprId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub range: TextRange,
    pub scope: ScopeId,
    pub receiver: ExprId,
    pub field_reference: ReferenceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentedField {
    pub name: String,
    pub annotation: TypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDirective {
    pub range: TextRange,
    pub scope: ScopeId,
    pub module_range: Option<TextRange>,
    pub module_text: Option<String>,
    pub module_reference: Option<ReferenceId>,
    pub alias: Option<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDirective {
    pub range: TextRange,
    pub scope: ScopeId,
    pub target_range: Option<TextRange>,
    pub target_text: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceLocation {
    pub reference: ReferenceId,
    pub kind: ReferenceKind,
    pub range: TextRange,
    pub target: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSymbol {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub docs: Option<DocBlockId>,
    pub annotation: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHintParameter {
    pub symbol: Option<SymbolId>,
    pub name: String,
    pub annotation: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHint {
    pub call: CallSiteId,
    pub callee: NavigationTarget,
    pub callee_name: String,
    pub active_parameter: usize,
    pub parameters: Vec<ParameterHintParameter>,
    pub return_type: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindReferencesResult {
    pub symbol: SymbolId,
    pub declaration: NavigationTarget,
    pub references: Vec<ReferenceLocation>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StableSymbolKey {
    pub name: String,
    pub kind: SymbolKind,
    pub container_path: Vec<String>,
    pub ordinal: u32,
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
pub struct ModuleImportEdge {
    pub import: usize,
    pub module: Option<ModuleSpecifier>,
    pub alias: Option<FileBackedSymbolIdentity>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticDiagnosticKind {
    UnresolvedImport,
    UnresolvedExport,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHir {
    pub root_range: TextRange,
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    pub bodies: Vec<Body>,
    pub exprs: Vec<ExprNode>,
    pub type_slots: Vec<TypeSlot>,
    pub value_flows: Vec<SymbolValueFlow>,
    pub calls: Vec<CallSite>,
    pub object_fields: Vec<ObjectFieldInfo>,
    pub member_accesses: Vec<MemberAccess>,
    pub imports: Vec<ImportDirective>,
    pub exports: Vec<ExportDirective>,
    pub docs: Vec<DocBlock>,
}
