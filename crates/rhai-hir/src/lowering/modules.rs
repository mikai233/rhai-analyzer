use crate::lowering::ctx::LoweringContext;
use crate::model::{
    ExportDirective, ImportDirective, ImportExposureKind, ImportLinkageKind, ScopeId, ScopeKind,
    SymbolKind,
};
use rhai_syntax::{AstNode, Item, Stmt};

impl<'a> LoweringContext<'a> {
    pub(crate) fn lower_import_stmt(
        &mut self,
        import_stmt: rhai_syntax::ImportStmt,
        scope: ScopeId,
    ) {
        let mut module_expr = None;
        let mut module_range = None;
        let mut module_text = None;
        let mut module_reference = None;
        let mut linkage = ImportLinkageKind::DynamicExpr;
        if let Some(module) = import_stmt.module() {
            let reference_start = self.file.references.len();
            let module_range_value = module.syntax().text_range();
            module_range = Some(module_range_value);
            module_text = Some(self.text_for_range(module_range_value));
            if matches!(
                &module,
                rhai_syntax::Expr::Literal(literal)
                    if matches!(
                        literal.token().and_then(|token| token.kind().token_kind()),
                        Some(
                            rhai_syntax::TokenKind::String
                                | rhai_syntax::TokenKind::RawString
                                | rhai_syntax::TokenKind::BacktickString
                        )
                    )
            ) {
                linkage = ImportLinkageKind::StaticText;
            }
            module_expr = Some(self.lower_expr(module, scope));
            module_reference = self.first_name_reference_from(reference_start, module_range_value);
        }
        let mut alias_symbol = None;
        if let Some(alias) = import_stmt.alias().and_then(|alias| alias.alias_token()) {
            let docs = self.docs_for_range(import_stmt.syntax().text_range());
            alias_symbol = Some(self.alloc_symbol(
                alias.text().to_owned(),
                SymbolKind::ImportAlias,
                alias.text_range(),
                scope,
                docs,
            ));
        }
        self.file.imports.push(ImportDirective {
            range: import_stmt.syntax().text_range(),
            scope,
            module_expr,
            module_range,
            module_text,
            module_reference,
            alias: alias_symbol,
            is_global: self.file.scope(scope).kind == ScopeKind::File,
            linkage,
            exposure: if alias_symbol.is_some() {
                ImportExposureKind::Aliased
            } else {
                ImportExposureKind::Bare
            },
        });
    }

    pub(crate) fn lower_export_stmt(
        &mut self,
        export_stmt: rhai_syntax::ExportStmt,
        scope: ScopeId,
    ) {
        let mut target_range = None;
        let mut target_text = None;
        let mut target_symbol = None;
        let mut target_reference = None;
        if let Some(declaration) = export_stmt.declaration() {
            let binding = match &declaration {
                Stmt::Let(let_stmt) => let_stmt.name_token(),
                Stmt::Const(const_stmt) => const_stmt.name_token(),
                _ => None,
            };
            self.lower_item(Item::Stmt(declaration), scope);
            if let Some(binding) = binding {
                target_range = Some(binding.text_range());
                target_text = Some(binding.text().to_owned());
                target_symbol = self.file.symbol_at(binding.text_range());
            }
        } else if let Some(target) = export_stmt.target() {
            let reference_start = self.file.references.len();
            let target_range_value = target.syntax().text_range();
            target_range = Some(target_range_value);
            target_text = Some(self.text_for_range(target_range_value));
            self.lower_expr(target, scope);
            target_reference = self.first_name_reference_from(reference_start, target_range_value);
        }
        let mut alias_symbol = None;
        if let Some(alias) = export_stmt.alias().and_then(|alias| alias.alias_token()) {
            let docs = self.docs_for_range(export_stmt.syntax().text_range());
            alias_symbol = Some(self.alloc_symbol(
                alias.text().to_owned(),
                SymbolKind::ExportAlias,
                alias.text_range(),
                scope,
                docs,
            ));
        }
        self.file.exports.push(ExportDirective {
            range: export_stmt.syntax().text_range(),
            scope,
            target_range,
            target_text,
            target_symbol,
            target_reference,
            alias: alias_symbol,
        });
    }
}
