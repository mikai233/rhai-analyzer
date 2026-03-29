use std::collections::HashMap;

use crate::model::{
    DocumentSymbol, FileBackedSymbolIdentity, FileHir, FileSymbolId, FileSymbolIndex,
    FileSymbolIndexEntry, IndexableSymbol, IndexingHandoff, ScopeKind, StableSymbolKey, SymbolId,
    SymbolKind, WorkspaceSymbol,
};

impl FileHir {
    pub fn file_backed_symbol_identity(&self, symbol: SymbolId) -> FileBackedSymbolIdentity {
        let symbol_data = self.symbol(symbol);
        FileBackedSymbolIdentity {
            symbol,
            stable_key: self.stable_symbol_key(symbol),
            name: symbol_data.name.clone(),
            kind: symbol_data.kind,
            declaration_range: symbol_data.range,
            container_path: self.container_path_of(symbol),
            exported: self.is_exported_symbol(symbol),
        }
    }

    pub fn indexable_symbols(&self) -> Vec<IndexableSymbol> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(index, _)| self.is_indexable_symbol(SymbolId(*index as u32)))
            .map(|(index, symbol)| {
                let symbol_id = SymbolId(index as u32);
                IndexableSymbol {
                    symbol: symbol_id,
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    range: symbol.range,
                    container: self.container_symbol_of(symbol_id),
                    exported: self.is_exported_symbol(symbol_id),
                }
            })
            .collect()
    }

    pub fn file_symbol_index(&self) -> FileSymbolIndex {
        let entries = self
            .indexable_symbols()
            .into_iter()
            .enumerate()
            .map(|(index, item)| FileSymbolIndexEntry {
                id: FileSymbolId(index as u32),
                symbol: item.symbol,
                stable_key: self.stable_symbol_key(item.symbol),
                name: item.name,
                kind: item.kind,
                full_range: item.range,
                focus_range: self.symbol(item.symbol).range,
                container_name: item
                    .container
                    .map(|container| self.symbol(container).name.clone()),
                exported: item.exported,
            })
            .collect();

        FileSymbolIndex { entries }
    }

    pub fn stable_symbol_key(&self, symbol: SymbolId) -> StableSymbolKey {
        let symbol_data = self.symbol(symbol);
        StableSymbolKey {
            name: symbol_data.name.clone(),
            kind: symbol_data.kind,
            container_path: self.container_path_of(symbol),
            ordinal: self.stable_symbol_ordinal(symbol),
        }
    }

    pub fn document_symbols(&self) -> Vec<DocumentSymbol> {
        let mut by_parent = HashMap::<Option<SymbolId>, Vec<FileSymbolIndexEntry>>::new();
        for entry in self.file_symbol_index().entries {
            by_parent
                .entry(self.container_symbol_of(entry.symbol))
                .or_default()
                .push(entry);
        }

        fn build(
            by_parent: &mut HashMap<Option<SymbolId>, Vec<FileSymbolIndexEntry>>,
            parent: Option<SymbolId>,
        ) -> Vec<DocumentSymbol> {
            let mut children = by_parent.remove(&parent).unwrap_or_default();
            children.sort_by_key(|entry| entry.full_range.start());
            children
                .into_iter()
                .map(|entry| DocumentSymbol {
                    symbol: entry.symbol,
                    stable_key: entry.stable_key.clone(),
                    name: entry.name,
                    kind: entry.kind,
                    full_range: entry.full_range,
                    focus_range: entry.focus_range,
                    children: build(by_parent, Some(entry.symbol)),
                })
                .collect()
        }

        build(&mut by_parent, None)
    }

    pub fn workspace_symbols(&self) -> Vec<WorkspaceSymbol> {
        self.file_symbol_index()
            .entries
            .into_iter()
            .map(|entry| WorkspaceSymbol {
                id: entry.id,
                stable_key: entry.stable_key,
                symbol: entry.symbol,
                name: entry.name,
                kind: entry.kind,
                full_range: entry.full_range,
                focus_range: entry.focus_range,
                container_name: entry.container_name,
                exported: entry.exported,
            })
            .collect()
    }

    pub fn indexing_handoff(&self) -> IndexingHandoff {
        IndexingHandoff {
            file_symbols: self.file_symbol_index(),
            workspace_symbols: self.workspace_symbols(),
            module_graph: self.module_graph_index(),
        }
    }

    pub(crate) fn stable_symbol_ordinal(&self, symbol: SymbolId) -> u32 {
        let symbol_data = self.symbol(symbol);
        let container_path = self.container_path_of(symbol);

        self.symbols
            .iter()
            .enumerate()
            .take(symbol.0 as usize + 1)
            .filter(|(index, candidate)| {
                candidate.name == symbol_data.name
                    && candidate.kind == symbol_data.kind
                    && self.container_path_of(SymbolId(*index as u32)) == container_path
            })
            .count()
            .saturating_sub(1) as u32
    }

    pub(crate) fn is_indexable_symbol(&self, symbol: SymbolId) -> bool {
        let symbol = self.symbol(symbol);
        matches!(
            symbol.kind,
            SymbolKind::Function
                | SymbolKind::Constant
                | SymbolKind::ImportAlias
                | SymbolKind::ExportAlias
        ) || self.scope(symbol.scope).kind == ScopeKind::File
    }

    pub(crate) fn container_symbol_of(&self, symbol: SymbolId) -> Option<SymbolId> {
        let mut scope = self.symbol(symbol).scope;

        loop {
            let owner = self
                .bodies
                .iter()
                .find(|body| body.scope == scope)
                .and_then(|body| body.owner);
            if owner.is_some() && owner != Some(symbol) {
                return owner;
            }

            scope = self.scope(scope).parent?;
        }
    }

    pub(crate) fn container_path_of(&self, symbol: SymbolId) -> Vec<String> {
        let mut path = Vec::new();
        let mut current = self.container_symbol_of(symbol);
        while let Some(container) = current {
            path.push(self.symbol(container).name.clone());
            current = self.container_symbol_of(container);
        }
        path.reverse();
        path
    }
}
