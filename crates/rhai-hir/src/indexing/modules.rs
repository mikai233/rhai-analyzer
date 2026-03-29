use crate::model::{
    ExportDirective, FileHir, ModuleExportEdge, ModuleGraphIndex, ModuleImportEdge,
    ModuleSpecifier, ScopeKind, SymbolId, SymbolKind,
};

impl FileHir {
    pub fn module_graph_index(&self) -> ModuleGraphIndex {
        let imports = self
            .imports
            .iter()
            .enumerate()
            .map(|(index, import)| ModuleImportEdge {
                import: index,
                module: self.import_module_specifier(import),
                alias: import
                    .alias
                    .map(|symbol| self.file_backed_symbol_identity(symbol)),
            })
            .collect();

        let mut exports = self
            .exports
            .iter()
            .enumerate()
            .filter_map(|(index, export)| {
                let target_symbol = self.explicit_export_target_symbol(export)?;
                let target = self.file_backed_symbol_identity(target_symbol);
                let alias = export
                    .alias
                    .map(|symbol| self.file_backed_symbol_identity(symbol));
                let exported_name = alias
                    .as_ref()
                    .map(|alias| alias.name.clone())
                    .unwrap_or_else(|| target.name.clone());

                Some(ModuleExportEdge {
                    export: index,
                    target: Some(target),
                    exported_name: Some(exported_name),
                    alias,
                })
            })
            .collect::<Vec<_>>();

        let implicit_export_base = self.exports.len();
        exports.extend(
            self.symbols
                .iter()
                .enumerate()
                .filter_map(|(index, _symbol)| {
                    let symbol_id = SymbolId(index as u32);
                    self.implicit_export_name(symbol_id)
                        .map(|exported_name| ModuleExportEdge {
                            export: implicit_export_base + index,
                            target: Some(self.file_backed_symbol_identity(symbol_id)),
                            exported_name: Some(exported_name),
                            alias: None,
                        })
                }),
        );

        ModuleGraphIndex { imports, exports }
    }

    pub(crate) fn is_exported_symbol(&self, symbol: SymbolId) -> bool {
        self.implicit_export_name(symbol).is_some()
            || self.exports.iter().any(|export| {
                export.alias == Some(symbol)
                    || self.explicit_export_target_symbol(export) == Some(symbol)
            })
    }

    pub(crate) fn implicit_export_name(&self, symbol: SymbolId) -> Option<String> {
        let symbol_data = self.symbol(symbol);
        (symbol_data.kind == SymbolKind::Function
            && !symbol_data.is_private
            && self.scope(symbol_data.scope).kind == ScopeKind::File)
            .then(|| symbol_data.name.clone())
    }

    pub(crate) fn explicit_export_target_symbol(
        &self,
        export: &ExportDirective,
    ) -> Option<SymbolId> {
        let symbol = export.target_symbol.or_else(|| {
            export
                .target_reference
                .and_then(|reference| self.definition_of(reference))
        })?;
        let symbol_data = self.symbol(symbol);
        (matches!(
            symbol_data.kind,
            SymbolKind::Variable | SymbolKind::Constant
        ) && self.scope(symbol_data.scope).kind == ScopeKind::File)
            .then_some(symbol)
    }

    pub(crate) fn import_module_specifier(
        &self,
        import: &crate::ImportDirective,
    ) -> Option<ModuleSpecifier> {
        if let Some(reference) = import.module_reference
            && let Some(target) = self.definition_of(reference)
        {
            return Some(ModuleSpecifier::LocalSymbol(
                self.file_backed_symbol_identity(target),
            ));
        }

        import.module_text.clone().map(ModuleSpecifier::Text)
    }
}
