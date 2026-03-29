use crate::lowering::ctx::LoweringContext;
use crate::model::{ExportDirective, ImportDirective, ScopeId, SymbolKind};
use rhai_syntax::{AstNode, Item, Stmt};

impl<'a> LoweringContext<'a> {
    pub(crate) fn lower_import_stmt(
        &mut self,
        import_stmt: rhai_syntax::ImportStmt<'_>,
        scope: ScopeId,
    ) {
        let mut module_range = None;
        let mut module_text = None;
        let mut module_reference = None;
        if let Some(module) = import_stmt.module() {
            let reference_start = self.file.references.len();
            module_range = Some(module.syntax().range());
            module_text = Some(self.text_for_range(module.syntax().range()));
            self.lower_expr(module, scope);
            module_reference =
                self.first_name_reference_from(reference_start, module.syntax().range());
        }
        let mut alias_symbol = None;
        if let Some(alias) = import_stmt.alias().and_then(|alias| alias.alias_token()) {
            let docs = self.docs_for_range(import_stmt.syntax().range());
            alias_symbol = Some(self.alloc_symbol(
                alias.text(self.parse.text()).to_owned(),
                SymbolKind::ImportAlias,
                alias.range(),
                scope,
                docs,
            ));
        }
        self.file.imports.push(ImportDirective {
            range: import_stmt.syntax().range(),
            scope,
            module_range,
            module_text,
            module_reference,
            alias: alias_symbol,
        });
    }

    pub(crate) fn lower_export_stmt(
        &mut self,
        export_stmt: rhai_syntax::ExportStmt<'_>,
        scope: ScopeId,
    ) {
        let mut target_range = None;
        let mut target_text = None;
        let mut target_symbol = None;
        let mut target_reference = None;
        if let Some(declaration) = export_stmt.declaration() {
            self.lower_item(Item::Stmt(declaration), scope);
            let binding = match declaration {
                Stmt::Let(let_stmt) => let_stmt.name_token(),
                Stmt::Const(const_stmt) => const_stmt.name_token(),
                _ => None,
            };
            if let Some(binding) = binding {
                target_range = Some(binding.range());
                target_text = Some(binding.text(self.parse.text()).to_owned());
                target_symbol = self.file.symbol_at(binding.range());
            }
        } else if let Some(target) = export_stmt.target() {
            let reference_start = self.file.references.len();
            target_range = Some(target.syntax().range());
            target_text = Some(self.text_for_range(target.syntax().range()));
            self.lower_expr(target, scope);
            target_reference =
                self.first_name_reference_from(reference_start, target.syntax().range());
        }
        let mut alias_symbol = None;
        if let Some(alias) = export_stmt.alias().and_then(|alias| alias.alias_token()) {
            let docs = self.docs_for_range(export_stmt.syntax().range());
            alias_symbol = Some(self.alloc_symbol(
                alias.text(self.parse.text()).to_owned(),
                SymbolKind::ExportAlias,
                alias.range(),
                scope,
                docs,
            ));
        }
        self.file.exports.push(ExportDirective {
            range: export_stmt.syntax().range(),
            scope,
            target_range,
            target_text,
            target_symbol,
            target_reference,
            alias: alias_symbol,
        });
    }
}
