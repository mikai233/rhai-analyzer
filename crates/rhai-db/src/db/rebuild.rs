use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::change::{ChangeSet, FileChange};
use crate::db::AnalyzerDatabase;
use crate::infer::infer_file_types;
use crate::project::build_project_semantics;
use crate::types::{
    CachedFileAnalysis, ChangeImpact, FileAnalysisDependencies, FilePerformanceStats, HirInputSlot,
    IndexInputSlot, InvalidationReason, ParseInputSlot, RemovedFileImpact,
    WorkspaceDependencyGraph,
};
use rhai_hir::{
    DocumentSymbol, SemanticDiagnostic, SymbolId, TypeRef, WorkspaceSymbol, lower_file,
};
use rhai_project::ProjectConfig;
use rhai_syntax::{SyntaxError, parse_text};
use rhai_vfs::{FileId, normalize_path};

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

pub(crate) fn coalesce_file_changes(changes: Vec<FileChange>) -> Vec<FileChange> {
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

pub(crate) fn default_file_stats(file_id: FileId) -> FilePerformanceStats {
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

pub(crate) fn sorted_file_ids(mut file_ids: Vec<FileId>) -> Vec<FileId> {
    file_ids.sort_by_key(|file_id| file_id.0);
    file_ids.dedup_by_key(|file_id| file_id.0);
    file_ids
}

pub(crate) fn resolved_source_roots(project: &ProjectConfig) -> Vec<PathBuf> {
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

pub(crate) fn source_root_index_for_path(path: &Path, source_roots: &[PathBuf]) -> Option<usize> {
    source_roots
        .iter()
        .position(|source_root| path.starts_with(source_root))
}
