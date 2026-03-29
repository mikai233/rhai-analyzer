use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhai_hir::{FileBackedSymbolIdentity, SymbolKind, WorkspaceSymbol};
use rhai_vfs::FileId;
use rhai_vfs::normalize_path;

use crate::types::{
    CachedFileAnalysis, LinkedModuleImport, LocatedModuleExport, LocatedModuleGraph,
    LocatedSymbolIdentity, LocatedWorkspaceSymbol, SymbolIdentityKey, WorkspaceDependencyEdge,
    WorkspaceDependencyGraph, WorkspaceIndexes,
};

impl WorkspaceIndexes {
    pub(crate) fn rebuild_from_analysis(
        &mut self,
        analysis: &HashMap<FileId, Arc<CachedFileAnalysis>>,
    ) {
        self.symbols_by_file.clear();
        self.module_graphs_by_file.clear();
        self.exports_by_file.clear();
        self.symbol_locations_by_file.clear();
        self.linked_imports_by_file.clear();

        for (&file_id, file_analysis) in analysis {
            self.replace_file(file_id, Some(file_analysis), analysis);
        }
    }

    pub(crate) fn replace_file(
        &mut self,
        file_id: FileId,
        file_analysis: Option<&Arc<CachedFileAnalysis>>,
        analysis: &HashMap<FileId, Arc<CachedFileAnalysis>>,
    ) {
        match file_analysis {
            Some(file_analysis) => {
                self.symbols_by_file.insert(
                    file_id,
                    Arc::<[LocatedWorkspaceSymbol]>::from(
                        file_analysis
                            .workspace_symbols
                            .iter()
                            .cloned()
                            .map(|symbol| LocatedWorkspaceSymbol { file_id, symbol })
                            .collect::<Vec<_>>(),
                    ),
                );
                self.module_graphs_by_file.insert(
                    file_id,
                    Arc::new(LocatedModuleGraph {
                        file_id,
                        graph: Arc::clone(&file_analysis.module_graph),
                    }),
                );
                self.exports_by_file.insert(
                    file_id,
                    Arc::<[LocatedModuleExport]>::from(
                        file_analysis
                            .module_graph
                            .exports
                            .iter()
                            .cloned()
                            .map(|export| LocatedModuleExport { file_id, export })
                            .collect::<Vec<_>>(),
                    ),
                );
                self.symbol_locations_by_file.insert(
                    file_id,
                    Arc::<[LocatedSymbolIdentity]>::from(
                        file_analysis
                            .workspace_symbols
                            .iter()
                            .map(|symbol| LocatedSymbolIdentity {
                                file_id,
                                symbol: file_backed_identity_from_workspace_symbol(symbol),
                            })
                            .collect::<Vec<_>>(),
                    ),
                );
            }
            None => {
                self.symbols_by_file.remove(&file_id);
                self.module_graphs_by_file.remove(&file_id);
                self.exports_by_file.remove(&file_id);
                self.symbol_locations_by_file.remove(&file_id);
                self.linked_imports_by_file.remove(&file_id);
            }
        }

        self.rebuild_aggregates(analysis);
    }

