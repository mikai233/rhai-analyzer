use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use rhai_hir::{
    CompletionSymbol, DocumentSymbol, ExprId, ExprKind, ExternalSignatureIndex,
    FileBackedSymbolIdentity, FileHir, FileSymbolIndex, MemberCompletion, MemberCompletionSource,
    ModuleGraphIndex, NavigationTarget, RenamePreflightIssue, RenamePreflightIssueKind,
    SemanticDiagnostic, SemanticDiagnosticKind, SymbolId, TypeRef, WorkspaceSymbol, lower_file,
};
use rhai_project::ProjectConfig;
use rhai_syntax::{Parse, SyntaxError, TextSize, parse_text};
use rhai_vfs::{FileId, VirtualFileSystem, normalize_path};

use crate::change::{ChangeSet, FileChange};
use crate::infer::{ImportedMethodSignature, ImportedModuleMember, infer_file_types};
use crate::project::build_project_semantics;
use crate::types::{
    AutoImportCandidate, CachedFileAnalysis, CachedMemberCompletionSet, CachedNavigationTarget,
    ChangeImpact, CompletionInputs, DatabaseDebugView, DebugFileAnalysis, FileAnalysisDependencies,
    FilePerformanceStats, FileTypeInference, HirInputSlot, HostModule, HostType, IndexInputSlot,
    InvalidationReason, LinkedModuleImport, LocatedModuleExport, LocatedModuleGraph,
    LocatedNavigationTarget, LocatedProjectReference, LocatedRenamePreflightIssue,
    LocatedSymbolIdentity, LocatedWorkspaceSymbol, ParseInputSlot, PerFileQuerySupport,
    PerformanceStats, ProjectDiagnostic, ProjectDiagnosticKind, ProjectReferenceKind,
    ProjectReferences, ProjectRenamePlan, ProjectSemantics, RemovedFileImpact, SymbolIdentityKey,
    WorkspaceDependencyGraph, WorkspaceFileInfo, WorkspaceIndexes,
};

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

    pub fn project_diagnostics(&self, file_id: FileId) -> Vec<ProjectDiagnostic> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        let mut diagnostics = self
            .syntax_diagnostics(file_id)
            .iter()
            .map(|diagnostic| ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Syntax,
                range: diagnostic.range(),
                message: diagnostic.message().to_owned(),
                related_range: None,
            })
            .collect::<Vec<_>>();

        diagnostics.extend(project_semantic_diagnostics(
            self,
            file_id,
            analysis.hir.as_ref(),
            analysis.semantic_diagnostics.as_ref(),
        ));

        diagnostics.sort_by(|left, right| {
            left.range
                .start()
                .cmp(&right.range.start())
                .then_with(|| {
                    project_diagnostic_kind_rank(left.kind)
                        .cmp(&project_diagnostic_kind_rank(right.kind))
                })
                .then_with(|| left.message.cmp(&right.message))
        });
        diagnostics.dedup_by(|left, right| {
            left.kind == right.kind
                && left.range == right.range
                && left.related_range == right.related_range
                && left.message == right.message
        });
        diagnostics
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

    pub fn completion_inputs(&self, file_id: FileId, offset: TextSize) -> Option<CompletionInputs> {
        let analysis = self.analysis.get(&file_id)?;
        let visible_symbols = visible_completion_symbols(analysis, offset);
        let member_symbols = cached_member_completion_at(analysis, offset);
        let visible_names = visible_symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<HashSet<_>>();
        let project_symbols = self
            .workspace_symbols
            .iter()
            .filter(|symbol| !visible_names.contains(symbol.symbol.name.as_str()))
            .cloned()
            .collect();

        Some(CompletionInputs {
            file_id,
            offset,
            visible_symbols,
            project_symbols,
            member_symbols,
        })
    }

    pub fn auto_import_candidates(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<AutoImportCandidate> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };
        let Some(reference_id) = analysis.hir.reference_at_offset(offset) else {
            return Vec::new();
        };
        let reference = analysis.hir.reference(reference_id);
        if reference.target.is_some() || reference.kind != rhai_hir::ReferenceKind::Name {
            return Vec::new();
        }

        self.auto_import_candidates_for_name(file_id, reference.name.as_str())
    }

    pub fn auto_import_candidates_for_name(
        &self,
        file_id: FileId,
        name: &str,
    ) -> Vec<AutoImportCandidate> {
        let _ = (file_id, name);
        Vec::new()
    }

    pub fn goto_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedNavigationTarget> {
        self.project_targets_at(file_id, offset)
            .iter()
            .flat_map(|target| self.navigation_targets_for_identity(&target.symbol))
            .collect()
    }

    pub fn find_references(&self, file_id: FileId, offset: TextSize) -> Option<ProjectReferences> {
        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        Some(ProjectReferences {
            references: self.collect_project_references(&targets),
            targets,
        })
    }

    pub fn rename_plan(
        &self,
        file_id: FileId,
        offset: TextSize,
        new_name: impl Into<String>,
    ) -> Option<ProjectRenamePlan> {
        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        let new_name = new_name.into();
        let mut issues = Vec::new();

        for target in &targets {
            let Some(analysis) = self.analysis.get(&target.file_id) else {
                continue;
            };

            let local_plan = analysis
                .hir
                .rename_plan(target.symbol.symbol, new_name.clone());
            issues.extend(
                local_plan
                    .issues
                    .into_iter()
                    .map(|issue| LocatedRenamePreflightIssue {
                        file_id: target.file_id,
                        issue,
                    }),
            );
        }

        issues.extend(self.project_rename_preflight_issues(&targets, &new_name));

        Some(ProjectRenamePlan {
            occurrences: self.collect_project_references(&targets),
            targets,
            new_name,
            issues,
        })
    }

    pub fn debug_view(&self) -> DatabaseDebugView {
        let mut files = self
            .analysis
            .iter()
            .map(|(&file_id, analysis)| {
                let dependencies = analysis.dependencies.as_ref().clone();
                DebugFileAnalysis {
                    file_id,
                    normalized_path: dependencies.normalized_path.clone(),
                    document_version: dependencies.document_version,
                    source_root: dependencies.source_root,
                    is_workspace_file: dependencies.is_workspace_file,
                    dependencies,
                    stats: self
                        .file_stats
                        .get(&file_id)
                        .cloned()
                        .unwrap_or_else(|| default_file_stats(file_id)),
                }
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.file_id.0.cmp(&right.file_id.0));

        DatabaseDebugView {
            revision: self.revision,
            project_revision: self.project_revision,
            source_roots: self.source_root_paths(),
            files,
            stats: (*self.stats).clone(),
        }
    }

    fn navigation_targets_for_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedNavigationTarget> {
        let mut targets = self
            .locate_symbol(identity)
            .iter()
            .filter_map(|location| {
                let analysis = self.analysis.get(&location.file_id)?;
                Some(LocatedNavigationTarget {
                    file_id: location.file_id,
                    target: cached_navigation_target(analysis, location.symbol.symbol),
                })
            })
            .collect::<Vec<_>>();

        targets.sort_by(|left, right| {
            left.file_id.0.cmp(&right.file_id.0).then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
        });
        targets
            .dedup_by(|left, right| left.file_id == right.file_id && left.target == right.target);
        targets
    }

    fn project_targets_at(&self, file_id: FileId, offset: TextSize) -> Vec<LocatedSymbolIdentity> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        if let Some(reference_id) = analysis.hir.reference_at_offset(offset) {
            let path_targets = linked_import_targets_for_path_reference(
                self,
                file_id,
                analysis.hir.as_ref(),
                reference_id,
            );
            if !path_targets.is_empty() {
                return path_targets;
            }

            if let Some(symbol) = analysis.hir.definition_of(reference_id) {
                return self
                    .locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
                    .to_vec();
            }

            if let Some(import_index) = analysis
                .hir
                .imports
                .iter()
                .position(|import| import.module_reference == Some(reference_id))
                && let Some(linked_import) = self.linked_import(file_id, import_index)
            {
                return dedupe_symbol_locations(
                    linked_import
                        .exports
                        .iter()
                        .filter_map(project_identity_for_export)
                        .flat_map(|identity| self.locate_symbol(identity).iter().cloned())
                        .collect(),
                );
            }

            if analysis.hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
                && let Some(access) = analysis
                    .hir
                    .member_accesses
                    .iter()
                    .find(|access| access.field_reference == reference_id)
                && let Some(receiver_ty) = analysis
                    .hir
                    .expr_type(access.receiver, &analysis.type_inference.expr_types)
            {
                let imported = imported_global_method_symbols(
                    self,
                    file_id,
                    receiver_ty,
                    analysis.hir.reference(reference_id).name.as_str(),
                );
                if !imported.is_empty() {
                    return imported;
                }
            }

            return Vec::new();
        }

        let Some(symbol) = analysis.hir.symbol_at_offset(offset) else {
            return Vec::new();
        };

        self.locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
            .to_vec()
    }

    fn collect_project_references(
        &self,
        targets: &[LocatedSymbolIdentity],
    ) -> Vec<LocatedProjectReference> {
        let mut references = Vec::new();

        for target in targets {
            let Some(analysis) = self.analysis.get(&target.file_id) else {
                continue;
            };

            references.push(LocatedProjectReference {
                file_id: target.file_id,
                range: target.symbol.declaration_range,
                kind: ProjectReferenceKind::Definition,
            });

            references.extend(analysis.hir.references_to(target.symbol.symbol).map(
                |reference_id| LocatedProjectReference {
                    file_id: target.file_id,
                    range: analysis.hir.reference(reference_id).range,
                    kind: ProjectReferenceKind::Reference,
                },
            ));
        }

        references.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.range.start().cmp(&right.range.start()))
                .then_with(|| {
                    project_reference_kind_rank(left.kind)
                        .cmp(&project_reference_kind_rank(right.kind))
                })
        });
        references.dedup_by(|left, right| {
            left.file_id == right.file_id && left.range == right.range && left.kind == right.kind
        });
        references
    }

    fn exports_for_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Vec<&LocatedModuleExport> {
        self.workspace_exports
            .iter()
            .filter(|export| export_matches_identity(export, identity))
            .collect()
    }

    fn project_rename_preflight_issues(
        &self,
        targets: &[LocatedSymbolIdentity],
        new_name: &str,
    ) -> Vec<LocatedRenamePreflightIssue> {
        let mut issues = Vec::new();

        for target in targets {
            for export in self.exports_for_identity(&target.symbol) {
                if export.export.exported_name.as_deref() == Some(new_name) {
                    continue;
                }

                for conflict in self.exports_named(new_name) {
                    if same_export_edge(export, conflict) {
                        continue;
                    }

                    issues.push(LocatedRenamePreflightIssue {
                        file_id: target.file_id,
                        issue: RenamePreflightIssue {
                            kind: RenamePreflightIssueKind::DuplicateDefinition,
                            message: format!(
                                "renaming exported symbol `{}` to `{new_name}` would collide with another workspace export",
                                target.symbol.name
                            ),
                            range: target.symbol.declaration_range,
                            related_symbol: project_identity_for_export(conflict).cloned(),
                        },
                    });
                }
            }
        }

        issues.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.issue.range.start().cmp(&right.issue.range.start()))
                .then_with(|| left.issue.message.cmp(&right.issue.message))
        });
        issues.dedup_by(|left, right| {
            left.file_id == right.file_id
                && left.issue.range == right.issue.range
                && left.issue.kind == right.issue.kind
                && left.issue.message == right.issue.message
        });
        issues
    }
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

