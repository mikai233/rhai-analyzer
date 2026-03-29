use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::DatabaseSnapshot;
use crate::db::imports::imported_global_method_symbols;
use crate::db::navigation::workspace_symbol_match_rank;
use crate::db::rebuild::{resolved_source_roots, source_root_index_for_path};
use crate::types::{
    FileAnalysisDependencies, FilePerformanceStats, FileTypeInference, HostModule, HostType,
    LinkedModuleImport, LocatedModuleExport, LocatedModuleGraph, LocatedSymbolIdentity,
    LocatedWorkspaceSymbol, PerFileQuerySupport, PerformanceStats, SymbolIdentityKey,
    WorkspaceDependencyGraph, WorkspaceFileInfo,
};
use rhai_hir::{
    DocumentSymbol, ExternalSignatureIndex, FileBackedSymbolIdentity, FileHir, FileSymbolIndex,
    ModuleGraphIndex, SemanticDiagnostic, SymbolId, TypeRef, WorkspaceSymbol,
};
use rhai_project::ProjectConfig;
use rhai_syntax::{Parse, SyntaxError, TextSize};
use rhai_vfs::{FileId, VirtualFileSystem};

impl DatabaseSnapshot {
    pub fn vfs(&self) -> &VirtualFileSystem {
        &self.vfs
    }