    fn rebuild_aggregates(&mut self, analysis: &HashMap<FileId, Arc<CachedFileAnalysis>>) {
        let mut workspace_symbols = self
            .symbols_by_file
            .values()
            .flat_map(|symbols| symbols.iter().cloned())
            .collect::<Vec<_>>();

        workspace_symbols.sort_by(|left, right| {
            left.symbol
                .name
                .cmp(&right.symbol.name)
                .then_with(|| {
                    symbol_kind_rank(left.symbol.kind).cmp(&symbol_kind_rank(right.symbol.kind))
                })
                .then_with(|| left.file_id.0.cmp(&right.file_id.0))
                .then_with(|| {
                    left.symbol
                        .stable_key
                        .container_path
                        .cmp(&right.symbol.stable_key.container_path)
                })
                .then_with(|| {
                    left.symbol
                        .stable_key
                        .ordinal
                        .cmp(&right.symbol.stable_key.ordinal)
                })
                .then_with(|| {
                    left.symbol
                        .full_range
                        .start()
                        .cmp(&right.symbol.full_range.start())
                })
        });

        let mut workspace_module_graphs = self
            .module_graphs_by_file
            .values()
            .map(|graph| (**graph).clone())
            .collect::<Vec<_>>();
        workspace_module_graphs.sort_by_key(|graph| graph.file_id.0);

        let mut workspace_exports = self
            .exports_by_file
            .values()
            .flat_map(|exports| exports.iter().cloned())
            .collect::<Vec<_>>();
        workspace_exports.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.export.export.cmp(&right.export.export))
                .then_with(|| left.export.exported_name.cmp(&right.export.exported_name))
        });

        let mut symbol_locations = HashMap::<SymbolIdentityKey, Vec<LocatedSymbolIdentity>>::new();
        for locations in self.symbol_locations_by_file.values() {
            for location in locations.iter().cloned() {
                symbol_locations
                    .entry(SymbolIdentityKey::from(&location.symbol))
                    .or_default()
                    .push(location);
            }
        }
        let symbol_locations = symbol_locations
            .into_iter()
            .map(|(key, mut locations)| {
                locations.sort_by(|left, right| {
                    left.file_id
                        .0
                        .cmp(&right.file_id.0)
                        .then_with(|| left.symbol.name.cmp(&right.symbol.name))
                        .then_with(|| {
                            left.symbol
                                .declaration_range
                                .start()
                                .cmp(&right.symbol.declaration_range.start())
                        })
                });
                (key, Arc::<[LocatedSymbolIdentity]>::from(locations))
            })
            .collect();

        let mut exports_by_name = HashMap::<String, Vec<LocatedModuleExport>>::new();
        for export in &workspace_exports {
            if let Some(name) = export.export.exported_name.clone() {
                exports_by_name
                    .entry(name)
                    .or_default()
                    .push(export.clone());
            }
        }
        let exports_by_name = exports_by_name
            .into_iter()
            .map(|(name, mut exports)| {
                exports.sort_by(|left, right| {
                    left.file_id
                        .0
                        .cmp(&right.file_id.0)
                        .then_with(|| left.export.export.cmp(&right.export.export))
                });
                (name, Arc::<[LocatedModuleExport]>::from(exports))
            })
            .collect::<HashMap<_, _>>();

        self.linked_imports_by_file.clear();
        for (&file_id, file_analysis) in analysis {
            let linked_imports = file_analysis
                .hir
                .imports
                .iter()
                .enumerate()
                .filter_map(|(index, import)| {
                    let module_name = static_import_module_path(import.module_text.as_deref()?)?;
                    let provider_file_id = resolve_workspace_module_file(
                        &file_analysis.dependencies.normalized_path,
                        module_name.as_str(),
                        analysis,
                    )?;
                    let exports = self.exports_by_file.get(&provider_file_id)?;
                    Some(LinkedModuleImport {
                        file_id,
                        provider_file_id,
                        import: index,
                        module_name,
                        exports: Arc::clone(exports),
                    })
                })
                .collect::<Vec<_>>();

            if !linked_imports.is_empty() {
                self.linked_imports_by_file
                    .insert(file_id, Arc::<[LinkedModuleImport]>::from(linked_imports));
            }
        }

        self.workspace_symbols = Arc::from(workspace_symbols);
        self.workspace_module_graphs = Arc::from(workspace_module_graphs);
        self.workspace_exports = Arc::from(workspace_exports);
        self.symbol_locations = Arc::new(symbol_locations);
        self.exports_by_name = Arc::new(exports_by_name);
        self.linked_imports = Arc::new(self.linked_imports_by_file.clone());
        self.workspace_dependency_graph = Arc::new(build_dependency_graph(&self.linked_imports));
    }
}

fn static_import_module_path(module_text: &str) -> Option<String> {
    if module_text.len() >= 2 && module_text.starts_with('"') && module_text.ends_with('"') {
        return Some(module_text[1..module_text.len() - 1].to_owned());
    }

    None
}

