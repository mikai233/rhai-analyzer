use crate::docs::DocBlockId;
use crate::lowering::ctx::LoweringContext;
use crate::model::{
    BodyId, BodyKind, ControlFlowKind, FunctionInfo, ScopeId, ScopeKind, SymbolKind, ValueFlowKind,
};
use crate::ty::TypeRef;
use rhai_syntax::{AstNode, BlockExpr, Item, Stmt, TextRange};

impl<'a> LoweringContext<'a> {
    pub(crate) fn function_param_bindings(
        &self,
        function: rhai_syntax::FnItem<'_>,
    ) -> Vec<(String, TextRange)> {
        function
            .params()
            .into_iter()
            .flat_map(|params| params.params())
            .map(|token| (token.text(self.parse.text()).to_owned(), token.range()))
            .collect()
    }

    pub(crate) fn closure_param_bindings(
        &self,
        closure: rhai_syntax::ClosureExpr<'_>,
    ) -> Vec<(String, TextRange)> {
        closure
            .params()
            .into_iter()
            .flat_map(|params| params.params())
            .map(|token| (token.text(self.parse.text()).to_owned(), token.range()))
            .collect()
    }

    pub(crate) fn function_this_type(&self, function: rhai_syntax::FnItem<'_>) -> Option<TypeRef> {
        let this_type = function.this_type_name(self.parse.text())?;
        crate::parse_type_ref(&this_type).or(Some(TypeRef::Named(this_type)))
    }

    pub(crate) fn lower_param_symbols(
        &mut self,
        params: &[(String, TextRange)],
        docs: Option<DocBlockId>,
        scope: ScopeId,
    ) {
        for (name, range) in params {
            let annotation = self.param_annotation_from_docs(docs, name);
            self.alloc_symbol_with_annotation(
                name.clone(),
                SymbolKind::Parameter,
                false,
                *range,
                scope,
                None,
                annotation,
            );
        }
    }

    pub(crate) fn lower_item(&mut self, item: Item<'_>, scope: ScopeId) -> Option<crate::ExprId> {
        match item {
            Item::Fn(function) => {
                self.lower_function(function, scope);
                None
            }
            Item::Stmt(stmt) => self.lower_stmt(stmt, scope),
        }
    }

    pub(crate) fn lower_function(&mut self, function: rhai_syntax::FnItem<'_>, scope: ScopeId) {
        let docs = self.docs_for_range(function.syntax().range());
        let params = self.function_param_bindings(function);
        let annotation = self.lower_function_signature(docs, &params);
        let this_type = self.function_this_type(function);
        let Some(name_token) = function.name_token() else {
            return;
        };
        let symbol = self.alloc_symbol_with_annotation(
            name_token.text(self.parse.text()).to_owned(),
            SymbolKind::Function,
            function.is_private(),
            name_token.range(),
            scope,
            docs,
            annotation,
        );
        self.file
            .function_infos
            .push(FunctionInfo { symbol, this_type });

        let function_scope =
            self.new_scope(ScopeKind::Function, function.syntax().range(), Some(scope));
        let Some(body) = function.body() else {
            return;
        };
        self.new_body(
            BodyKind::Function,
            body.syntax().range(),
            function_scope,
            Some(symbol),
        );

        self.lower_param_symbols(&params, docs, function_scope);
        let body_id = BodyId((self.file.bodies.len() - 1) as u32);
        self.with_body(body_id, |this| this.lower_block_items(body, function_scope));
    }