impl AnalyzerDatabase {
    pub fn apply_change(&mut self, change_set: ChangeSet) {
        let _ = self.apply_change_report(change_set);
    }

    pub fn apply_change_report(&mut self, change_set: ChangeSet) -> ChangeImpact {
        let mut changed_files = Vec::new();
        let previous_dependency_graph =
            Arc::clone(&self.workspace_indexes.workspace_dependency_graph);
        let mut removed_files = Vec::new();
        let project_change = change_set.project.clone();

        for path in change_set.removed_files {
            if let Some(file_id) = self.vfs.remove_file(&path) {
                self.analysis.remove(&file_id);
                self.query_support_tickets.remove(&file_id);
                self.file_stats.remove(&file_id);
                removed_files.push(RemovedFileImpact {
                    file_id,
                    normalized_path: normalize_path(&path),
                });
            }
        }

        for change in coalesce_file_changes(change_set.files) {
            if !self.should_apply_file_change(&change) {
                continue;
            }

            let reason = if self.vfs.file_id(&change.path).is_some() {
                InvalidationReason::TextChanged
            } else {
                InvalidationReason::InitialLoad
            };
            let file_id = self.vfs.set_file(change.path, change.text, change.version);
            changed_files.push((file_id, reason));
        }

        if !removed_files.is_empty() || !changed_files.is_empty() || project_change.is_some() {
            self.revision += 1;
        }

        if let Some(project) = project_change {
            self.project = project;
            self.project_revision += 1;
            self.project_semantics = Arc::new(build_project_semantics(&self.project));
            self.analysis.clear();
            self.query_support_tickets.clear();
            self.rebuild_all_file_analysis(InvalidationReason::ProjectChanged);
            self.workspace_indexes.rebuild_from_analysis(&self.analysis);
            self.rebuild_workspace_type_inference();

            let rebuilt_files = sorted_file_ids(self.analysis.keys().copied().collect());
            let evicted_query_support_files = self.enforce_query_support_budget();
            self.refresh_file_stats_metadata();

            return ChangeImpact {
                revision: self.revision,
                project_revision: self.project_revision,
                project_changed: true,
                changed_files: sorted_file_ids(
                    changed_files
                        .into_iter()
                        .map(|(file_id, _)| file_id)
                        .collect(),
                ),
                rebuilt_files,
                removed_files,
                dependency_affected_files: sorted_file_ids(self.analysis.keys().copied().collect()),
                evicted_query_support_files,
            };
        }

        for removed in &removed_files {
            self.workspace_indexes
                .replace_file(removed.file_id, None, &self.analysis);
        }

        let mut rebuilt_files = Vec::new();
        for (file_id, reason) in changed_files {
            self.rebuild_file_analysis(file_id, reason);
            self.workspace_indexes.replace_file(
                file_id,
                self.analysis.get(&file_id),
                &self.analysis,
            );
            rebuilt_files.push(file_id);
        }

        if !rebuilt_files.is_empty() || !removed_files.is_empty() {
            self.rebuild_workspace_type_inference();
        }

        let evicted_query_support_files = self.enforce_query_support_budget();
        self.refresh_file_stats_metadata();

        let dependency_affected_files = self.collect_dependency_affected_files(
            &previous_dependency_graph,
            &removed_files,
            &rebuilt_files,
        );

        ChangeImpact {
            revision: self.revision,
            project_revision: self.project_revision,
            project_changed: false,
            changed_files: sorted_file_ids(rebuilt_files.clone()),
            rebuilt_files: sorted_file_ids(rebuilt_files),
            removed_files,
            dependency_affected_files,
            evicted_query_support_files,
        }
    }