fn resolve_workspace_module_file(
    importer_path: &Path,
    module_name: &str,
    analysis: &HashMap<FileId, Arc<CachedFileAnalysis>>,
) -> Option<FileId> {
    candidate_module_paths(importer_path, module_name)
        .into_iter()
        .find_map(|candidate| {
            analysis.iter().find_map(|(&file_id, file_analysis)| {
                (file_analysis.dependencies.normalized_path == candidate).then_some(file_id)
            })
        })
}

fn candidate_module_paths(importer_path: &Path, module_name: &str) -> Vec<PathBuf> {
    let relative = module_path_with_extension(module_name);
    let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));

    let mut candidates = vec![normalize_path(&importer_dir.join(&relative))];
    let direct = normalize_path(&relative);
    if !candidates.iter().any(|candidate| candidate == &direct) {
        candidates.push(direct);
    }
    candidates
}

fn module_path_with_extension(module_name: &str) -> PathBuf {
    let mut path = PathBuf::from(module_name);
    if path.extension().is_none() {
        path.set_extension("rhai");
    }
    path
}

fn build_dependency_graph(
    linked_imports: &HashMap<FileId, Arc<[LinkedModuleImport]>>,
) -> WorkspaceDependencyGraph {
    let mut edges = Vec::<WorkspaceDependencyEdge>::new();
    let mut dependencies_by_file = HashMap::<FileId, BTreeSet<FileId>>::new();
    let mut dependents_by_file = HashMap::<FileId, BTreeSet<FileId>>::new();

    for (&importer_file_id, imports) in linked_imports {
        for linked_import in imports.iter() {
            for export in linked_import.exports.iter() {
                edges.push(WorkspaceDependencyEdge {
                    importer_file_id,
                    exporter_file_id: export.file_id,
                    module_name: linked_import.module_name.clone(),
                    import: linked_import.import,
                    export: export.export.export,
                });
                dependencies_by_file
                    .entry(importer_file_id)
                    .or_default()
                    .insert(export.file_id);
                dependents_by_file
                    .entry(export.file_id)
                    .or_default()
                    .insert(importer_file_id);
            }
        }
    }

    edges.sort_by(|left, right| {
        left.importer_file_id
            .0
            .cmp(&right.importer_file_id.0)
            .then_with(|| left.exporter_file_id.0.cmp(&right.exporter_file_id.0))
            .then_with(|| left.module_name.cmp(&right.module_name))
            .then_with(|| left.import.cmp(&right.import))
            .then_with(|| left.export.cmp(&right.export))
    });

    WorkspaceDependencyGraph {
        edges: Arc::from(edges),
        dependencies_by_file: Arc::new(
            dependencies_by_file
                .into_iter()
                .map(|(file_id, files)| {
                    (
                        file_id,
                        Arc::<[FileId]>::from(files.into_iter().collect::<Vec<_>>()),
                    )
                })
                .collect(),
        ),
        dependents_by_file: Arc::new(
            dependents_by_file
                .into_iter()
                .map(|(file_id, files)| {
                    (
                        file_id,
                        Arc::<[FileId]>::from(files.into_iter().collect::<Vec<_>>()),
                    )
                })
                .collect(),
        ),
    }
}

fn symbol_kind_rank(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Variable => 0,
        SymbolKind::Parameter => 1,
        SymbolKind::Constant => 2,
        SymbolKind::Function => 3,
        SymbolKind::ImportAlias => 4,
        SymbolKind::ExportAlias => 5,
    }
}

fn file_backed_identity_from_workspace_symbol(
    symbol: &WorkspaceSymbol,
) -> FileBackedSymbolIdentity {
    FileBackedSymbolIdentity {
        symbol: symbol.symbol,
        stable_key: symbol.stable_key.clone(),
        name: symbol.name.clone(),
        kind: symbol.kind,
        declaration_range: symbol.full_range,
        container_path: symbol.stable_key.container_path.clone(),
        exported: symbol.exported,
    }
}