    pub(crate) fn lower_stmt(&mut self, stmt: Stmt<'_>, scope: ScopeId) -> Option<crate::ExprId> {
        match stmt {
            Stmt::Let(let_stmt) => {
                let initializer = let_stmt
                    .initializer()
                    .map(|initializer| self.lower_expr(initializer, scope));

                let symbol = let_stmt.name_token().map(|name| {
                    let docs = self.docs_for_range(let_stmt.syntax().range());
                    self.alloc_symbol(
                        name.text(self.parse.text()).to_owned(),
                        SymbolKind::Variable,
                        name.range(),
                        scope,
                        docs,
                    )
                });

                if let (Some(symbol), Some(initializer)) = (symbol, initializer) {
                    self.push_value_flow(
                        symbol,
                        initializer,
                        ValueFlowKind::Initializer,
                        let_stmt.syntax().range(),
                    );
                }
                None
            }
            Stmt::Const(const_stmt) => {
                let value = const_stmt
                    .value()
                    .map(|value| self.lower_expr(value, scope));

                let symbol = const_stmt.name_token().map(|name| {
                    let docs = self.docs_for_range(const_stmt.syntax().range());
                    self.alloc_symbol(
                        name.text(self.parse.text()).to_owned(),
                        SymbolKind::Constant,
                        name.range(),
                        scope,
                        docs,
                    )
                });

                if let (Some(symbol), Some(value)) = (symbol, value) {
                    self.push_value_flow(
                        symbol,
                        value,
                        ValueFlowKind::Initializer,
                        const_stmt.syntax().range(),
                    );
                }
                None
            }
            Stmt::Import(import_stmt) => {
                self.lower_import_stmt(import_stmt, scope);
                None
            }
            Stmt::Export(export_stmt) => {
                self.lower_export_stmt(export_stmt, scope);
                None
            }
            Stmt::Break(stmt) => {
                let value_range = stmt.value().map(|value| value.syntax().range());
                self.record_control_flow(
                    ControlFlowKind::Break,
                    stmt.syntax().range(),
                    value_range,
                );
                if let Some(value) = stmt.value() {
                    self.lower_expr(value, scope);
                }
                None
            }
            Stmt::Continue(stmt) => {
                self.record_control_flow(ControlFlowKind::Continue, stmt.syntax().range(), None);
                None
            }
            Stmt::Return(stmt) => {
                let value_range = stmt.value().map(|value| value.syntax().range());
                self.record_control_flow(
                    ControlFlowKind::Return,
                    stmt.syntax().range(),
                    value_range,
                );
                if let Some(value) = stmt.value() {
                    let value_expr = self.lower_expr(value, scope);
                    self.record_body_value(ControlFlowKind::Return, value_expr);
                }
                None
            }
            Stmt::Throw(stmt) => {
                let value_range = stmt.value().map(|value| value.syntax().range());
                self.record_control_flow(
                    ControlFlowKind::Throw,
                    stmt.syntax().range(),
                    value_range,
                );
                if let Some(value) = stmt.value() {
                    let value_expr = self.lower_expr(value, scope);
                    self.record_body_value(ControlFlowKind::Throw, value_expr);
                }
                None
            }
            Stmt::Try(stmt) => {
                if let Some(body) = stmt.body() {
                    let try_scope =
                        self.new_scope(ScopeKind::Block, body.syntax().range(), Some(scope));
                    let body_id =
                        self.new_body(BodyKind::Block, body.syntax().range(), try_scope, None);
                    self.with_body(body_id, |this| this.lower_block_items(body, try_scope));
                }

                if let Some(catch_clause) = stmt.catch_clause() {
                    let catch_scope = self.new_scope(
                        ScopeKind::Catch,
                        catch_clause.syntax().range(),
                        Some(scope),
                    );
                    if let Some(binding) = catch_clause.binding_token() {
                        self.alloc_symbol(
                            binding.text(self.parse.text()).to_owned(),
                            SymbolKind::Variable,
                            binding.range(),
                            catch_scope,
                            None,
                        );
                    }
                    if let Some(body) = catch_clause.body() {
                        let body_id = self.new_body(
                            BodyKind::Block,
                            body.syntax().range(),
                            catch_scope,
                            None,
                        );
                        self.with_body(body_id, |this| this.lower_block_items(body, catch_scope));
                    }
                }
                None
            }
            Stmt::Expr(stmt) => stmt.expr().map(|expr| self.lower_expr(expr, scope)),
        }
    }

    pub(crate) fn lower_block_items(&mut self, block: BlockExpr<'_>, scope: ScopeId) {
        let mut terminated = false;
        let mut may_fall_through = true;
        let mut tail_value = None;
        for item in block.items() {
            if terminated && let Some(body) = self.current_body_mut() {
                body.unreachable_ranges.push(item.syntax().range());
            }

            let item_fallthrough = self.item_may_fall_through(item);
            let item_expr = self.lower_item(item, scope);

            if !terminated {
                may_fall_through = item_fallthrough;
                terminated = !item_fallthrough;
                tail_value = match item {
                    Item::Stmt(Stmt::Expr(expr_stmt))
                        if !expr_stmt.has_semicolon() && item_fallthrough =>
                    {
                        item_expr
                    }
                    _ => None,
                };
            }
        }

        if let Some(body) = self.current_body_mut() {
            body.may_fall_through = may_fall_through;
            body.tail_value = tail_value;
        }
    }
}