    pub fn query_support_budget(&self) -> Option<usize> {
        self.query_support_budget
    }

    pub fn set_query_support_budget(&mut self, budget: Option<usize>) -> Vec<FileId> {
        self.query_support_budget = budget;
        let evicted = self.enforce_query_support_budget();
        self.refresh_file_stats_metadata();
        evicted
    }

    pub fn warm_query_support(&mut self, file_ids: &[FileId]) -> usize {
        let mut warmed = 0;
        for &file_id in file_ids {
            warmed += usize::from(self.ensure_query_support(file_id));
        }
        let _ = self.enforce_query_support_budget();
        self.refresh_file_stats_metadata();
        warmed
    }

    pub fn warm_workspace_queries(&mut self) -> usize {
        let mut file_ids = self.analysis.keys().copied().collect::<Vec<_>>();
        file_ids.sort_by_key(|file_id| file_id.0);
        self.warm_query_support(&file_ids)
    }

    pub fn snapshot(&self) -> DatabaseSnapshot {
        DatabaseSnapshot {
            vfs: Arc::new(self.vfs.clone()),
            project: Arc::new(self.project.clone()),
            revision: self.revision,
            project_revision: self.project_revision,
            project_semantics: Arc::clone(&self.project_semantics),
            analysis: Arc::new(self.analysis.clone()),
            workspace_symbols: Arc::clone(&self.workspace_indexes.workspace_symbols),
            workspace_module_graphs: Arc::clone(&self.workspace_indexes.workspace_module_graphs),
            workspace_exports: Arc::clone(&self.workspace_indexes.workspace_exports),
            workspace_dependency_graph: Arc::clone(
                &self.workspace_indexes.workspace_dependency_graph,
            ),
            symbol_locations: Arc::clone(&self.workspace_indexes.symbol_locations),
            exports_by_name: Arc::clone(&self.workspace_indexes.exports_by_name),
            linked_imports: Arc::clone(&self.workspace_indexes.linked_imports),
            file_stats: Arc::new(self.file_stats.clone()),
            stats: Arc::new(self.stats.clone()),
        }
    }

    fn should_apply_file_change(&self, change: &FileChange) -> bool {
        let Some(file_id) = self.vfs.file_id(&change.path) else {
            return true;
        };
        let Some(current) = self.vfs.file(file_id) else {
            return true;
        };

        if change.version < current.version() {
            return false;
        }

        !(change.version == current.version() && change.text == current.text())
    }

    fn rebuild_all_file_analysis(&mut self, reason: InvalidationReason) {
        let file_ids: Vec<_> = self.vfs.iter().map(|(file_id, _)| file_id).collect();
        for file_id in file_ids {
            self.rebuild_file_analysis(file_id, reason);
        }
    }

    fn rebuild_file_analysis(&mut self, file_id: FileId, reason: InvalidationReason) {
        let Some(file) = self.vfs.file(file_id) else {
            self.analysis.remove(&file_id);
            self.query_support_tickets.remove(&file_id);
            self.file_stats.remove(&file_id);
            return;
        };

        let analysis_revision = self.next_analysis_revision;
        self.next_analysis_revision += 1;

        let parse_started = Instant::now();
        let parse = Arc::new(parse_text(file.text()));
        self.stats.parse_rebuilds += 1;
        self.stats.total_parse_time += parse_started.elapsed();

        let lower_started = Instant::now();
        let hir = Arc::new(lower_file(&parse));
        self.stats.lower_rebuilds += 1;
        self.stats.total_lower_time += lower_started.elapsed();

        let index_started = Instant::now();
        let syntax_diagnostics = Arc::<[SyntaxError]>::from(parse.errors().to_vec());
        let semantic_diagnostics = Arc::<[SemanticDiagnostic]>::from(hir.diagnostics());
        let file_symbol_index = Arc::new(hir.file_symbol_index());
        let document_symbols = Arc::<[DocumentSymbol]>::from(hir.document_symbols());
        let workspace_symbols = Arc::<[WorkspaceSymbol]>::from(hir.workspace_symbols());
        let module_graph = Arc::new(hir.module_graph_index());
        let imported_methods = self.imported_method_signatures(file_id);
        let imported_members = self.imported_module_members(file_id);
        let type_inference = Arc::new(infer_file_types(
            &hir,
            &self.project_semantics.external_signatures,
            &self.project_semantics.global_functions,
            &self.project_semantics.types,
            &imported_methods,
            &imported_members,
            &HashMap::new(),
        ));
        self.stats.index_rebuilds += 1;
        self.stats.total_index_time += index_started.elapsed();

        let normalized_path = file.path().to_path_buf();
        let source_roots = resolved_source_roots(&self.project);
        let source_root = source_root_index_for_path(file.path(), &source_roots);
        let is_workspace_file = source_roots.is_empty() || source_root.is_some();
        let dependencies = Arc::new(FileAnalysisDependencies {
            file_id,
            normalized_path: normalized_path.clone(),
            document_version: file.version(),
            source_root,
            is_workspace_file,
            parse: ParseInputSlot {
                normalized_path,
                document_version: file.version(),
            },
            hir: HirInputSlot {
                parse_revision: analysis_revision,
                project_revision: self.project_revision,
            },
            index: IndexInputSlot {
                hir_revision: analysis_revision,
                project_revision: self.project_revision,
            },
            last_invalidation: reason,
        });

        self.analysis.insert(
            file_id,
            Arc::new(CachedFileAnalysis {
                parse,
                hir,
                syntax_diagnostics,
                semantic_diagnostics,
                file_symbol_index,
                document_symbols,
                workspace_symbols,
                module_graph,
                type_inference,
                dependencies,
                query_support: None,
            }),
        );
        self.query_support_tickets.remove(&file_id);

        let entry = self
            .file_stats
            .entry(file_id)
            .or_insert_with(|| default_file_stats(file_id));
        entry.normalized_path = file.path().to_path_buf();
        entry.parse_rebuilds += 1;
        entry.lower_rebuilds += 1;
        entry.index_rebuilds += 1;
        entry.query_support_cached = false;
    }

