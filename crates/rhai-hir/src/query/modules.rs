use crate::model::{
    ExprId, ExprKind, FileHir, ImportDirective, ImportExposureKind, ImportLinkageKind,
    ImportedModulePath, ModuleSpecifier, SymbolId, SymbolKind,
};

impl FileHir {
    pub fn import_linkage_kind(&self, import_index: usize) -> ImportLinkageKind {
        self.import(import_index).linkage
    }

    pub fn import_module_specifier_for_index(
        &self,
        import_index: usize,
    ) -> Option<ModuleSpecifier> {
        self.import_module_specifier(self.import(import_index))
    }

    pub fn qualified_path_parts(&self, expr: ExprId) -> Option<Vec<String>> {
        let path = self.path_expr(expr)?;
        let mut parts = Vec::new();
        if let Some(base) = path.base {
            parts.extend(self.expr_path_parts(base)?);
        }
        parts.extend(
            path.segments
                .iter()
                .map(|reference| self.reference(*reference).name.clone()),
        );
        (!parts.is_empty()).then_some(parts)
    }

    pub fn qualified_path_name(&self, expr: ExprId) -> Option<String> {
        self.qualified_path_parts(expr)
            .map(|parts| parts.join("::"))
    }

    pub fn imported_module_path(&self, expr: ExprId) -> Option<ImportedModulePath> {
        let path = self.path_expr(expr)?;
        let base = path.base?;
        let alias_symbol = self.expr_root_symbol(base)?;
        let alias_data = self.symbol(alias_symbol);
        if alias_data.kind != SymbolKind::ImportAlias {
            return None;
        }

        let import_index = self
            .imports
            .iter()
            .enumerate()
            .find_map(|(index, import)| (import.alias == Some(alias_symbol)).then_some(index))?;
        let import = self.import(import_index);
        if import.exposure != ImportExposureKind::Aliased || !import.is_global {
            return None;
        }

        Some(ImportedModulePath {
            import: import_index,
            alias: alias_symbol,
            parts: self.qualified_path_parts(expr)?,
        })
    }

    pub(crate) fn import_module_specifier(
        &self,
        import: &ImportDirective,
    ) -> Option<ModuleSpecifier> {
        match import.linkage {
            ImportLinkageKind::StaticText => import.module_text.clone().map(ModuleSpecifier::Text),
            ImportLinkageKind::LocalSymbol => import
                .module_reference
                .and_then(|reference| self.definition_of(reference))
                .map(|target| {
                    ModuleSpecifier::LocalSymbol(self.file_backed_symbol_identity(target))
                }),
            ImportLinkageKind::DynamicExpr => None,
        }
    }

    fn expr_path_parts(&self, expr: ExprId) -> Option<Vec<String>> {
        match self.expr(expr).kind {
            ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .map(|reference| vec![self.reference(reference).name.clone()]),
            ExprKind::Path => self.qualified_path_parts(expr),
            ExprKind::Paren => self
                .expr_at_offset(self.expr(expr).range.start() + rhai_syntax::TextSize::from(1))
                .and_then(|inner| self.expr_path_parts(inner)),
            _ => None,
        }
    }

    fn expr_root_symbol(&self, expr: ExprId) -> Option<SymbolId> {
        match self.expr(expr).kind {
            ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference)),
            ExprKind::Path => self
                .path_expr(expr)
                .and_then(|path| path.base)
                .and_then(|base| self.expr_root_symbol(base)),
            ExprKind::Paren => self
                .expr_at_offset(self.expr(expr).range.start() + rhai_syntax::TextSize::from(1))
                .and_then(|inner| self.expr_root_symbol(inner)),
            _ => None,
        }
    }
}
