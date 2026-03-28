use crate::{
    Body, BodyId, BodyKind, CallSite, CallSiteId, ControlFlowEvent, ControlFlowKind,
    ControlFlowMergePoint, DocBlock, DocBlockId, DocTag, ExportDirective, ExprId, ExprKind,
    ExprNode, FileHir, FunctionTypeRef, ImportDirective, LoweredFile, MemberAccess, MergePointKind,
    ObjectFieldInfo, Reference, ReferenceId, ReferenceKind, Scope, ScopeId, ScopeKind, Symbol,
    SymbolId, SymbolKind, SymbolValueFlow, TypeRef, TypeSlot, TypeSlotId, ValueFlowKind,
    collect_doc_block,
};
use rhai_syntax::{
    AstNode, BlockExpr, Expr, Item, Parse, Root, Stmt, StringPart, SwitchArm, SwitchPatternList,
    TextRange, TextSize,
};

pub fn lower_file(parse: &Parse) -> LoweredFile {
    let root = Root::cast(parse.root()).expect("root syntax node should cast");
    let mut ctx = LoweringContext::new(parse);
    let file_scope = ctx.new_scope(ScopeKind::File, parse.root().range(), None);

    for item in root.items() {
        ctx.lower_item(item, file_scope);
    }

    ctx.finish()
}

struct LoweringContext<'a> {
    parse: &'a Parse,
    file: FileHir,
    body_stack: Vec<BodyId>,
    loop_stack: Vec<ScopeId>,
    pending_value_flows: Vec<PendingValueFlow>,
}

struct PendingValueFlow {
    reference: ReferenceId,
    expr: ExprId,
    kind: ValueFlowKind,
    range: TextRange,
}

impl<'a> LoweringContext<'a> {
    fn new(parse: &'a Parse) -> Self {
        Self {
            parse,
            file: FileHir {
                root_range: parse.root().range(),
                scopes: Vec::new(),
                symbols: Vec::new(),
                references: Vec::new(),
                bodies: Vec::new(),
                exprs: Vec::new(),
                type_slots: Vec::new(),
                value_flows: Vec::new(),
                calls: Vec::new(),
                object_fields: Vec::new(),
                member_accesses: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                docs: Vec::new(),
            },
            body_stack: Vec::new(),
            loop_stack: Vec::new(),
            pending_value_flows: Vec::new(),
        }
    }

    fn first_name_reference_from(&self, start: usize, range: TextRange) -> Option<ReferenceId> {
        self.file.references[start..]
            .iter()
            .enumerate()
            .find_map(|(offset, reference)| {
                (reference.kind == ReferenceKind::Name
                    && reference.range.start() >= range.start()
                    && reference.range.end() <= range.end())
                .then_some(ReferenceId((start + offset) as u32))
            })
    }

    fn first_reference_from(&self, start: usize, range: TextRange) -> Option<ReferenceId> {
        self.file.references[start..]
            .iter()
            .enumerate()
            .find_map(|(offset, reference)| {
                (reference.range.start() >= range.start() && reference.range.end() <= range.end())
                    .then_some(ReferenceId((start + offset) as u32))
            })
    }

    fn finish(mut self) -> FileHir {
        self.resolve_references();
        self.resolve_call_mappings();
        self.resolve_value_flows();
        self.annotate_symbol_relationships();
        self.file
    }

    fn new_scope(&mut self, kind: ScopeKind, range: TextRange, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.file.scopes.len() as u32);
        self.file.scopes.push(Scope {
            kind,
            range,
            parent,
            children: Vec::new(),
            symbols: Vec::new(),
            references: Vec::new(),
            bodies: Vec::new(),
        });

        if let Some(parent) = parent {
            self.file.scopes[parent.0 as usize].children.push(id);
        }