    fn rebuild_workspace_type_inference(&mut self) {
        if self.analysis.is_empty() {
            return;
        }

        let max_iterations = self.analysis.len()
            + self
                .workspace_indexes
                .linked_imports
                .values()
                .map(|imports| imports.len())
                .sum::<usize>()
            + 1;
        let mut applied_seeds = HashMap::<FileId, HashMap<SymbolId, TypeRef>>::new();
        let mut force_recompute = true;

        for _ in 0..max_iterations.max(1) {
            let next_seeds = self.derive_workspace_type_seeds();
            let mut changed_files = BTreeMap::<FileId, Option<HashMap<SymbolId, TypeRef>>>::new();

            if force_recompute {
                for &file_id in self.analysis.keys() {
                    changed_files.insert(
                        file_id,
                        Some(applied_seeds.get(&file_id).cloned().unwrap_or_default()),
                    );
                }
            }
            for &file_id in applied_seeds.keys() {
                changed_files.entry(file_id).or_insert(None);
            }
            for (&file_id, seeds) in &next_seeds {
                changed_files.insert(file_id, Some(seeds.clone()));
            }

            let mut changed = false;
            for (file_id, maybe_seeds) in changed_files {
                let next = maybe_seeds.unwrap_or_default();
                if applied_seeds.get(&file_id) == Some(&next) {
                    continue;
                }

                let Some(existing) = self.analysis.get(&file_id).cloned() else {
                    continue;
                };
                let imported_methods = self.imported_method_signatures(file_id);
                let imported_members = self.imported_module_members(file_id);
                let type_inference = Arc::new(infer_file_types(
                    &existing.hir,
                    &self.project_semantics.external_signatures,
                    &self.project_semantics.global_functions,
                    &self.project_semantics.types,
                    &imported_methods,
                    &imported_members,
                    &next,
                ));

                self.analysis.insert(
                    file_id,
                    Arc::new(CachedFileAnalysis {
                        type_inference,
                        ..(*existing).clone()
                    }),
                );
                if next.is_empty() {
                    applied_seeds.remove(&file_id);
                } else {
                    applied_seeds.insert(file_id, next);
                }
                changed = true;
            }

            if !changed {
                break;
            }
            force_recompute = false;
        }
    }

    fn derive_workspace_type_seeds(&self) -> HashMap<FileId, HashMap<SymbolId, TypeRef>> {
        HashMap::new()
    }

