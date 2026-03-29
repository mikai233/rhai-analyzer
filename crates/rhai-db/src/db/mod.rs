use rhai_project::ProjectConfig;
use rhai_vfs::{FileId, VirtualFileSystem};
use std::collections::HashMap;
use std::sync::Arc;

use crate::project::build_project_semantics;
use crate::types::{
    CachedFileAnalysis, FilePerformanceStats, LinkedModuleImport, LocatedModuleExport,
    LocatedModuleGraph, LocatedSymbolIdentity, LocatedWorkspaceSymbol, PerformanceStats,
    ProjectSemantics, SymbolIdentityKey, WorkspaceDependencyGraph, WorkspaceIndexes,
};

mod diagnostics;
mod imports;
mod navigation;
mod query;
mod query_support;
mod rebuild;
mod rename;
mod snapshot;

#[derive(Debug, Clone)]
pub struct DatabaseSnapshot {
    vfs: Arc<VirtualFileSystem>,
    project: Arc<ProjectConfig>,
    revision: u64,
    project_revision: u64,
    project_semantics: Arc<ProjectSemantics>,
    analysis: Arc<HashMap<FileId, Arc<CachedFileAnalysis>>>,
    workspace_symbols: Arc<[LocatedWorkspaceSymbol]>,
    workspace_module_graphs: Arc<[LocatedModuleGraph]>,
    workspace_exports: Arc<[LocatedModuleExport]>,
    workspace_dependency_graph: Arc<WorkspaceDependencyGraph>,
    symbol_locations: Arc<HashMap<SymbolIdentityKey, Arc<[LocatedSymbolIdentity]>>>,
    exports_by_name: Arc<HashMap<String, Arc<[LocatedModuleExport]>>>,
    linked_imports: Arc<HashMap<FileId, Arc<[LinkedModuleImport]>>>,
    file_stats: Arc<HashMap<FileId, FilePerformanceStats>>,
    stats: Arc<PerformanceStats>,
}

#[derive(Debug)]
pub struct AnalyzerDatabase {
    vfs: VirtualFileSystem,
    project: ProjectConfig,
    revision: u64,
    project_revision: u64,
    next_analysis_revision: u64,
    next_query_support_ticket: u64,
    project_semantics: Arc<ProjectSemantics>,
    analysis: HashMap<FileId, Arc<CachedFileAnalysis>>,
    file_stats: HashMap<FileId, FilePerformanceStats>,
    query_support_budget: Option<usize>,
    query_support_tickets: HashMap<FileId, u64>,
    stats: PerformanceStats,
    pub(crate) workspace_indexes: WorkspaceIndexes,
}

impl Default for AnalyzerDatabase {
    fn default() -> Self {
        let project = ProjectConfig::default();
        let project_semantics = Arc::new(build_project_semantics(&project));

        Self {
            vfs: VirtualFileSystem::default(),
            project,
            revision: 0,
            project_revision: 0,
            next_analysis_revision: 0,
            next_query_support_ticket: 0,
            project_semantics,
            analysis: HashMap::new(),
            file_stats: HashMap::new(),
            query_support_budget: None,
            query_support_tickets: HashMap::new(),
            stats: PerformanceStats::default(),
            workspace_indexes: WorkspaceIndexes::default(),
        }
    }
}