    pub fn project(&self) -> &ProjectConfig {
        &self.project
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn project_revision(&self) -> u64 {
        self.project_revision
    }

    pub fn source_root_paths(&self) -> Vec<PathBuf> {
        resolved_source_roots(&self.project)
    }

    pub fn normalized_path(&self, file_id: FileId) -> Option<&Path> {
        self.vfs.file(file_id).map(|file| file.path())
    }

    pub fn source_root_index(&self, file_id: FileId) -> Option<usize> {
        let path = self.normalized_path(file_id)?;
        let roots = self.source_root_paths();
        source_root_index_for_path(path, &roots)
    }

    pub fn is_workspace_file(&self, file_id: FileId) -> bool {
        let Some(path) = self.normalized_path(file_id) else {
            return false;
        };
        let roots = self.source_root_paths();
        if roots.is_empty() {
            return true;
        }
        source_root_index_for_path(path, &roots).is_some()
    }

    pub fn workspace_files(&self) -> Vec<WorkspaceFileInfo> {
        let roots = self.source_root_paths();
        let mut files = self
            .vfs
            .iter()
            .map(|(file_id, file)| WorkspaceFileInfo {
                file_id,
                normalized_path: file.path().to_path_buf(),
                source_root: source_root_index_for_path(file.path(), &roots),
                is_workspace_file: roots.is_empty()
                    || source_root_index_for_path(file.path(), &roots).is_some(),
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.file_id.0.cmp(&right.file_id.0));
        files
    }

    pub fn external_signatures(&self) -> &ExternalSignatureIndex {
        &self.project_semantics.external_signatures
    }

    pub fn global_functions(&self) -> &[crate::HostFunction] {
        &self.project_semantics.global_functions
    }

    pub fn global_function(&self, name: &str) -> Option<&crate::HostFunction> {
        self.project_semantics
            .global_functions
            .iter()
            .find(|function| function.name == name)
    }

    pub fn host_modules(&self) -> &[HostModule] {
        &self.project_semantics.modules
    }

    pub fn host_types(&self) -> &[HostType] {
        &self.project_semantics.types
    }

    pub fn disabled_symbols(&self) -> &[String] {
        &self.project_semantics.disabled_symbols
    }

    pub fn custom_syntaxes(&self) -> &[String] {
        &self.project_semantics.custom_syntaxes
    }

    pub fn file_text(&self, file_id: FileId) -> Option<Arc<str>> {
        self.vfs.file_text(file_id)
    }

    pub fn analysis_dependencies(&self, file_id: FileId) -> Option<&FileAnalysisDependencies> {
        self.analysis
            .get(&file_id)
            .map(|analysis| analysis.dependencies.as_ref())
    }

    pub fn parse(&self, file_id: FileId) -> Option<Arc<Parse>> {
        self.analysis
            .get(&file_id)
            .map(|analysis| Arc::clone(&analysis.parse))
    }

    pub fn hir(&self, file_id: FileId) -> Option<Arc<FileHir>> {
        self.analysis
            .get(&file_id)
            .map(|analysis| Arc::clone(&analysis.hir))
    }

    pub fn syntax_diagnostics(&self, file_id: FileId) -> &[SyntaxError] {
        self.analysis
            .get(&file_id)
            .map_or(&[], |analysis| analysis.syntax_diagnostics.as_ref())
    }

    pub fn semantic_diagnostics(&self, file_id: FileId) -> &[SemanticDiagnostic] {
        self.analysis
            .get(&file_id)
            .map_or(&[], |analysis| analysis.semantic_diagnostics.as_ref())
    }

    pub fn file_symbol_index(&self, file_id: FileId) -> Option<Arc<FileSymbolIndex>> {
        self.analysis
            .get(&file_id)
            .map(|analysis| Arc::clone(&analysis.file_symbol_index))
    }

    pub fn document_symbols(&self, file_id: FileId) -> &[DocumentSymbol] {
        self.analysis
            .get(&file_id)
            .map_or(&[], |analysis| analysis.document_symbols.as_ref())
    }

    pub fn file_workspace_symbols(&self, file_id: FileId) -> &[WorkspaceSymbol] {
        self.analysis
            .get(&file_id)
            .map_or(&[], |analysis| analysis.workspace_symbols.as_ref())
    }

    pub fn module_graph(&self, file_id: FileId) -> Option<Arc<ModuleGraphIndex>> {
        self.analysis
            .get(&file_id)
            .map(|analysis| Arc::clone(&analysis.module_graph))
    }

    pub fn type_inference(&self, file_id: FileId) -> Option<&FileTypeInference> {
        self.analysis
            .get(&file_id)
            .map(|analysis| analysis.type_inference.as_ref())
    }

    pub fn inferred_expr_type_at(&self, file_id: FileId, offset: TextSize) -> Option<&TypeRef> {
        let analysis = self.analysis.get(&file_id)?;
        analysis
            .hir
            .expr_type_at_offset(offset, &analysis.type_inference.expr_types)
    }

    pub fn inferred_symbol_type(&self, file_id: FileId, symbol: SymbolId) -> Option<&TypeRef> {
        let analysis = self.analysis.get(&file_id)?;
        analysis
            .type_inference
            .symbol_types
            .get(&symbol)
            .or_else(|| analysis.hir.declared_symbol_type(symbol))
    }

    pub fn inferred_symbol_type_at(&self, file_id: FileId, offset: TextSize) -> Option<&TypeRef> {
        let analysis = self.analysis.get(&file_id)?;
        let symbol = analysis.hir.definition_at_offset(offset)?;
        self.inferred_symbol_type(file_id, symbol)
    }

    pub fn query_support(&self, file_id: FileId) -> Option<&PerFileQuerySupport> {
        self.analysis
            .get(&file_id)
            .and_then(|analysis| analysis.query_support.as_deref())
    }

    pub fn workspace_symbols(&self) -> &[LocatedWorkspaceSymbol] {
        self.workspace_symbols.as_ref()
    }

    pub fn workspace_module_graphs(&self) -> &[LocatedModuleGraph] {
        self.workspace_module_graphs.as_ref()
    }

    pub fn workspace_exports(&self) -> &[LocatedModuleExport] {
        self.workspace_exports.as_ref()
    }

    pub fn workspace_dependency_graph(&self) -> &WorkspaceDependencyGraph {
        &self.workspace_dependency_graph
    }

    pub fn dependency_files(&self, file_id: FileId) -> &[FileId] {
        self.workspace_dependency_graph
            .dependencies_by_file
            .get(&file_id)
            .map_or(&[], Arc::as_ref)
    }

    pub fn dependent_files(&self, file_id: FileId) -> &[FileId] {
        self.workspace_dependency_graph
            .dependents_by_file
            .get(&file_id)
            .map_or(&[], Arc::as_ref)
    }

    pub fn exports_named(&self, name: &str) -> &[LocatedModuleExport] {
        self.exports_by_name.get(name).map_or(&[], Arc::as_ref)
    }

    pub fn linked_imports(&self, file_id: FileId) -> &[LinkedModuleImport] {
        self.linked_imports.get(&file_id).map_or(&[], Arc::as_ref)
    }

    pub fn linked_import(&self, file_id: FileId, import: usize) -> Option<&LinkedModuleImport> {
        self.linked_imports(file_id)
            .iter()
            .find(|linked_import| linked_import.import == import)
    }

    pub fn imported_global_method_symbols(
        &self,
        file_id: FileId,
        receiver_ty: &TypeRef,
        method_name: &str,
    ) -> Vec<LocatedSymbolIdentity> {
        imported_global_method_symbols(self, file_id, receiver_ty, method_name)
    }

    pub fn stats(&self) -> &PerformanceStats {
        &self.stats
    }

    pub fn file_stats(&self, file_id: FileId) -> Option<&FilePerformanceStats> {
        self.file_stats.get(&file_id)
    }

    pub fn all_file_stats(&self) -> Vec<FilePerformanceStats> {
        let mut stats = self.file_stats.values().cloned().collect::<Vec<_>>();
        stats.sort_by_key(|entry| entry.file_id.0);
        stats
    }

    pub fn locate_symbol(&self, identity: &FileBackedSymbolIdentity) -> &[LocatedSymbolIdentity] {
        self.symbol_locations
            .get(&SymbolIdentityKey::from(identity))
            .map_or(&[], Arc::as_ref)
    }

    pub fn symbol_owner(&self, identity: &FileBackedSymbolIdentity) -> Option<FileId> {
        let locations = self.locate_symbol(identity);
        if locations.len() == 1 {
            Some(locations[0].file_id)
        } else {
            None
        }
    }

    pub fn workspace_symbols_matching(&self, query: &str) -> Vec<LocatedWorkspaceSymbol> {
        let needle = query.trim().to_ascii_lowercase();
        let mut matches = self
            .workspace_symbols
            .iter()
            .filter(|symbol| {
                needle.is_empty()
                    || symbol
                        .symbol
                        .name
                        .to_ascii_lowercase()
                        .contains(needle.as_str())
                    || symbol
                        .symbol
                        .stable_key
                        .container_path
                        .iter()
                        .any(|segment| segment.to_ascii_lowercase().contains(needle.as_str()))
            })
            .cloned()
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            workspace_symbol_match_rank(left, needle.as_str())
                .cmp(&workspace_symbol_match_rank(right, needle.as_str()))
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
                .then_with(|| left.file_id.0.cmp(&right.file_id.0))
        });
        matches
    }
}