    fn imported_method_signatures(&self, file_id: FileId) -> Vec<ImportedMethodSignature> {
        self.workspace_indexes
            .linked_imports
            .get(&file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
            .flat_map(|linked_import| linked_import.exports.iter())
            .filter_map(|export| {
                let identity = project_identity_for_export(export)?;
                (identity.kind == rhai_hir::SymbolKind::Function)
                    .then_some((export.file_id, identity))
            })
            .filter_map(|(provider_file_id, identity)| {
                let provider_hir = self.analysis.get(&provider_file_id)?.hir.clone();
                let this_type = provider_hir
                    .function_info(identity.symbol)?
                    .this_type
                    .clone()?;
                let signature = match provider_hir.symbol(identity.symbol).annotation.as_ref()? {
                    TypeRef::Function(signature) => signature.clone(),
                    _ => return None,
                };
                Some(ImportedMethodSignature {
                    name: identity.name.clone(),
                    receiver: this_type,
                    signature,
                })
            })
            .collect()
    }

    fn imported_module_members(&self, file_id: FileId) -> Vec<ImportedModuleMember> {
        let Some(importer_hir) = self
            .analysis
            .get(&file_id)
            .map(|analysis| analysis.hir.clone())
        else {
            return Vec::new();
        };
        let mut members = Vec::new();
        for linked_import in self
            .workspace_indexes
            .linked_imports
            .get(&file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
        {
            let Some(alias) = importer_hir.import(linked_import.import).alias else {
                continue;
            };
            let module_path = vec![importer_hir.symbol(alias).name.clone()];
            self.collect_imported_module_members(
                linked_import,
                &module_path,
                &mut Vec::new(),
                &mut members,
            );
        }
        members
    }

    fn collect_imported_module_members(
        &self,
        linked_import: &LinkedModuleImport,
        module_path: &[String],
        visited_files: &mut Vec<FileId>,
        members: &mut Vec<ImportedModuleMember>,
    ) {
        let provider_file_id = linked_import.provider_file_id;
        if visited_files.contains(&provider_file_id) {
            return;
        }
        visited_files.push(provider_file_id);

        for export in linked_import.exports.iter() {
            let Some(exported_name) = export.export.exported_name.as_ref() else {
                continue;
            };
            let Some(identity) = project_identity_for_export(export) else {
                continue;
            };
            let Some(provider_analysis) = self.analysis.get(&export.file_id) else {
                continue;
            };
            let Some(ty) = provider_analysis
                .type_inference
                .symbol_types
                .get(&identity.symbol)
                .cloned()
                .or_else(|| {
                    provider_analysis
                        .hir
                        .declared_symbol_type(identity.symbol)
                        .cloned()
                })
            else {
                continue;
            };
            members.push(ImportedModuleMember {
                module_path: module_path.to_vec(),
                name: exported_name.clone(),
                ty,
            });
        }

        let Some(provider_hir) = self
            .analysis
            .get(&provider_file_id)
            .map(|analysis| analysis.hir.clone())
        else {
            visited_files.pop();
            return;
        };
        for nested in self
            .workspace_indexes
            .linked_imports
            .get(&provider_file_id)
            .into_iter()
            .flat_map(|imports| imports.iter())
        {
            let Some(alias) = provider_hir.import(nested.import).alias else {
                continue;
            };
            let mut nested_path = module_path.to_vec();
            nested_path.push(provider_hir.symbol(alias).name.clone());
            self.collect_imported_module_members(nested, &nested_path, visited_files, members);
        }

        visited_files.pop();
    }

    fn ensure_query_support(&mut self, file_id: FileId) -> bool {
        let Some(existing) = self.analysis.get(&file_id).cloned() else {
            return false;
        };
        if existing.query_support.is_some() {
            return false;
        }

        let query_started = Instant::now();
        let query_support = Arc::new(build_query_support(
            file_id,
            existing.dependencies.normalized_path.clone(),
            &existing.hir,
        ));
        self.stats.query_support_rebuilds += 1;
        self.stats.total_query_support_time += query_started.elapsed();

        let mut updated = (*existing).clone();
        updated.query_support = Some(query_support);
        self.analysis.insert(file_id, Arc::new(updated));
        self.touch_query_support(file_id);
        let entry = self
            .file_stats
            .entry(file_id)
            .or_insert_with(|| default_file_stats(file_id));
        entry.normalized_path = existing.dependencies.normalized_path.clone();
        entry.query_support_rebuilds += 1;
        entry.query_support_cached = true;
        true
    }

    fn touch_query_support(&mut self, file_id: FileId) {
        self.next_query_support_ticket += 1;
        self.query_support_tickets
            .insert(file_id, self.next_query_support_ticket);
    }

    fn enforce_query_support_budget(&mut self) -> Vec<FileId> {
        let Some(budget) = self.query_support_budget else {
            return Vec::new();
        };

        let cached_count = self
            .analysis
            .values()
            .filter(|analysis| analysis.query_support.is_some())
            .count();
        if cached_count <= budget {
            return Vec::new();
        }

        let mut ranked = self
            .query_support_tickets
            .iter()
            .filter_map(|(&file_id, &ticket)| {
                self.analysis
                    .get(&file_id)
                    .and_then(|analysis| analysis.query_support.as_ref().map(|_| (file_id, ticket)))
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(_, ticket)| *ticket);

        let mut evicted = Vec::new();
        while self
            .analysis
            .values()
            .filter(|analysis| analysis.query_support.is_some())
            .count()
            > budget
        {
            let Some((file_id, _)) = ranked.first().copied() else {
                break;
            };
            ranked.remove(0);
            if self.evict_query_support(file_id) {
                evicted.push(file_id);
            }
        }

        evicted.sort_by_key(|file_id| file_id.0);
        evicted
    }

    fn evict_query_support(&mut self, file_id: FileId) -> bool {
        let Some(existing) = self.analysis.get(&file_id).cloned() else {
            return false;
        };
        if existing.query_support.is_none() {
            return false;
        }

        let mut updated = (*existing).clone();
        updated.query_support = None;
        self.analysis.insert(file_id, Arc::new(updated));
        self.query_support_tickets.remove(&file_id);
        self.stats.query_support_evictions += 1;

        let entry = self
            .file_stats
            .entry(file_id)
            .or_insert_with(|| default_file_stats(file_id));
        entry.query_support_evictions += 1;
        entry.query_support_cached = false;
        true
    }

    fn refresh_file_stats_metadata(&mut self) {
        let mut active_file_ids = self.analysis.keys().copied().collect::<Vec<_>>();
        active_file_ids.sort_by_key(|file_id| file_id.0);

        for file_id in active_file_ids {
            let dependency_count = self
                .workspace_indexes
                .workspace_dependency_graph
                .dependencies_by_file
                .get(&file_id)
                .map_or(0, |files| files.len());
            let dependent_count = self
                .workspace_indexes
                .workspace_dependency_graph
                .dependents_by_file
                .get(&file_id)
                .map_or(0, |files| files.len());
            let normalized_path = self
                .analysis
                .get(&file_id)
                .map(|analysis| analysis.dependencies.normalized_path.clone())
                .unwrap_or_default();
            let query_support_cached = self
                .analysis
                .get(&file_id)
                .is_some_and(|analysis| analysis.query_support.is_some());

            let entry = self
                .file_stats
                .entry(file_id)
                .or_insert_with(|| default_file_stats(file_id));
            entry.normalized_path = normalized_path;
            entry.dependency_count = dependency_count;
            entry.dependent_count = dependent_count;
            entry.query_support_cached = query_support_cached;
        }

        self.file_stats
            .retain(|file_id, _| self.analysis.contains_key(file_id));
    }

    fn collect_dependency_affected_files(
        &self,
        previous_dependency_graph: &WorkspaceDependencyGraph,
        removed_files: &[RemovedFileImpact],
        rebuilt_files: &[FileId],
    ) -> Vec<FileId> {
        let mut affected = BTreeMap::<FileId, ()>::new();

        for &file_id in rebuilt_files {
            for dependent in previous_dependency_graph
                .dependents_by_file
                .get(&file_id)
                .into_iter()
                .flat_map(|files| files.iter().copied())
            {
                affected.insert(dependent, ());
            }
            for dependent in self
                .workspace_indexes
                .workspace_dependency_graph
                .dependents_by_file
                .get(&file_id)
                .into_iter()
                .flat_map(|files| files.iter().copied())
            {
                affected.insert(dependent, ());
            }
        }

        for removed in removed_files {
            for dependent in previous_dependency_graph
                .dependents_by_file
                .get(&removed.file_id)
                .into_iter()
                .flat_map(|files| files.iter().copied())
            {
                affected.insert(dependent, ());
            }
        }

        for &file_id in rebuilt_files {
            affected.remove(&file_id);
        }
        for removed in removed_files {
            affected.remove(&removed.file_id);
        }

        affected.into_keys().collect()
    }
}

fn workspace_symbol_match_rank(symbol: &LocatedWorkspaceSymbol, query: &str) -> (u8, u8, String) {
    let name = symbol.symbol.name.to_ascii_lowercase();
    let container = symbol
        .symbol
        .stable_key
        .container_path
        .join("::")
        .to_ascii_lowercase();

    let name_rank = if query.is_empty() || name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else if container.contains(query) {
        3
    } else {
        4
    };

    let export_rank = if symbol.symbol.exported { 0 } else { 1 };
    (name_rank, export_rank, name)
}

fn project_diagnostic_kind_rank(kind: ProjectDiagnosticKind) -> u8 {
    match kind {
        ProjectDiagnosticKind::Syntax => 0,
        ProjectDiagnosticKind::Semantic => 1,
        ProjectDiagnosticKind::BrokenLinkedImport => 2,
        ProjectDiagnosticKind::AmbiguousLinkedImport => 3,
    }
}

fn project_reference_kind_rank(kind: ProjectReferenceKind) -> u8 {
    match kind {
        ProjectReferenceKind::Definition => 0,
        ProjectReferenceKind::Reference => 1,
        ProjectReferenceKind::LinkedImport => 2,
    }
}

fn project_identity_for_export(export: &LocatedModuleExport) -> Option<&FileBackedSymbolIdentity> {
    export
        .export
        .alias
        .as_ref()
        .or(export.export.target.as_ref())
}

fn linked_import_targets_for_path_reference(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    reference_id: rhai_hir::ReferenceId,
) -> Vec<LocatedSymbolIdentity> {
    let Some(path_expr) = enclosing_path_expr(hir, hir.reference(reference_id).range.start())
    else {
        return Vec::new();
    };
    let Some(path_parts) = linked_import_path_parts(hir, path_expr) else {
        return Vec::new();
    };
    resolve_linked_import_path_targets(snapshot, file_id, &path_parts)
}

fn export_matches_identity(
    export: &LocatedModuleExport,
    identity: &FileBackedSymbolIdentity,
) -> bool {
    export
        .export
        .alias
        .as_ref()
        .is_some_and(|alias| alias == identity)
        || (export.export.alias.is_none()
            && export
                .export
                .target
                .as_ref()
                .is_some_and(|target| target == identity))
}

fn dedupe_symbol_locations(
    mut locations: Vec<LocatedSymbolIdentity>,
) -> Vec<LocatedSymbolIdentity> {
    locations.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    locations.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    locations
}

fn enclosing_path_expr(hir: &FileHir, offset: TextSize) -> Option<ExprId> {
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(_, expr)| expr.kind == ExprKind::Path && expr.range.contains(offset))
        .min_by_key(|(_, expr)| expr.range.len())
        .map(|(index, _)| ExprId(index as u32))
}

fn linked_import_path_parts(hir: &FileHir, expr: ExprId) -> Option<Vec<String>> {
    let range = hir.expr(expr).range;
    let mut references = hir
        .references
        .iter()
        .enumerate()
        .filter(|(_, reference)| {
            matches!(
                reference.kind,
                rhai_hir::ReferenceKind::Name | rhai_hir::ReferenceKind::PathSegment
            ) && reference.range.start() >= range.start()
                && reference.range.end() <= range.end()
        })
        .collect::<Vec<_>>();

    references.sort_by_key(|(_, reference)| reference.range.start());
    let (first_index, first_reference) = references.first()?;
    let alias_symbol = (first_reference.kind == rhai_hir::ReferenceKind::Name)
        .then(|| hir.definition_of(rhai_hir::ReferenceId(*first_index as u32)))
        .flatten()?;
    if hir.symbol(alias_symbol).kind != rhai_hir::SymbolKind::ImportAlias {
        return None;
    }
    Some(
        references
            .into_iter()
            .map(|(_, reference)| reference.name.clone())
            .collect(),
    )
}

fn resolve_linked_import_path_targets(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    path_parts: &[String],
) -> Vec<LocatedSymbolIdentity> {
    resolve_linked_import_path_targets_inner(snapshot, file_id, path_parts, &mut Vec::new())
}

fn resolve_linked_import_path_targets_inner(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    path_parts: &[String],
    visited_files: &mut Vec<FileId>,
) -> Vec<LocatedSymbolIdentity> {
    let [alias_name, rest @ ..] = path_parts else {
        return Vec::new();
    };
    if rest.is_empty() || visited_files.contains(&file_id) {
        return Vec::new();
    }
    visited_files.push(file_id);

    let Some(linked_import) = linked_import_for_alias_name(snapshot, file_id, alias_name) else {
        visited_files.pop();
        return Vec::new();
    };

    let result = if rest.len() == 1 {
        let member_name = &rest[0];
        dedupe_symbol_locations(
            linked_import
                .exports
                .iter()
                .filter(|export| {
                    export.export.exported_name.as_deref() == Some(member_name.as_str())
                })
                .filter_map(project_identity_for_export)
                .flat_map(|identity| snapshot.locate_symbol(identity).iter().cloned())
                .collect(),
        )
    } else {
        let provider_file_id = linked_import.provider_file_id;
        resolve_linked_import_path_targets_inner(snapshot, provider_file_id, rest, visited_files)
    };

    visited_files.pop();
    result
}

fn linked_import_for_alias_name<'a>(
    snapshot: &'a DatabaseSnapshot,
    file_id: FileId,
    alias_name: &str,
) -> Option<&'a LinkedModuleImport> {
    let hir = snapshot.hir(file_id)?;
    snapshot
        .linked_imports(file_id)
        .iter()
        .find(|linked_import| {
            hir.import(linked_import.import)
                .alias
                .is_some_and(|alias| hir.symbol(alias).name == alias_name)
        })
}

