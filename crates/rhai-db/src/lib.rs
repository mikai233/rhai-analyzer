mod builtin;
mod change;
mod db;
mod infer;
mod overload;
mod project;
mod types;
mod workspace;

pub use change::{ChangeSet, FileChange};
pub use db::{AnalyzerDatabase, DatabaseSnapshot};
pub use infer::generics::specialize_signature_with_receiver_and_arg_types;
pub use overload::{best_matching_signature_index, best_matching_signature_indexes};
pub use types::{
    AutoImportCandidate, CachedMemberCompletionSet, CachedNavigationTarget, ChangeImpact,
    CompletionInputs, DatabaseDebugView, DebugFileAnalysis, FileAnalysisDependencies,
    FilePerformanceStats, FileTypeInference, HirInputSlot, HostConstant, HostFunction,
    HostFunctionOverload, HostModule, HostType, IndexInputSlot, InvalidationReason,
    LinkedModuleImport, LocatedModuleExport, LocatedModuleGraph, LocatedNavigationTarget,
    LocatedProjectReference, LocatedRenamePreflightIssue, LocatedSymbolIdentity,
    LocatedWorkspaceSymbol, ParseInputSlot, PerFileQuerySupport, PerformanceStats,
    ProjectDiagnostic, ProjectDiagnosticKind, ProjectReferenceKind, ProjectReferences,
    ProjectRenamePlan, RemovedFileImpact, WorkspaceDependencyEdge, WorkspaceDependencyGraph,
    WorkspaceFileInfo,
};

#[cfg(test)]
mod tests;
