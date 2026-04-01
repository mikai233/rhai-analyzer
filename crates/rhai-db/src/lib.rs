mod builtin;
mod change;
mod db;
mod infer;
mod overload;
mod project;
mod types;
mod workspace;

pub use builtin::signatures::builtin_universal_method_signature;
pub use change::{ChangeSet, FileChange};
pub use db::{AnalyzerDatabase, DatabaseSnapshot};
pub use infer::generics::specialize_signature_with_receiver_and_arg_types;
pub use overload::{best_matching_signature_index, best_matching_signature_indexes};
pub use types::{
    AutoImportCandidate, CachedMemberCompletionSet, CachedNavigationTarget, ChangeImpact,
    CompletionInputs, DatabaseDebugView, DebugFileAnalysis, FileAnalysisDependencies,
    FilePerformanceStats, FileTypeInference, HirInputSlot, HostConstant, HostFunction,
    HostFunctionOverload, HostModule, HostType, ImportedModuleCompletion, IndexInputSlot,
    InvalidationReason, LinkedModuleImport, LocatedCallHierarchyItem, LocatedIncomingCall,
    LocatedModuleExport, LocatedModuleGraph, LocatedNavigationTarget, LocatedOutgoingCall,
    LocatedProjectReference, LocatedRenamePreflightIssue, LocatedSymbolIdentity,
    LocatedWorkspaceSymbol, ObjectFieldHoverInfo, ParseInputSlot, PerFileQuerySupport,
    PerformanceStats, ProjectDiagnostic, ProjectDiagnosticKind, ProjectDiagnosticSeverity,
    ProjectDiagnosticTag, ProjectReferenceKind, ProjectReferences, ProjectRenamePlan,
    RemovedFileImpact, WorkspaceDependencyEdge, WorkspaceDependencyGraph, WorkspaceFileInfo,
};

#[cfg(test)]
mod tests;