fn project_semantic_diagnostics(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    diagnostics: &[SemanticDiagnostic],
) -> Vec<ProjectDiagnostic> {
    let mut projected = Vec::new();

    for diagnostic in diagnostics {
        if diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && unresolved_name_is_known_external(snapshot, hir, diagnostic)
        {
            continue;
        }

        if diagnostic.kind != SemanticDiagnosticKind::UnresolvedImport {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
            });
            continue;
        }

        let Some(import_index) = import_index_for_diagnostic(hir, diagnostic) else {
            projected.push(ProjectDiagnostic {
                kind: ProjectDiagnosticKind::Semantic,
                range: diagnostic.range,
                message: diagnostic.message.clone(),
                related_range: diagnostic.related_range,
            });
            continue;
        };

        match snapshot.linked_import(file_id, import_index) {
            Some(linked_import) if linked_import.exports.len() == 1 => {}
            Some(linked_import) => {
                projected.push(ProjectDiagnostic {
                    kind: ProjectDiagnosticKind::AmbiguousLinkedImport,
                    range: diagnostic.range,
                    message: format!(
                        "ambiguous import module `{}` matches multiple workspace exports",
                        linked_import.module_name
                    ),
                    related_range: Some(hir.import(import_index).range),
                });
                projected.extend(linked_import_usage_diagnostics(
                    hir,
                    import_index,
                    ProjectDiagnosticKind::AmbiguousLinkedImport,
                    format!(
                        "import alias cannot be resolved uniquely because module `{}` matches multiple workspace exports",
                        linked_import.module_name
                    ),
                ));
            }
            None => {
                projected.push(ProjectDiagnostic {
                    kind: ProjectDiagnosticKind::Semantic,
                    range: diagnostic.range,
                    message: diagnostic.message.clone(),
                    related_range: diagnostic.related_range,
                });
                projected.extend(linked_import_usage_diagnostics(
                    hir,
                    import_index,
                    ProjectDiagnosticKind::BrokenLinkedImport,
                    format!(
                        "import alias no longer resolves because module `{}` is unavailable in the workspace",
                        hir.reference(
                            hir.import(import_index)
                                .module_reference
                                .expect("expected import reference")
                        )
                        .name
                    ),
                ));
            }
        }
    }

    projected
}