        id
    }

    fn new_body(
        &mut self,
        kind: BodyKind,
        range: TextRange,
        scope: ScopeId,
        owner: Option<SymbolId>,
    ) -> BodyId {
        let id = BodyId(self.file.bodies.len() as u32);
        self.file.bodies.push(Body {
            kind,
            range,
            scope,
            owner,
            control_flow: Vec::new(),
            return_values: Vec::new(),
            throw_values: Vec::new(),
            merge_points: Vec::new(),
            may_fall_through: true,
            unreachable_ranges: Vec::new(),
        });
        self.file.scopes[scope.0 as usize].bodies.push(id);
        id
    }

    fn new_call(
        &mut self,
        range: TextRange,
        scope: ScopeId,
        callee_range: Option<TextRange>,
        callee_reference: Option<ReferenceId>,
        arg_ranges: Vec<TextRange>,
    ) -> CallSiteId {
        let id = CallSiteId(self.file.calls.len() as u32);
        self.file.calls.push(CallSite {
            range,
            scope,
            callee_range,
            callee_reference,
            resolved_callee: None,
            arg_ranges,
            parameter_bindings: Vec::new(),
        });
        id
    }

    fn alloc_expr(&mut self, kind: ExprKind, range: TextRange, scope: ScopeId) -> ExprId {
        let result_slot = TypeSlotId(self.file.type_slots.len() as u32);
        self.file.type_slots.push(TypeSlot { range });
        let id = ExprId(self.file.exprs.len() as u32);
        self.file.exprs.push(ExprNode {
            kind,
            range,
            scope,
            result_slot,
        });
        id
    }

    fn push_value_flow(
        &mut self,
        symbol: SymbolId,
        expr: ExprId,
        kind: ValueFlowKind,
        range: TextRange,
    ) {
        self.file.value_flows.push(SymbolValueFlow {
            symbol,
            expr,
            kind,
            range,
        });
    }

    fn with_body<T>(&mut self, body: BodyId, f: impl FnOnce(&mut Self) -> T) -> T {
        self.body_stack.push(body);
        let result = f(self);
        let popped = self.body_stack.pop();
        debug_assert_eq!(popped, Some(body));
        result
    }

    fn with_loop<T>(&mut self, loop_scope: ScopeId, f: impl FnOnce(&mut Self) -> T) -> T {
        self.loop_stack.push(loop_scope);
        let result = f(self);
        let popped = self.loop_stack.pop();
        debug_assert_eq!(popped, Some(loop_scope));
        result
    }

    fn record_control_flow(
        &mut self,
        kind: ControlFlowKind,
        range: TextRange,
        value_range: Option<TextRange>,
    ) {
        let target_loop = match kind {
            ControlFlowKind::Break | ControlFlowKind::Continue => self.loop_stack.last().copied(),
            ControlFlowKind::Return | ControlFlowKind::Throw => None,
        };
        for &body_id in self.body_stack.iter().rev() {
            self.file.bodies[body_id.0 as usize]
                .control_flow
                .push(ControlFlowEvent {
                    kind,
                    range,
                    value_range,
                    target_loop,
                });

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    fn record_body_value(&mut self, kind: ControlFlowKind, expr: ExprId) {
        for &body_id in self.body_stack.iter().rev() {
            let body = &mut self.file.bodies[body_id.0 as usize];
            match kind {
                ControlFlowKind::Return => body.return_values.push(expr),
                ControlFlowKind::Throw => body.throw_values.push(expr),
                ControlFlowKind::Break | ControlFlowKind::Continue => {}
            }

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    fn record_merge_point(&mut self, kind: MergePointKind, range: TextRange) {
        for &body_id in self.body_stack.iter().rev() {
            self.file.bodies[body_id.0 as usize]
                .merge_points
                .push(ControlFlowMergePoint { kind, range });

            if self.is_body_flow_boundary(body_id) {
                break;
            }
        }
    }

    fn is_body_flow_boundary(&self, body: BodyId) -> bool {
        matches!(
            self.file.bodies[body.0 as usize].kind,
            BodyKind::Function | BodyKind::Closure | BodyKind::Interpolation
        )
    }

    fn current_body_mut(&mut self) -> Option<&mut Body> {
        let body = *self.body_stack.last()?;
        Some(&mut self.file.bodies[body.0 as usize])
    }

    fn alloc_doc_block(&mut self, doc: DocBlock) -> DocBlockId {
        let id = DocBlockId(self.file.docs.len() as u32);
        self.file.docs.push(doc);
        id
    }

    fn docs_for_range(&mut self, range: TextRange) -> Option<DocBlockId> {
        let doc = collect_doc_block(self.parse.tokens(), self.parse.text(), range.start())?;
        Some(self.alloc_doc_block(doc))
    }

    fn text_for_range(&self, range: TextRange) -> String {
        let start: u32 = range.start().into();
        let end: u32 = range.end().into();
        self.parse.text()[start as usize..end as usize].to_owned()
    }

    fn doc_block(&self, docs: Option<DocBlockId>) -> Option<&DocBlock> {
        let docs = docs?;
        Some(self.file.doc_block(docs))
    }

    fn annotation_from_docs(&self, docs: Option<DocBlockId>) -> Option<TypeRef> {
        self.doc_block(docs)?.tags.iter().find_map(|tag| match tag {
            DocTag::Type(ty) => Some(ty.clone()),
            _ => None,
        })
    }

    fn param_annotation_from_docs(&self, docs: Option<DocBlockId>, name: &str) -> Option<TypeRef> {
        self.doc_block(docs)?.tags.iter().find_map(|tag| match tag {
            DocTag::Param { name: param, ty } if param == name => Some(ty.clone()),
            _ => None,
        })
    }

    fn function_annotation_from_docs(
        &self,
        docs: Option<DocBlockId>,
        params: &[String],
    ) -> Option<TypeRef> {
        if let Some(annotation) = self.annotation_from_docs(docs) {
            return Some(annotation);
        }

        let docs = self.doc_block(docs)?;
        let has_signature_tags = docs
            .tags
            .iter()
            .any(|tag| matches!(tag, DocTag::Param { .. } | DocTag::Return(_)));

        if !has_signature_tags {
            return None;
        }

        let params = params
            .iter()
            .map(|name| {
                docs.tags
                    .iter()
                    .find_map(|tag| match tag {
                        DocTag::Param { name: param, ty } if param == name => Some(ty.clone()),
                        _ => None,
                    })
                    .unwrap_or(TypeRef::Unknown)
            })
            .collect();
        let ret = docs
            .tags
            .iter()
            .find_map(|tag| match tag {
                DocTag::Return(ty) => Some(ty.clone()),
                _ => None,
            })
            .unwrap_or(TypeRef::Unknown);

        Some(TypeRef::Function(FunctionTypeRef {
            params,
            ret: Box::new(ret),
        }))
    }

    fn alloc_symbol(
        &mut self,
        name: String,
        kind: SymbolKind,
        range: TextRange,
        scope: ScopeId,
        docs: Option<DocBlockId>,
    ) -> SymbolId {
        let annotation = self.annotation_from_docs(docs);
        self.alloc_symbol_with_annotation(name, kind, range, scope, docs, annotation)
    }

    fn alloc_symbol_with_annotation(
        &mut self,
        name: String,
        kind: SymbolKind,
        range: TextRange,
        scope: ScopeId,
        docs: Option<DocBlockId>,
        annotation: Option<TypeRef>,
    ) -> SymbolId {
        let id = SymbolId(self.file.symbols.len() as u32);
        self.file.symbols.push(Symbol {
            name,
            kind,
            range,
            scope,
            docs,
            annotation,
            references: Vec::new(),
            shadowed: None,
            duplicate_of: None,
        });
        self.file.scopes[scope.0 as usize].symbols.push(id);
        id
    }

    fn alloc_reference(
        &mut self,
        name: String,
        kind: ReferenceKind,
        range: TextRange,
        scope: ScopeId,
    ) -> ReferenceId {
        let id = ReferenceId(self.file.references.len() as u32);
        self.file.references.push(Reference {
            name,
            kind,
            range,
            scope,
            target: None,
        });
        self.file.scopes[scope.0 as usize].references.push(id);
        id
    }

    fn resolve_references(&mut self) {
        for symbol in &mut self.file.symbols {
            symbol.references.clear();
        }

        for index in 0..self.file.references.len() {
            let (kind, scope, range, name) = {
                let reference = &self.file.references[index];
                (
                    reference.kind,
                    reference.scope,
                    reference.range,
                    reference.name.clone(),
                )
            };

            let target = match kind {
                ReferenceKind::Name => self.resolve_name_at(scope, &name, range.start()),
                ReferenceKind::This | ReferenceKind::PathSegment | ReferenceKind::Field => None,
            };
            self.file.references[index].target = target;
            if let Some(target) = target {
                self.file.symbols[target.0 as usize]
                    .references
                    .push(ReferenceId(index as u32));
            }
        }
    }

    fn resolve_value_flows(&mut self) {
        let pending_flows = std::mem::take(&mut self.pending_value_flows);
        for pending in pending_flows {
            let Some(symbol) = self.file.references[pending.reference.0 as usize].target else {
                continue;
            };
            self.push_value_flow(symbol, pending.expr, pending.kind, pending.range);
        }
    }

    fn resolve_call_mappings(&mut self) {
        for index in 0..self.file.calls.len() {
            let (callee_reference, arg_count) = {
                let call = &self.file.calls[index];
                (call.callee_reference, call.arg_ranges.len())
            };

            let resolved_callee = callee_reference
                .and_then(|reference| self.file.references[reference.0 as usize].target)
                .filter(|symbol| self.file.symbols[symbol.0 as usize].kind == SymbolKind::Function);

            let parameter_bindings = resolved_callee
                .map(|function| {
                    self.file
                        .function_parameters(function)
                        .into_iter()
                        .map(Some)
                        .chain(std::iter::repeat(None))
                        .take(arg_count)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![None; arg_count]);

            let call = &mut self.file.calls[index];
            call.resolved_callee = resolved_callee;
            call.parameter_bindings = parameter_bindings;
        }
    }

    fn resolve_name_at(
        &self,
        mut scope: ScopeId,
        name: &str,
        reference_start: TextSize,
    ) -> Option<SymbolId> {
        loop {
            if let Some(symbol) = self.resolve_name_in_scope(scope, name, reference_start) {
                return Some(symbol);
            }

            scope = self.file.scopes[scope.0 as usize].parent?;
        }
    }

    fn resolve_name_in_scope(
        &self,
        scope: ScopeId,
        name: &str,
        reference_start: TextSize,
    ) -> Option<SymbolId> {
        self.file.scopes[scope.0 as usize]
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = &self.file.symbols[symbol_id.0 as usize];
                symbol.name == name && self.symbol_is_visible_at(symbol, reference_start)
            })
    }

    fn symbol_is_visible_at(&self, symbol: &Symbol, reference_start: TextSize) -> bool {
        matches!(symbol.kind, SymbolKind::Function) || symbol.range.start() <= reference_start
    }

    fn annotate_symbol_relationships(&mut self) {
        for symbol in &mut self.file.symbols {
            symbol.shadowed = None;
            symbol.duplicate_of = None;
        }

        for scope_index in 0..self.file.scopes.len() {
            let symbols = self.file.scopes[scope_index].symbols.clone();
            let mut seen = std::collections::HashMap::<String, SymbolId>::new();

            for symbol_id in symbols {
                let (name, start) = {
                    let symbol = &self.file.symbols[symbol_id.0 as usize];
                    (symbol.name.clone(), symbol.range.start())
                };

                if let Some(previous) = seen.get(&name).copied() {
                    self.file.symbols[symbol_id.0 as usize].duplicate_of = Some(previous);
                    continue;
                }

                seen.insert(name.clone(), symbol_id);

                let shadowed = self.file.scopes[scope_index]
                    .parent
                    .and_then(|parent| self.resolve_name_at(parent, &name, start));
                self.file.symbols[symbol_id.0 as usize].shadowed = shadowed;
            }
        }
    }

    fn function_param_bindings(
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

    fn closure_param_bindings(
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

    fn lower_function_signature(
        &self,
        docs: Option<DocBlockId>,
        params: &[(String, TextRange)],
    ) -> Option<TypeRef> {
        let names = params
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.function_annotation_from_docs(docs, &names)
    }

    fn lower_param_symbols(
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
                *range,
                scope,
                None,
                annotation,
            );
        }
    }

    fn lower_item(&mut self, item: Item<'_>, scope: ScopeId) {
        match item {
            Item::Fn(function) => self.lower_function(function, scope),
            Item::Stmt(stmt) => self.lower_stmt(stmt, scope),
        }
    }

    fn lower_function(&mut self, function: rhai_syntax::FnItem<'_>, scope: ScopeId) {
        let docs = self.docs_for_range(function.syntax().range());
        let params = self.function_param_bindings(function);
        let annotation = self.lower_function_signature(docs, &params);
        let Some(name_token) = function.name_token() else {
            return;
        };
        let symbol = self.alloc_symbol_with_annotation(
            name_token.text(self.parse.text()).to_owned(),
            SymbolKind::Function,
            name_token.range(),
            scope,
            docs,
            annotation,
        );

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

    fn lower_stmt(&mut self, stmt: Stmt<'_>, scope: ScopeId) {
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
            }
            Stmt::Import(import_stmt) => {
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
            Stmt::Export(export_stmt) => {
                let mut target_range = None;
                let mut target_text = None;
                let mut target_reference = None;
                if let Some(target) = export_stmt.target() {
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
                    target_reference,
                    alias: alias_symbol,
                });
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
            }
            Stmt::Continue(stmt) => {
                self.record_control_flow(ControlFlowKind::Continue, stmt.syntax().range(), None);
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
            }
            Stmt::Expr(stmt) => {
                if let Some(expr) = stmt.expr() {
                    self.lower_expr(expr, scope);
                }
            }
        }
    }

    fn lower_block_items(&mut self, block: BlockExpr<'_>, scope: ScopeId) {
        let mut terminated = false;
        let mut may_fall_through = true;
        for item in block.items() {
            if terminated && let Some(body) = self.current_body_mut() {
                body.unreachable_ranges.push(item.syntax().range());
            }

            let item_fallthrough = self.item_may_fall_through(item);
            self.lower_item(item, scope);

            if !terminated {
                may_fall_through = item_fallthrough;
                terminated = !item_fallthrough;
            }
        }

        if let Some(body) = self.current_body_mut() {
            body.may_fall_through = may_fall_through;
        }
    }

    fn lower_expr(&mut self, expr: Expr<'_>, scope: ScopeId) -> ExprId {
        let expr_id = self.alloc_expr(expr_kind(expr), expr.syntax().range(), scope);

        match expr {
            Expr::Name(name) => {
                if let Some(token) = name.token() {
                    let kind = match token.kind() {
                        rhai_syntax::TokenKind::ThisKw => ReferenceKind::This,
                        _ => ReferenceKind::Name,
                    };
                    self.alloc_reference(
                        token.text(self.parse.text()).to_owned(),
                        kind,
                        token.range(),
                        scope,
                    );
                }
            }
            Expr::Literal(_) | Expr::Error(_) => {}
            Expr::Array(array) => {
                if let Some(items) = array.items() {
                    for item in items.exprs() {
                        self.lower_expr(item, scope);
                    }
                }
            }
            Expr::Object(object) => {
                for field in object.fields() {
                    let value = field.value().map(|value| self.lower_expr(value, scope));
                    if let Some(name) = field.name_token() {
                        self.file.object_fields.push(ObjectFieldInfo {
                            owner: expr_id,
                            name: normalize_object_field_name(name.text(self.parse.text())),
                            range: name.range(),
                            value,
                        });
                    }
                }
            }
            Expr::If(if_expr) => {
                if let Some(condition) = if_expr.condition() {
                    self.lower_expr(condition, scope);
                }
                if let Some(then_branch) = if_expr.then_branch() {
                    self.lower_block_in_child_scope(then_branch, scope);
                }
                if let Some(else_branch) = if_expr.else_branch().and_then(|branch| branch.body()) {
                    self.lower_expr(else_branch, scope);
                }
                self.record_merge_point(MergePointKind::IfElse, if_expr.syntax().range());
            }
            Expr::Switch(switch_expr) => {
                if let Some(scrutinee) = switch_expr.scrutinee() {
                    self.lower_expr(scrutinee, scope);
                }
                for arm in switch_expr.arms() {
                    self.lower_switch_arm(arm, scope);
                }
                self.record_merge_point(MergePointKind::Switch, switch_expr.syntax().range());
            }
            Expr::While(while_expr) => {
                if let Some(condition) = while_expr.condition() {
                    self.lower_expr(condition, scope);
                }
                if let Some(body) = while_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().range(), Some(scope));
                    let body_id =
                        self.new_body(BodyKind::Block, body.syntax().range(), loop_scope, None);
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                self.record_merge_point(MergePointKind::LoopIteration, while_expr.syntax().range());
            }
            Expr::Loop(loop_expr) => {
                if let Some(body) = loop_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().range(), Some(scope));
                    let body_id =
                        self.new_body(BodyKind::Block, body.syntax().range(), loop_scope, None);
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                self.record_merge_point(MergePointKind::LoopIteration, loop_expr.syntax().range());
            }
            Expr::For(for_expr) => {
                if let Some(iterable) = for_expr.iterable() {
                    self.lower_expr(iterable, scope);
                }

                let loop_scope =
                    self.new_scope(ScopeKind::Loop, for_expr.syntax().range(), Some(scope));
                if let Some(bindings) = for_expr.bindings() {
                    for binding in bindings.names() {
                        self.alloc_symbol(
                            binding.text(self.parse.text()).to_owned(),
                            SymbolKind::Variable,
                            binding.range(),
                            loop_scope,
                            None,
                        );
                    }
                }
                if let Some(body) = for_expr.body() {
                    let body_id =
                        self.new_body(BodyKind::Block, body.syntax().range(), loop_scope, None);
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                self.record_merge_point(MergePointKind::LoopIteration, for_expr.syntax().range());
            }
            Expr::Do(do_expr) => {
                if let Some(body) = do_expr.body() {
                    let loop_scope =
                        self.new_scope(ScopeKind::Loop, body.syntax().range(), Some(scope));
                    let body_id =
                        self.new_body(BodyKind::Block, body.syntax().range(), loop_scope, None);
                    self.with_loop(loop_scope, |this| {
                        this.with_body(body_id, |this| this.lower_block_items(body, loop_scope))
                    });
                }
                if let Some(condition) = do_expr.condition().and_then(|condition| condition.expr())
                {
                    self.lower_expr(condition, scope);
                }
                self.record_merge_point(MergePointKind::LoopIteration, do_expr.syntax().range());
            }
            Expr::Path(path) => {
                if let Some(base) = path.base()
                    && !matches!(
                        base,
                        Expr::Name(name)
                            if matches!(
                                name.token().map(|token| token.kind()),
                                Some(rhai_syntax::TokenKind::GlobalKw)
                            )
                    )
                {
                    self.lower_expr(base, scope);
                }
                for segment in path.segments() {
                    self.alloc_reference(
                        segment.text(self.parse.text()).to_owned(),
                        ReferenceKind::PathSegment,
                        segment.range(),
                        scope,
                    );
                }
            }
            Expr::Closure(closure) => {
                let closure_scope =
                    self.new_scope(ScopeKind::Closure, closure.syntax().range(), Some(scope));
                self.new_body(
                    BodyKind::Closure,
                    closure.syntax().range(),
                    closure_scope,
                    None,
                );
                let body_id = BodyId((self.file.bodies.len() - 1) as u32);
                let params = self.closure_param_bindings(closure);
                self.lower_param_symbols(&params, None, closure_scope);
                if let Some(body) = closure.body() {
                    self.with_body(body_id, |this| this.lower_expr(body, closure_scope));
                }
            }
            Expr::InterpolatedString(string) => {
                let interpolation_scope = self.new_scope(
                    ScopeKind::Interpolation,
                    string.syntax().range(),
                    Some(scope),
                );
                self.new_body(
                    BodyKind::Interpolation,
                    string.syntax().range(),
                    interpolation_scope,
                    None,
                );
                let body_id = BodyId((self.file.bodies.len() - 1) as u32);
                for part in string.parts() {
                    if let StringPart::Interpolation(part) = part
                        && let Some(body) = part.body()
                    {
                        self.with_body(body_id, |this| {
                            for item in body.items() {
                                this.lower_item(item, interpolation_scope);
                            }
                        });
                    }
                }
            }
            Expr::Unary(unary) => {
                if let Some(expr) = unary.expr() {
                    self.lower_expr(expr, scope);
                }
            }
            Expr::Binary(binary) => {
                if let Some(lhs) = binary.lhs() {
                    self.lower_expr(lhs, scope);
                }
                if let Some(rhs) = binary.rhs() {
                    self.lower_expr(rhs, scope);
                }
            }
            Expr::Assign(assign) => {
                let assignment_start = self.file.references.len();
                let lhs_reference = if let Some(lhs) = assign.lhs() {
                    let lhs_range = lhs.syntax().range();
                    self.lower_expr(lhs, scope);
                    if matches!(lhs, Expr::Name(_)) {
                        self.first_name_reference_from(assignment_start, lhs_range)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(rhs) = assign.rhs() {
                    let rhs_expr = self.lower_expr(rhs, scope);
                    if let Some(reference) = lhs_reference {
                        self.pending_value_flows.push(PendingValueFlow {
                            reference,
                            expr: rhs_expr,
                            kind: ValueFlowKind::Assignment,
                            range: assign.syntax().range(),
                        });
                    }
                }
            }
            Expr::Paren(paren) => {
                if let Some(expr) = paren.expr() {
                    self.lower_expr(expr, scope);
                }
            }
            Expr::Call(call) => {
                let reference_start = self.file.references.len();
                let callee_range = call.callee().map(|callee| callee.syntax().range());
                if let Some(callee) = call.callee() {
                    self.lower_expr(callee, scope);
                }
                let callee_reference = callee_range
                    .and_then(|range| self.first_reference_from(reference_start, range));
                let mut arg_ranges = Vec::new();
                if let Some(args) = call.args() {
                    for arg in args.args() {
                        arg_ranges.push(arg.syntax().range());
                        self.lower_expr(arg, scope);
                    }
                }
                self.new_call(
                    call.syntax().range(),
                    scope,
                    callee_range,
                    callee_reference,
                    arg_ranges,
                );
            }
            Expr::Index(index) => {
                if let Some(receiver) = index.receiver() {
                    self.lower_expr(receiver, scope);
                }
                if let Some(expr) = index.index() {
                    self.lower_expr(expr, scope);
                }
            }
            Expr::Field(field) => {
                if let Some(receiver) = field.receiver() {
                    let receiver_expr = self.lower_expr(receiver, scope);
                    if let Some(name) = field.name_token() {
                        let field_reference = self.alloc_reference(
                            name.text(self.parse.text()).to_owned(),
                            ReferenceKind::Field,
                            name.range(),
                            scope,
                        );
                        self.file.member_accesses.push(MemberAccess {
                            range: field.syntax().range(),
                            scope,
                            receiver: receiver_expr,
                            field_reference,
                        });
                    }
                }
            }
            Expr::Block(block) => self.lower_block_in_child_scope(block, scope),
        }

        expr_id
    }

    fn lower_block_in_child_scope(&mut self, block: BlockExpr<'_>, parent_scope: ScopeId) {
        let block_scope =
            self.new_scope(ScopeKind::Block, block.syntax().range(), Some(parent_scope));
        let body_id = self.new_body(BodyKind::Block, block.syntax().range(), block_scope, None);
        self.with_body(body_id, |this| this.lower_block_items(block, block_scope));
    }

    fn lower_switch_arm(&mut self, arm: SwitchArm<'_>, scope: ScopeId) {
        let arm_scope = self.new_scope(ScopeKind::SwitchArm, arm.syntax().range(), Some(scope));
        if let Some(patterns) = arm.patterns() {
            self.lower_switch_patterns(patterns, arm_scope);
        }
        if let Some(value) = arm.value() {
            self.lower_expr(value, arm_scope);
        }
    }

    fn lower_switch_patterns(&mut self, patterns: SwitchPatternList<'_>, scope: ScopeId) {
        for expr in patterns.exprs() {
            self.lower_expr(expr, scope);
        }
    }

    fn item_may_fall_through(&self, item: Item<'_>) -> bool {
        match item {
            Item::Fn(_) => true,
            Item::Stmt(stmt) => self.stmt_may_fall_through(stmt),
        }
    }

    fn stmt_may_fall_through(&self, stmt: Stmt<'_>) -> bool {
        match stmt {
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Return(_) | Stmt::Throw(_) => false,
            Stmt::Expr(stmt) => stmt
                .expr()
                .is_none_or(|expr| self.expr_may_fall_through(expr)),
            Stmt::Let(stmt) => stmt
                .initializer()
                .is_none_or(|expr| self.expr_may_fall_through(expr)),
            Stmt::Const(stmt) => stmt
                .value()
                .is_none_or(|expr| self.expr_may_fall_through(expr)),
            Stmt::Try(stmt) => {
                let body_fallthrough = stmt
                    .body()
                    .is_none_or(|body| self.block_may_fall_through(body));
                let catch_fallthrough = stmt
                    .catch_clause()
                    .and_then(|catch| catch.body())
                    .is_none_or(|body| self.block_may_fall_through(body));
                body_fallthrough || catch_fallthrough
            }
            Stmt::Import(_) | Stmt::Export(_) => true,
        }
    }

    fn block_may_fall_through(&self, block: BlockExpr<'_>) -> bool {
        let mut may_fall_through = true;
        for item in block.items() {
            may_fall_through = self.item_may_fall_through(item);
            if !may_fall_through {
                break;
            }
        }
        may_fall_through
    }

    fn expr_may_fall_through(&self, expr: Expr<'_>) -> bool {
        match expr {
            Expr::If(if_expr) => {
                let then_fallthrough = if_expr
                    .then_branch()
                    .is_none_or(|block| self.block_may_fall_through(block));
                let else_fallthrough = if_expr
                    .else_branch()
                    .and_then(|branch| branch.body())
                    .is_none_or(|expr| self.expr_may_fall_through(expr));
                then_fallthrough || else_fallthrough
            }
            Expr::Switch(switch_expr) => {
                let mut saw_wildcard = false;
                let mut all_arms_terminate = true;
                for arm in switch_expr.arms() {
                    if let Some(patterns) = arm.patterns()
                        && patterns.wildcard_token().is_some()
                    {
                        saw_wildcard = true;
                    }

                    let arm_fallthrough = arm
                        .value()
                        .is_none_or(|expr| self.expr_may_fall_through(expr));
                    if arm_fallthrough {
                        all_arms_terminate = false;
                    }
                }

                !(saw_wildcard && all_arms_terminate)
            }
            Expr::Block(block) => self.block_may_fall_through(block),
            Expr::Paren(paren) => paren
                .expr()
                .is_none_or(|expr| self.expr_may_fall_through(expr)),
            Expr::Do(_) | Expr::While(_) | Expr::Loop(_) | Expr::For(_) => true,
            _ => true,
        }
    }
}

fn expr_kind(expr: Expr<'_>) -> ExprKind {
    match expr {
        Expr::Name(_) => ExprKind::Name,
        Expr::Literal(_) => ExprKind::Literal,
        Expr::Array(_) => ExprKind::Array,
        Expr::Object(_) => ExprKind::Object,
        Expr::If(_) => ExprKind::If,
        Expr::Switch(_) => ExprKind::Switch,
        Expr::While(_) => ExprKind::While,
        Expr::Loop(_) => ExprKind::Loop,
        Expr::For(_) => ExprKind::For,
        Expr::Do(_) => ExprKind::Do,
        Expr::Path(_) => ExprKind::Path,
        Expr::Closure(_) => ExprKind::Closure,
        Expr::InterpolatedString(_) => ExprKind::InterpolatedString,
        Expr::Unary(_) => ExprKind::Unary,
        Expr::Binary(_) => ExprKind::Binary,
        Expr::Assign(_) => ExprKind::Assign,
        Expr::Paren(_) => ExprKind::Paren,
        Expr::Call(_) => ExprKind::Call,
        Expr::Index(_) => ExprKind::Index,
        Expr::Field(_) => ExprKind::Field,
        Expr::Block(_) => ExprKind::Block,
        Expr::Error(_) => ExprKind::Error,
    }
}

fn normalize_object_field_name(text: &str) -> String {
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(text)
        .to_owned()
}
