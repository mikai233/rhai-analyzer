use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rhai_hir::{
    CompletionSymbol, DocumentSymbol, ExternalSignatureIndex, FileBackedSymbolIdentity, FileHir,
    FileSymbolIndex, FunctionTypeRef, MemberCompletion, ModuleExportEdge, ModuleGraphIndex,
    NavigationTarget, RenamePreflightIssue, SemanticDiagnostic, SemanticDiagnosticCode,
    StableSymbolKey, SymbolId, SymbolKind, TypeRef, TypeSlotAssignments, WorkspaceSymbol,
};
use rhai_syntax::{Parse, SyntaxError, SyntaxErrorCode, TextRange, TextSize};
use rhai_vfs::{DocumentVersion, FileId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseInputSlot {
    pub normalized_path: PathBuf,
    pub document_version: DocumentVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirInputSlot {
    pub parse_revision: u64,
    pub project_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexInputSlot {
    pub hir_revision: u64,
    pub project_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvalidationReason {
    InitialLoad,
    TextChanged,
    ProjectChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFileInfo {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
    pub source_root: Option<usize>,
    pub is_workspace_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAnalysisDependencies {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
    pub document_version: DocumentVersion,
    pub source_root: Option<usize>,
    pub is_workspace_file: bool,
    pub parse: ParseInputSlot,
    pub hir: HirInputSlot,
    pub index: IndexInputSlot,
    pub last_invalidation: InvalidationReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerformanceStats {
    pub parse_rebuilds: u64,
    pub lower_rebuilds: u64,
    pub index_rebuilds: u64,
    pub query_support_rebuilds: u64,
    pub query_support_evictions: u64,
    pub total_parse_time: Duration,
    pub total_lower_time: Duration,
    pub total_index_time: Duration,
    pub total_query_support_time: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePerformanceStats {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
    pub parse_rebuilds: u64,
    pub lower_rebuilds: u64,
    pub index_rebuilds: u64,
    pub query_support_rebuilds: u64,
    pub query_support_evictions: u64,
    pub query_support_cached: bool,
    pub dependency_count: usize,
    pub dependent_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugFileAnalysis {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
    pub document_version: DocumentVersion,
    pub source_root: Option<usize>,
    pub is_workspace_file: bool,
    pub dependencies: FileAnalysisDependencies,
    pub stats: FilePerformanceStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseDebugView {
    pub revision: u64,
    pub project_revision: u64,
    pub source_roots: Vec<PathBuf>,
    pub files: Vec<DebugFileAnalysis>,
    pub stats: PerformanceStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedWorkspaceSymbol {
    pub file_id: FileId,
    pub symbol: WorkspaceSymbol,
}

#[derive(Debug, Clone)]
pub struct LocatedModuleGraph {
    pub file_id: FileId,
    pub graph: Arc<ModuleGraphIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedModuleExport {
    pub file_id: FileId,
    pub export: ModuleExportEdge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedSymbolIdentity {
    pub file_id: FileId,
    pub symbol: FileBackedSymbolIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedNavigationTarget {
    pub file_id: FileId,
    pub target: NavigationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedCallHierarchyItem {
    pub file_id: FileId,
    pub symbol: FileBackedSymbolIdentity,
    pub target: NavigationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedIncomingCall {
    pub from: LocatedCallHierarchyItem,
    pub from_ranges: Arc<[TextRange]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedOutgoingCall {
    pub to: LocatedCallHierarchyItem,
    pub from_ranges: Arc<[TextRange]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedModuleImport {
    pub file_id: FileId,
    pub provider_file_id: FileId,
    pub import: usize,
    pub module_name: String,
    pub exports: Arc<[LocatedModuleExport]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDependencyEdge {
    pub importer_file_id: FileId,
    pub exporter_file_id: FileId,
    pub module_name: String,
    pub import: usize,
    pub export: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceDependencyGraph {
    pub edges: Arc<[WorkspaceDependencyEdge]>,
    pub(crate) dependencies_by_file: Arc<HashMap<FileId, Arc<[FileId]>>>,
    pub(crate) dependents_by_file: Arc<HashMap<FileId, Arc<[FileId]>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionInputs {
    pub file_id: FileId,
    pub offset: TextSize,
    pub visible_symbols: Vec<CompletionSymbol>,
    pub project_symbols: Vec<LocatedWorkspaceSymbol>,
    pub member_symbols: Vec<MemberCompletion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModuleCompletion {
    pub name: String,
    pub kind: SymbolKind,
    pub origin: Option<String>,
    pub file_id: Option<FileId>,
    pub symbol: Option<SymbolId>,
    pub annotation: Option<TypeRef>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFieldHoverInfo {
    pub name: String,
    pub declaration_range: TextRange,
    pub declared_annotation: Option<TypeRef>,
    pub inferred_annotation: Option<TypeRef>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectDiagnosticKind {
    Syntax,
    Semantic,
    BrokenLinkedImport,
    AmbiguousLinkedImport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectDiagnosticTag {
    Unnecessary,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectDiagnosticCode {
    Syntax(SyntaxErrorCode),
    Semantic(SemanticDiagnosticCode),
    BrokenLinkedImport,
    AmbiguousLinkedImport,
    UnresolvedImportMember,
    CallerScopeRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDiagnostic {
    pub kind: ProjectDiagnosticKind,
    pub code: ProjectDiagnosticCode,
    pub severity: ProjectDiagnosticSeverity,
    pub range: TextRange,
    pub message: String,
    pub related_range: Option<TextRange>,
    pub tags: Arc<[ProjectDiagnosticTag]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoImportCandidate {
    pub file_id: FileId,
    pub provider_file_id: FileId,
    pub provider_path: PathBuf,
    pub symbol: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub annotation: Option<TypeRef>,
    pub docs: Option<String>,
    pub module_name: String,
    pub alias: String,
    pub replace_range: TextRange,
    pub qualified_reference_text: String,
    pub insertion_offset: TextSize,
    pub insert_text: String,
    pub import_cost: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTypeInference {
    pub expr_types: TypeSlotAssignments,
    pub symbol_types: HashMap<SymbolId, TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedNavigationTarget {
    pub symbol: FileBackedSymbolIdentity,
    pub target: NavigationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedMemberCompletionSet {
    pub symbol: FileBackedSymbolIdentity,
    pub members: Arc<[MemberCompletion]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerFileQuerySupport {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
    pub completion_symbols: Arc<[CompletionSymbol]>,
    pub navigation_targets: Arc<[CachedNavigationTarget]>,
    pub member_completion_sets: Arc<[CachedMemberCompletionSet]>,
    pub(crate) completion_symbols_by_symbol: Arc<HashMap<SymbolId, CompletionSymbol>>,
    pub(crate) navigation_targets_by_symbol: Arc<HashMap<SymbolId, NavigationTarget>>,
    pub(crate) member_completion_sets_by_symbol: Arc<HashMap<SymbolId, Arc<[MemberCompletion]>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectReferenceKind {
    Definition,
    Reference,
    LinkedImport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedProjectReference {
    pub file_id: FileId,
    pub range: TextRange,
    pub kind: ProjectReferenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReferences {
    pub targets: Vec<LocatedSymbolIdentity>,
    pub references: Vec<LocatedProjectReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedRenamePreflightIssue {
    pub file_id: FileId,
    pub issue: RenamePreflightIssue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRenamePlan {
    pub targets: Vec<LocatedSymbolIdentity>,
    pub new_name: String,
    pub occurrences: Vec<LocatedProjectReference>,
    pub issues: Vec<LocatedRenamePreflightIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedFileImpact {
    pub file_id: FileId,
    pub normalized_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeImpact {
    pub revision: u64,
    pub project_revision: u64,
    pub project_changed: bool,
    pub changed_files: Vec<FileId>,
    pub rebuilt_files: Vec<FileId>,
    pub removed_files: Vec<RemovedFileImpact>,
    pub dependency_affected_files: Vec<FileId>,
    pub evicted_query_support_files: Vec<FileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunctionOverload {
    pub signature: Option<FunctionTypeRef>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunction {
    pub name: String,
    pub overloads: Vec<HostFunctionOverload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConstant {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostModule {
    pub name: String,
    pub docs: Option<String>,
    pub functions: Vec<HostFunction>,
    pub constants: Vec<HostConstant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostType {
    pub name: String,
    pub generic_params: Vec<String>,
    pub docs: Option<String>,
    pub methods: Vec<HostFunction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileCommentDirectives {
    pub external_signatures: ExternalSignatureIndex,
    pub external_modules: BTreeSet<String>,
    pub allowed_unresolved_names: BTreeSet<String>,
    pub allowed_unresolved_imports: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProjectSemantics {
    pub(crate) external_signatures: ExternalSignatureIndex,
    pub(crate) global_functions: Vec<HostFunction>,
    pub(crate) modules: Vec<HostModule>,
    pub(crate) types: Vec<HostType>,
    pub(crate) disabled_symbols: Vec<String>,
    pub(crate) custom_syntaxes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SymbolIdentityKey {
    pub(crate) stable_key: StableSymbolKey,
    pub(crate) declaration_range: TextRange,
    pub(crate) exported: bool,
}

impl From<&FileBackedSymbolIdentity> for SymbolIdentityKey {
    fn from(identity: &FileBackedSymbolIdentity) -> Self {
        Self {
            stable_key: identity.stable_key.clone(),
            declaration_range: identity.declaration_range,
            exported: identity.exported,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CachedFileAnalysis {
    pub(crate) parse: Arc<Parse>,
    pub(crate) hir: Arc<FileHir>,
    pub(crate) comment_directives: Arc<FileCommentDirectives>,
    pub(crate) syntax_diagnostics: Arc<[SyntaxError]>,
    pub(crate) semantic_diagnostics: Arc<[SemanticDiagnostic]>,
    pub(crate) file_symbol_index: Arc<FileSymbolIndex>,
    pub(crate) document_symbols: Arc<[DocumentSymbol]>,
    pub(crate) workspace_symbols: Arc<[WorkspaceSymbol]>,
    pub(crate) module_graph: Arc<ModuleGraphIndex>,
    pub(crate) type_inference: Arc<FileTypeInference>,
    pub(crate) dependencies: Arc<FileAnalysisDependencies>,
    pub(crate) query_support: Option<Arc<PerFileQuerySupport>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceIndexes {
    pub(crate) symbols_by_file: HashMap<FileId, Arc<[LocatedWorkspaceSymbol]>>,
    pub(crate) module_graphs_by_file: HashMap<FileId, Arc<LocatedModuleGraph>>,
    pub(crate) exports_by_file: HashMap<FileId, Arc<[LocatedModuleExport]>>,
    pub(crate) symbol_locations_by_file: HashMap<FileId, Arc<[LocatedSymbolIdentity]>>,
    pub(crate) linked_imports_by_file: HashMap<FileId, Arc<[LinkedModuleImport]>>,
    pub(crate) workspace_dependency_graph: Arc<WorkspaceDependencyGraph>,
    pub(crate) workspace_symbols: Arc<[LocatedWorkspaceSymbol]>,
    pub(crate) workspace_module_graphs: Arc<[LocatedModuleGraph]>,
    pub(crate) workspace_exports: Arc<[LocatedModuleExport]>,
    pub(crate) symbol_locations: Arc<HashMap<SymbolIdentityKey, Arc<[LocatedSymbolIdentity]>>>,
    pub(crate) exports_by_name: Arc<HashMap<String, Arc<[LocatedModuleExport]>>>,
    pub(crate) linked_imports: Arc<HashMap<FileId, Arc<[LinkedModuleImport]>>>,
}