fn unresolved_name_is_known_external(
    snapshot: &DatabaseSnapshot,
    hir: &FileHir,
    diagnostic: &SemanticDiagnostic,
) -> bool {
    let Some(reference_id) = hir
        .reference_at(diagnostic.range)
        .or_else(|| hir.reference_at_offset(diagnostic.range.start()))
    else {
        return false;
    };
    let name = hir.reference(reference_id).name.as_str();
    snapshot.external_signatures().get(name).is_some() || snapshot.global_function(name).is_some()
}

fn import_index_for_diagnostic(hir: &FileHir, diagnostic: &SemanticDiagnostic) -> Option<usize> {
    hir.imports.iter().position(|import| {
        import
            .module_reference
            .is_some_and(|reference_id| hir.reference(reference_id).range == diagnostic.range)
    })
}

fn linked_import_usage_diagnostics(
    hir: &FileHir,
    import_index: usize,
    kind: ProjectDiagnosticKind,
    message: String,
) -> Vec<ProjectDiagnostic> {
    let Some(alias_symbol) = hir.import(import_index).alias else {
        return Vec::new();
    };

    hir.references_to(alias_symbol)
        .map(|reference_id| ProjectDiagnostic {
            kind,
            range: hir.reference(reference_id).range,
            message: message.clone(),
            related_range: Some(hir.import(import_index).range),
        })
        .collect()
}

fn imported_global_method_symbols(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    receiver_ty: &TypeRef,
    method_name: &str,
) -> Vec<LocatedSymbolIdentity> {
    let mut matches = snapshot
        .linked_imports(file_id)
        .iter()
        .flat_map(|linked_import| linked_import.exports.iter())
        .filter_map(|export| {
            let identity = project_identity_for_export(export)?;
            (identity.kind == rhai_hir::SymbolKind::Function && identity.name == method_name)
                .then_some((export.file_id, identity))
        })
        .filter_map(|(provider_file_id, identity)| {
            let provider_hir = snapshot.hir(provider_file_id)?;
            let this_type = provider_hir
                .function_info(identity.symbol)?
                .this_type
                .as_ref()?;
            receiver_matches_method_type(receiver_ty, this_type)
                .then_some(snapshot.locate_symbol(identity).iter().cloned())
        })
        .flatten()
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.symbol
                    .declaration_range
                    .start()
                    .cmp(&right.symbol.declaration_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    matches.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    matches
}

fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) => items
            .iter()
            .any(|item| receiver_matches_method_type(item, expected)),
        (TypeRef::Nullable(inner), expected) => receiver_matches_method_type(inner, expected),
        (TypeRef::Applied { name, .. }, TypeRef::Named(expected_name))
        | (
            TypeRef::Named(name),
            TypeRef::Applied {
                name: expected_name,
                ..
            },
        ) => name == expected_name,
        (
            TypeRef::Applied { name, args },
            TypeRef::Applied {
                name: expected_name,
                args: expected_args,
            },
        ) => {
            name == expected_name
                && args.len() == expected_args.len()
                && args
                    .iter()
                    .zip(expected_args.iter())
                    .all(|(arg, expected)| receiver_matches_method_type(arg, expected))
        }
        _ => false,
    }
}

fn visible_completion_symbols(
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<CompletionSymbol> {
    let Some(query_support) = analysis.query_support.as_ref() else {
        return analysis.hir.completion_symbols_at(offset);
    };

    analysis
        .hir
        .visible_symbols_at(offset)
        .into_iter()
        .filter_map(|symbol| {
            query_support
                .completion_symbols_by_symbol
                .get(&symbol)
                .cloned()
        })
        .collect()
}

fn cached_member_completion_at(
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<MemberCompletion> {
    let Some(query_support) = analysis.query_support.as_ref() else {
        return analysis.hir.member_completion_at(offset);
    };

    let Some(access) = analysis
        .hir
        .member_accesses
        .iter()
        .filter(|access| {
            access.range.contains(offset)
                || analysis
                    .hir
                    .reference(access.field_reference)
                    .range
                    .contains(offset)
        })
        .min_by_key(|access| access.range.len())
    else {
        return Vec::new();
    };

    let mut members = BTreeMap::<String, MemberCompletion>::new();
    for member in object_field_member_completions(&analysis.hir, access.receiver) {
        members.entry(member.name.clone()).or_insert(member);
    }

    if let Some(symbol) = symbol_for_expr(&analysis.hir, access.receiver)
        && let Some(cached) = query_support.member_completion_sets_by_symbol.get(&symbol)
    {
        for member in cached.iter().cloned() {
            members.entry(member.name.clone()).or_insert(member);
        }
    }

    members.into_values().collect()
}

fn cached_navigation_target(analysis: &CachedFileAnalysis, symbol: SymbolId) -> NavigationTarget {
    analysis
        .query_support
        .as_ref()
        .and_then(|query_support| {
            query_support
                .navigation_targets_by_symbol
                .get(&symbol)
                .copied()
        })
        .unwrap_or_else(|| analysis.hir.navigation_target(symbol))
}

fn build_query_support(
    file_id: FileId,
    normalized_path: PathBuf,
    hir: &FileHir,
) -> PerFileQuerySupport {
    let completion_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            (
                SymbolId(index as u32),
                CompletionSymbol {
                    symbol: SymbolId(index as u32),
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    range: symbol.range,
                    docs: symbol.docs,
                    annotation: symbol.annotation.clone(),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let navigation_targets = hir
        .symbols
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let symbol = SymbolId(index as u32);
            (
                symbol,
                CachedNavigationTarget {
                    symbol: hir.file_backed_symbol_identity(symbol),
                    target: hir.navigation_target(symbol),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let member_completion_sets = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            let symbol = SymbolId(index as u32);
            let members = member_completion_support_for_symbol(hir, symbol);
            (!members.is_empty()).then_some((
                symbol,
                CachedMemberCompletionSet {
                    symbol: hir.file_backed_symbol_identity(symbol),
                    members,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut completion_symbol_entries = completion_symbols.values().cloned().collect::<Vec<_>>();
    completion_symbol_entries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.range.start().cmp(&right.range.start()))
    });

    let mut navigation_target_entries = navigation_targets.values().cloned().collect::<Vec<_>>();
    navigation_target_entries.sort_by(|left, right| {
        left.symbol.name.cmp(&right.symbol.name).then_with(|| {
            left.target
                .full_range
                .start()
                .cmp(&right.target.full_range.start())
        })
    });

    let mut member_completion_entries =
        member_completion_sets.values().cloned().collect::<Vec<_>>();
    member_completion_entries.sort_by(|left, right| left.symbol.name.cmp(&right.symbol.name));

    PerFileQuerySupport {
        file_id,
        normalized_path,
        completion_symbols: Arc::from(completion_symbol_entries),
        navigation_targets: Arc::from(navigation_target_entries),
        member_completion_sets: Arc::from(member_completion_entries),
        completion_symbols_by_symbol: Arc::new(completion_symbols),
        navigation_targets_by_symbol: Arc::new(
            navigation_targets
                .into_iter()
                .map(|(symbol, entry)| (symbol, entry.target))
                .collect(),
        ),
        member_completion_sets_by_symbol: Arc::new(
            member_completion_sets
                .into_iter()
                .map(|(symbol, entry)| (symbol, entry.members))
                .collect(),
        ),
    }
}

fn member_completion_support_for_symbol(
    hir: &FileHir,
    symbol: SymbolId,
) -> Arc<[MemberCompletion]> {
    let mut members = BTreeMap::<String, MemberCompletion>::new();

    for field in hir.documented_fields(symbol) {
        members
            .entry(field.name.clone())
            .or_insert(MemberCompletion {
                name: field.name,
                annotation: Some(field.annotation),
                range: None,
                source: MemberCompletionSource::DocumentedField,
            });
    }

    for flow in hir.value_flows_into(symbol) {
        for member in object_field_member_completions(hir, flow.expr) {
            members.entry(member.name.clone()).or_insert(member);
        }
    }

    Arc::from(members.into_values().collect::<Vec<_>>())
}

fn object_field_member_completions(hir: &FileHir, expr: ExprId) -> Vec<MemberCompletion> {
    hir.object_fields
        .iter()
        .filter(|field| field.owner == expr)
        .map(|field| MemberCompletion {
            name: field.name.clone(),
            annotation: field
                .value
                .and_then(|value| object_field_annotation_from_expr(hir, value)),
            range: Some(field.range),
            source: MemberCompletionSource::ObjectLiteralField,
        })
        .collect()
}

fn object_field_annotation_from_expr(hir: &FileHir, expr: ExprId) -> Option<TypeRef> {
    match hir.expr(expr).kind {
        ExprKind::Literal => None,
        ExprKind::Object => Some(TypeRef::Object(
            hir.object_fields
                .iter()
                .filter(|field| field.owner == expr)
                .map(|field| {
                    (
                        field.name.clone(),
                        field
                            .value
                            .and_then(|value| object_field_annotation_from_expr(hir, value))
                            .unwrap_or(TypeRef::Unknown),
                    )
                })
                .collect(),
        )),
        ExprKind::Array => Some(TypeRef::Array(Box::new(TypeRef::Unknown))),
        ExprKind::Closure => Some(TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Unknown),
        })),
        ExprKind::Name => {
            symbol_for_expr(hir, expr).and_then(|symbol| hir.declared_symbol_type(symbol).cloned())
        }
        _ => None,
    }
}

fn symbol_for_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Name => hir
            .reference_at(hir.expr(expr).range)
            .and_then(|reference| hir.definition_of(reference)),
        _ => None,
    }
}

fn coalesce_file_changes(changes: Vec<FileChange>) -> Vec<FileChange> {
    let mut by_path = HashMap::<PathBuf, FileChange>::new();

    for mut change in changes {
        let normalized_path = normalize_path(&change.path);
        change.path = normalized_path.clone();

        let replace = by_path.get(&normalized_path).is_none_or(|current| {
            change.version > current.version
                || (change.version == current.version && change.text != current.text)
        });
        if replace {
            by_path.insert(normalized_path, change);
        }
    }

    let mut changes = by_path.into_values().collect::<Vec<_>>();
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    changes
}

fn same_export_edge(left: &LocatedModuleExport, right: &LocatedModuleExport) -> bool {
    left.file_id == right.file_id && left.export.export == right.export.export
}

fn default_file_stats(file_id: FileId) -> FilePerformanceStats {
    FilePerformanceStats {
        file_id,
        normalized_path: PathBuf::new(),
        parse_rebuilds: 0,
        lower_rebuilds: 0,
        index_rebuilds: 0,
        query_support_rebuilds: 0,
        query_support_evictions: 0,
        query_support_cached: false,
        dependency_count: 0,
        dependent_count: 0,
    }
}

fn sorted_file_ids(mut file_ids: Vec<FileId>) -> Vec<FileId> {
    file_ids.sort_by_key(|file_id| file_id.0);
    file_ids.dedup_by_key(|file_id| file_id.0);
    file_ids
}

fn resolved_source_roots(project: &ProjectConfig) -> Vec<PathBuf> {
    let root = normalize_path(&project.root);
    let mut roots = if !project.source_roots.is_empty() {
        project
            .source_roots
            .iter()
            .map(|source_root| {
                if source_root.is_absolute() || project.root.as_os_str().is_empty() {
                    normalize_path(source_root)
                } else {
                    normalize_path(&root.join(source_root))
                }
            })
            .collect::<Vec<_>>()
    } else if !project.root.as_os_str().is_empty() {
        vec![root]
    } else {
        Vec::new()
    };

    roots.sort();
    roots.dedup();
    roots
}

fn source_root_index_for_path(path: &Path, source_roots: &[PathBuf]) -> Option<usize> {
    source_roots
        .iter()
        .position(|source_root| path.starts_with(source_root))
}
