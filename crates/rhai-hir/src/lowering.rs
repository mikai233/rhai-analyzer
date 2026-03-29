use crate::{
    ArrayExprInfo, AssignExprInfo, AssignmentOperator, BinaryExprInfo, BinaryOperator,
    BlockExprInfo, Body, BodyId, BodyKind, CallSite, CallSiteId, ClosureExprInfo, ControlFlowEvent,
    ControlFlowKind, ControlFlowMergePoint, DocBlock, DocBlockId, DocTag, ExportDirective, ExprId,
    ExprKind, ExprNode, FileHir, ForExprInfo, FunctionInfo, FunctionTypeRef, IfExprInfo,
    ImportDirective, IndexExprInfo, LiteralInfo, LiteralKind, LoweredFile, MemberAccess,
    MergePointKind, MutationPathSegment, ObjectFieldInfo, Reference, ReferenceId, ReferenceKind,
    Scope, ScopeId, ScopeKind, SwitchExprInfo, Symbol, SymbolId, SymbolKind, SymbolMutation,
    SymbolMutationKind, SymbolValueFlow, TypeRef, TypeSlot, TypeSlotId, UnaryExprInfo,
    UnaryOperator, ValueFlowKind, collect_doc_block,
};
use rhai_syntax::{
    AstNode, BlockExpr, Expr, Item, Parse, Root, Stmt, StringPart, SwitchArm, SwitchPatternList,
    TextRange, TextSize, TokenKind,
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
    pending_mutations: Vec<PendingMutation>,
}

struct PendingValueFlow {
    reference: ReferenceId,
    expr: ExprId,
    kind: ValueFlowKind,
    range: TextRange,
}

struct PendingMutation {
    receiver_reference: ReferenceId,
    value: ExprId,
    kind: PendingMutationKind,
    range: TextRange,
}

enum PendingMutationKind {
    Path { segments: Vec<MutationPathSegment> },
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
                literals: Vec::new(),
                array_exprs: Vec::new(),
                block_exprs: Vec::new(),
                if_exprs: Vec::new(),
                switch_exprs: Vec::new(),
                closure_exprs: Vec::new(),
                for_exprs: Vec::new(),
                function_infos: Vec::new(),
                unary_exprs: Vec::new(),
                binary_exprs: Vec::new(),
                assign_exprs: Vec::new(),
                index_exprs: Vec::new(),
                type_slots: Vec::new(),
                value_flows: Vec::new(),
                symbol_mutations: Vec::new(),
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
            pending_mutations: Vec::new(),
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

    fn simple_receiver_reference_from(&self, start: usize, expr: Expr<'_>) -> Option<ReferenceId> {
        match expr {
            Expr::Name(_) => self.first_name_reference_from(start, expr.syntax().range()),
            Expr::Paren(paren) => paren
                .expr()
                .and_then(|inner| self.simple_receiver_reference_from(start, inner)),
            _ => None,
        }
    }

    fn expr_id_for_range(&self, range: TextRange) -> Option<ExprId> {
        self.file
            .exprs
            .iter()
            .enumerate()
            .find_map(|(index, expr)| (expr.range == range).then_some(ExprId(index as u32)))
    }

    fn mutation_target_from_expr(
        &self,
        start: usize,
        expr: Expr<'_>,
    ) -> Option<(ReferenceId, Vec<MutationPathSegment>)> {
        match expr {
            Expr::Field(field) => {
                let name = field.name_token()?.text(self.parse.text()).to_owned();
                let receiver = field.receiver()?;

                if let Some((reference, mut segments)) =
                    self.mutation_target_from_expr(start, receiver)
                {
                    segments.push(MutationPathSegment::Field { name });
                    return Some((reference, segments));
                }

                let reference = self.simple_receiver_reference_from(start, receiver)?;
                Some((reference, vec![MutationPathSegment::Field { name }]))
            }
            Expr::Index(index) => {
                let receiver = index.receiver()?;
                let owner = self.expr_id_for_range(index.syntax().range())?;
                let index_expr = self
                    .file
                    .index_exprs
                    .iter()
                    .find(|entry| entry.owner == owner)?
                    .index?;

                if let Some((reference, mut segments)) =
                    self.mutation_target_from_expr(start, receiver)
                {
                    segments.push(MutationPathSegment::Index { index: index_expr });
                    return Some((reference, segments));
                }

                let reference = self.simple_receiver_reference_from(start, receiver)?;
                Some((
                    reference,
                    vec![MutationPathSegment::Index { index: index_expr }],
                ))
            }
            Expr::Paren(paren) => paren
                .expr()
                .and_then(|inner| self.mutation_target_from_expr(start, inner)),
            _ => None,
        }
    }

    fn finish(mut self) -> FileHir {
        self.resolve_references();
        self.resolve_call_mappings();
        self.resolve_value_flows();
        self.resolve_mutations();
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
            tail_value: None,
            merge_points: Vec::new(),
            may_fall_through: true,
            unreachable_ranges: Vec::new(),
        });
        self.file.scopes[scope.0 as usize].bodies.push(id);
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn new_call(
        &mut self,
        range: TextRange,
        scope: ScopeId,
        caller_scope: bool,
        callee_range: Option<TextRange>,
        callee_reference: Option<ReferenceId>,
        arg_ranges: Vec<TextRange>,
        arg_exprs: Vec<ExprId>,
    ) -> CallSiteId {
        let id = CallSiteId(self.file.calls.len() as u32);
        self.file.calls.push(CallSite {
            range,
            scope,
            caller_scope,
            callee_range,
            callee_reference,
            resolved_callee: None,
            arg_ranges,
            arg_exprs,
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
        self.alloc_symbol_with_annotation(name, kind, false, range, scope, docs, annotation)
    }

    #[allow(clippy::too_many_arguments)]
    fn alloc_symbol_with_annotation(
        &mut self,
        name: String,
        kind: SymbolKind,
        is_private: bool,
        range: TextRange,
        scope: ScopeId,
        docs: Option<DocBlockId>,
        annotation: Option<TypeRef>,
    ) -> SymbolId {
        let id = SymbolId(self.file.symbols.len() as u32);
        self.file.symbols.push(Symbol {
            name,
            kind,
            is_private,
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

    fn resolve_mutations(&mut self) {
        let pending_mutations = std::mem::take(&mut self.pending_mutations);
        for pending in pending_mutations {
            let Some(symbol) = self.file.references[pending.receiver_reference.0 as usize].target
            else {
                continue;
            };

            let kind = match pending.kind {
                PendingMutationKind::Path { segments } => SymbolMutationKind::Path { segments },
            };

            self.file.symbol_mutations.push(SymbolMutation {
                symbol,
                value: pending.value,
                kind,
                range: pending.range,
            });
        }
    }

    fn resolve_call_mappings(&mut self) {
        for index in 0..self.file.calls.len() {
            let (caller_scope, callee_reference, arg_count, first_arg_range, call_start) = {
                let call = &self.file.calls[index];
                (
                    call.caller_scope,
                    call.callee_reference,
                    call.arg_ranges.len(),
                    call.arg_ranges.first().copied(),
                    call.range.start(),
                )
            };

            let callee_name = callee_reference
                .map(|reference| self.file.references[reference.0 as usize].name.as_str());
            let caller_scope_arg_offset = usize::from(caller_scope && callee_name == Some("call"));
            let resolved_callee = if caller_scope_arg_offset == 1 {
                first_arg_range
                    .and_then(|range| self.resolve_caller_scope_target(range, call_start))
            } else {
                callee_reference
                    .and_then(|reference| self.file.references[reference.0 as usize].target)
            };

            let parameter_bindings = resolved_callee
                .filter(|symbol| self.file.symbols[symbol.0 as usize].kind == SymbolKind::Function)
                .map(|function| {
                    let mut bindings = vec![None; caller_scope_arg_offset.min(arg_count)];
                    bindings.extend(
                        self.file
                            .function_parameters(function)
                            .into_iter()
                            .map(Some)
                            .chain(std::iter::repeat(None))
                            .take(arg_count.saturating_sub(caller_scope_arg_offset)),
                    );
                    bindings
                })
                .unwrap_or_else(|| vec![None; arg_count]);

            let call = &mut self.file.calls[index];
            call.resolved_callee = resolved_callee;
            call.parameter_bindings = parameter_bindings;
        }
    }

    fn resolve_caller_scope_target(
        &self,
        first_arg_range: TextRange,
        call_start: TextSize,
    ) -> Option<SymbolId> {
        if let Some(reference) = self.first_reference_in_range(first_arg_range) {
            let target = self.file.reference(reference).target?;
            if self.file.symbols[target.0 as usize].kind == SymbolKind::Function {
                return Some(target);
            }
        }

        let arg_expr = self.expr_id_for_range(first_arg_range)?;
        if self.file.expr(arg_expr).kind != ExprKind::Call {
            return None;
        }
        let fn_call = self
            .file
            .calls
            .iter()
            .find(|call| call.range == first_arg_range)?;
        let fn_name = fn_call
            .callee_reference
            .map(|reference| self.file.reference(reference).name.as_str());
        if fn_name != Some("Fn") {
            return None;
        }
        let name_expr = fn_call.arg_exprs.first().copied()?;
        let literal = self.file.literal(name_expr)?;
        let name = literal.text.as_deref()?;
        self.resolve_name_at(self.file_scope_id()?, name, call_start)
            .filter(|symbol| self.file.symbols[symbol.0 as usize].kind == SymbolKind::Function)
    }

    fn first_reference_in_range(&self, range: TextRange) -> Option<ReferenceId> {
        self.file
            .references
            .iter()
            .enumerate()
            .find_map(|(index, reference)| {
                (reference.range.start() >= range.start() && reference.range.end() <= range.end())
                    .then_some(ReferenceId(index as u32))
            })
    }

    fn file_scope_id(&self) -> Option<ScopeId> {
        self.file
            .scopes
            .iter()
            .enumerate()
            .find_map(|(index, scope)| {
                (scope.kind == ScopeKind::File).then_some(ScopeId(index as u32))
            })
    }

    fn resolve_name_at(
        &self,
        mut scope: ScopeId,
        name: &str,
        reference_start: TextSize,
    ) -> Option<SymbolId> {
        let mut crossed_function_boundary = false;
        loop {
            if let Some(symbol) =
                self.resolve_name_in_scope(scope, name, reference_start, crossed_function_boundary)
            {
                return Some(symbol);
            }

            crossed_function_boundary |=
                self.file.scopes[scope.0 as usize].kind == ScopeKind::Function;
            scope = self.file.scopes[scope.0 as usize].parent?;
        }
    }

    fn resolve_name_in_scope(
        &self,
        scope: ScopeId,
        name: &str,
        reference_start: TextSize,
        crossed_function_boundary: bool,
    ) -> Option<SymbolId> {
        self.file.scopes[scope.0 as usize]
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = &self.file.symbols[symbol_id.0 as usize];
                symbol.name == name
                    && self.symbol_is_visible_at(symbol, reference_start, crossed_function_boundary)
            })
    }

    fn symbol_is_visible_at(
        &self,
        symbol: &Symbol,
        reference_start: TextSize,
        crossed_function_boundary: bool,
    ) -> bool {
        if crossed_function_boundary {
            let symbol_scope = &self.file.scopes[symbol.scope.0 as usize];
            return symbol.kind == SymbolKind::Function
                || (symbol.kind == SymbolKind::ImportAlias
                    && symbol_scope.kind == ScopeKind::File
                    && symbol.range.start() <= reference_start);
        }

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
                    (
                        self.symbol_relationship_key(symbol_id),
                        symbol.range.start(),
                    )
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

    fn symbol_relationship_key(&self, symbol: SymbolId) -> String {
        let symbol_data = &self.file.symbols[symbol.0 as usize];
        if symbol_data.kind != SymbolKind::Function {
            return symbol_data.name.clone();
        }

        match self
            .file
            .function_infos
            .iter()
            .find(|info| info.symbol == symbol)
            .and_then(|info| info.this_type.as_ref())
        {
            Some(this_type) => {
                format!("{}#{}", symbol_data.name, self.function_type_key(this_type))
            }
            None => symbol_data.name.clone(),
        }
    }

    fn function_type_key(&self, this_type: &TypeRef) -> String {
        match this_type {
            TypeRef::Unknown => "unknown".to_owned(),
            TypeRef::Any => "any".to_owned(),
            TypeRef::Never => "never".to_owned(),
            TypeRef::Dynamic => "Dynamic".to_owned(),
            TypeRef::Bool => "bool".to_owned(),
            TypeRef::Int => "int".to_owned(),
            TypeRef::Float => "float".to_owned(),
            TypeRef::Decimal => "decimal".to_owned(),
            TypeRef::String => "string".to_owned(),
            TypeRef::Char => "char".to_owned(),
            TypeRef::Blob => "blob".to_owned(),
            TypeRef::Timestamp => "timestamp".to_owned(),
            TypeRef::FnPtr => "Fn".to_owned(),
            TypeRef::Unit => "()".to_owned(),
            TypeRef::Range => "range".to_owned(),
            TypeRef::RangeInclusive => "range=".to_owned(),
            TypeRef::Named(name) => name.clone(),
            TypeRef::Applied { name, .. } => name.clone(),
            TypeRef::Object(_) => "object".to_owned(),
            TypeRef::Array(_) => "array".to_owned(),
            TypeRef::Map(_, _) => "map".to_owned(),
            TypeRef::Nullable(inner) => format!("{}?", self.function_type_key(inner)),
            TypeRef::Union(items) => items
                .iter()
                .map(|item| self.function_type_key(item))
                .collect::<Vec<_>>()
                .join("|"),
            TypeRef::Function(_) => "fun".to_owned(),
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

    fn function_this_type(&self, function: rhai_syntax::FnItem<'_>) -> Option<TypeRef> {
        let this_type = function.this_type_name(self.parse.text())?;
        crate::parse_type_ref(&this_type).or(Some(TypeRef::Named(this_type)))
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
                false,
                *range,
                scope,
                None,
                annotation,
            );
        }
    }

    fn lower_item(&mut self, item: Item<'_>, scope: ScopeId) -> Option<ExprId> {
        match item {
            Item::Fn(function) => {
                self.lower_function(function, scope);
                None
            }
            Item::Stmt(stmt) => self.lower_stmt(stmt, scope),
        }
    }

    fn lower_function(&mut self, function: rhai_syntax::FnItem<'_>, scope: ScopeId) {
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

    fn lower_stmt(&mut self, stmt: Stmt<'_>, scope: ScopeId) -> Option<ExprId> {
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
                None
            }
            Stmt::Export(export_stmt) => {
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

    fn lower_block_items(&mut self, block: BlockExpr<'_>, scope: ScopeId) {
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
            Expr::Literal(literal) => {
                if let Some(token) = literal.token()
                    && let Some(kind) = literal_kind(token.kind())
                {
                    self.file.literals.push(LiteralInfo {
                        owner: expr_id,
                        kind,
                        range: token.range(),
                        text: Some(token.text(self.parse.text()).to_owned()),
                    });
                }
            }
            Expr::Error(_) => {}
            Expr::Array(array) => {
                let mut item_exprs = Vec::new();
                if let Some(items) = array.items() {
                    for item in items.exprs() {
                        item_exprs.push(self.lower_expr(item, scope));
                    }
                }
                self.file.array_exprs.push(ArrayExprInfo {
                    owner: expr_id,
                    items: item_exprs,
                });
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
                let condition = if_expr
                    .condition()
                    .map(|condition| self.lower_expr(condition, scope));
                let then_branch = if_expr
                    .then_branch()
                    .map(|then_branch| self.lower_block_expr(then_branch, scope));
                let else_branch = if_expr
                    .else_branch()
                    .and_then(|branch| branch.body())
                    .map(|else_branch| self.lower_expr(else_branch, scope));
                self.file.if_exprs.push(IfExprInfo {
                    owner: expr_id,
                    condition,
                    then_branch,
                    else_branch,
                });
                self.record_merge_point(MergePointKind::IfElse, if_expr.syntax().range());
            }
            Expr::Switch(switch_expr) => {
                let scrutinee = switch_expr
                    .scrutinee()
                    .map(|scrutinee| self.lower_expr(scrutinee, scope));
                let mut arms = Vec::new();
                for arm in switch_expr.arms() {
                    arms.push(self.lower_switch_arm(arm, scope));
                }
                self.file.switch_exprs.push(SwitchExprInfo {
                    owner: expr_id,
                    scrutinee,
                    arms,
                });
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
                let iterable_expr = for_expr
                    .iterable()
                    .map(|iterable| self.lower_expr(iterable, scope));

                let loop_scope =
                    self.new_scope(ScopeKind::Loop, for_expr.syntax().range(), Some(scope));
                let mut binding_symbols = Vec::new();
                if let Some(bindings) = for_expr.bindings() {
                    for binding in bindings.names() {
                        binding_symbols.push(self.alloc_symbol(
                            binding.text(self.parse.text()).to_owned(),
                            SymbolKind::Variable,
                            binding.range(),
                            loop_scope,
                            None,
                        ));
                    }
                }
                let mut body_id = None;
                if let Some(body) = for_expr.body() {
                    let lowered_body =
                        self.new_body(BodyKind::Block, body.syntax().range(), loop_scope, None);
                    self.with_loop(loop_scope, |this| {
                        this.with_body(lowered_body, |this| {
                            this.lower_block_items(body, loop_scope)
                        })
                    });
                    body_id = Some(lowered_body);
                }
                self.file.for_exprs.push(ForExprInfo {
                    owner: expr_id,
                    iterable: iterable_expr,
                    bindings: binding_symbols,
                    body: body_id,
                });
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
                let body_id = self.new_body(
                    BodyKind::Closure,
                    closure.syntax().range(),
                    closure_scope,
                    None,
                );
                let params = self.closure_param_bindings(closure);
                self.lower_param_symbols(&params, None, closure_scope);
                if let Some(body) = closure.body() {
                    let body_expr =
                        self.with_body(body_id, |this| this.lower_expr(body, closure_scope));
                    self.file.bodies[body_id.0 as usize].tail_value = Some(body_expr);
                }
                self.file.closure_exprs.push(ClosureExprInfo {
                    owner: expr_id,
                    body: body_id,
                });
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
                let operand = unary.expr().map(|expr| self.lower_expr(expr, scope));
                let operator_token = unary.operator_token();
                if let Some(token) = operator_token
                    && let Some(operator) = unary_operator(token.kind())
                {
                    self.file.unary_exprs.push(UnaryExprInfo {
                        owner: expr_id,
                        operator,
                        operand,
                        operator_range: Some(token.range()),
                    });
                }
            }
            Expr::Binary(binary) => {
                let lhs = binary.lhs().map(|lhs| self.lower_expr(lhs, scope));
                let rhs = binary.rhs().map(|rhs| self.lower_expr(rhs, scope));
                let operator_token = binary.operator_token();
                if let Some(token) = operator_token
                    && let Some(operator) = binary_operator(token.kind())
                {
                    self.file.binary_exprs.push(BinaryExprInfo {
                        owner: expr_id,
                        operator,
                        lhs,
                        rhs,
                        operator_range: Some(token.range()),
                    });
                }
            }
            Expr::Assign(assign) => {
                let assignment_start = self.file.references.len();
                let assignment_operator = assign
                    .operator_token()
                    .and_then(|token| assignment_operator(token.kind()));
                let lhs_syntax = assign.lhs();
                let lhs_expr = lhs_syntax.map(|lhs| self.lower_expr(lhs, scope));
                let rhs_expr = assign.rhs().map(|rhs| self.lower_expr(rhs, scope));

                if let Some(operator) = assignment_operator {
                    self.file.assign_exprs.push(AssignExprInfo {
                        owner: expr_id,
                        operator,
                        lhs: lhs_expr,
                        rhs: rhs_expr,
                        operator_range: assign.operator_token().map(|token| token.range()),
                    });
                }

                let stored_value = match assignment_operator {
                    Some(AssignmentOperator::Assign) => rhs_expr,
                    Some(_) => Some(expr_id),
                    None => rhs_expr,
                };

                if let Some(lhs) = lhs_syntax
                    && let Some(value_expr) = stored_value
                {
                    if let Some(reference) = lhs_expr
                        .filter(|lhs_expr| self.file.expr(*lhs_expr).kind == ExprKind::Name)
                        .and_then(|lhs_expr| {
                            self.first_name_reference_from(
                                assignment_start,
                                self.file.expr(lhs_expr).range,
                            )
                        })
                    {
                        self.pending_value_flows.push(PendingValueFlow {
                            reference,
                            expr: value_expr,
                            kind: ValueFlowKind::Assignment,
                            range: assign.syntax().range(),
                        });
                    }

                    if let Some((receiver_reference, segments)) =
                        self.mutation_target_from_expr(assignment_start, lhs)
                    {
                        self.pending_mutations.push(PendingMutation {
                            receiver_reference,
                            value: value_expr,
                            kind: PendingMutationKind::Path { segments },
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
                let mut arg_exprs = Vec::new();
                if let Some(args) = call.args() {
                    for arg in args.args() {
                        arg_ranges.push(arg.syntax().range());
                        arg_exprs.push(self.lower_expr(arg, scope));
                    }
                }
                self.new_call(
                    call.syntax().range(),
                    scope,
                    call.uses_caller_scope(),
                    callee_range,
                    callee_reference,
                    arg_ranges,
                    arg_exprs,
                );
            }
            Expr::Index(index) => {
                let receiver = index
                    .receiver()
                    .map(|receiver| self.lower_expr(receiver, scope));
                let index = index.index().map(|expr| self.lower_expr(expr, scope));
                self.file.index_exprs.push(IndexExprInfo {
                    owner: expr_id,
                    receiver,
                    index,
                });
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
                            owner: expr_id,
                            range: field.syntax().range(),
                            scope,
                            receiver: receiver_expr,
                            field_reference,
                        });
                    }
                }
            }
            Expr::Block(block) => {
                let body = self.lower_block_expr_with_owner(block, scope);
                self.file.block_exprs.push(BlockExprInfo {
                    owner: expr_id,
                    body,
                });
            }
        }

        expr_id
    }

    fn lower_block_expr(&mut self, block: BlockExpr<'_>, parent_scope: ScopeId) -> ExprId {
        let expr_id = self.alloc_expr(ExprKind::Block, block.syntax().range(), parent_scope);
        let body = self.lower_block_expr_with_owner(block, parent_scope);
        self.file.block_exprs.push(BlockExprInfo {
            owner: expr_id,
            body,
        });
        expr_id
    }

    fn lower_block_expr_with_owner(
        &mut self,
        block: BlockExpr<'_>,
        parent_scope: ScopeId,
    ) -> BodyId {
        let block_scope =
            self.new_scope(ScopeKind::Block, block.syntax().range(), Some(parent_scope));
        let body_id = self.new_body(BodyKind::Block, block.syntax().range(), block_scope, None);
        self.with_body(body_id, |this| this.lower_block_items(block, block_scope));
        body_id
    }

    fn lower_switch_arm(&mut self, arm: SwitchArm<'_>, scope: ScopeId) -> Option<ExprId> {
        let arm_scope = self.new_scope(ScopeKind::SwitchArm, arm.syntax().range(), Some(scope));
        if let Some(patterns) = arm.patterns() {
            self.lower_switch_patterns(patterns, arm_scope);
        }
        arm.value().map(|value| self.lower_expr(value, arm_scope))
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

fn literal_kind(kind: TokenKind) -> Option<LiteralKind> {
    match kind {
        TokenKind::Int => Some(LiteralKind::Int),
        TokenKind::Float => Some(LiteralKind::Float),
        TokenKind::String | TokenKind::RawString | TokenKind::BacktickString => {
            Some(LiteralKind::String)
        }
        TokenKind::Char => Some(LiteralKind::Char),
        TokenKind::TrueKw | TokenKind::FalseKw => Some(LiteralKind::Bool),
        _ => None,
    }
}

fn unary_operator(kind: TokenKind) -> Option<UnaryOperator> {
    match kind {
        TokenKind::Plus => Some(UnaryOperator::Plus),
        TokenKind::Minus => Some(UnaryOperator::Minus),
        TokenKind::Bang => Some(UnaryOperator::Not),
        _ => None,
    }
}

fn binary_operator(kind: TokenKind) -> Option<BinaryOperator> {
    match kind {
        TokenKind::PipePipe => Some(BinaryOperator::OrOr),
        TokenKind::Pipe => Some(BinaryOperator::Or),
        TokenKind::Caret => Some(BinaryOperator::Xor),
        TokenKind::AmpAmp => Some(BinaryOperator::AndAnd),
        TokenKind::Amp => Some(BinaryOperator::And),
        TokenKind::EqEq => Some(BinaryOperator::EqEq),
        TokenKind::BangEq => Some(BinaryOperator::NotEq),
        TokenKind::InKw => Some(BinaryOperator::In),
        TokenKind::Gt => Some(BinaryOperator::Gt),
        TokenKind::GtEq => Some(BinaryOperator::GtEq),
        TokenKind::Lt => Some(BinaryOperator::Lt),
        TokenKind::LtEq => Some(BinaryOperator::LtEq),
        TokenKind::QuestionQuestion => Some(BinaryOperator::NullCoalesce),
        TokenKind::Range => Some(BinaryOperator::Range),
        TokenKind::RangeEq => Some(BinaryOperator::RangeInclusive),
        TokenKind::Plus => Some(BinaryOperator::Add),
        TokenKind::Minus => Some(BinaryOperator::Subtract),
        TokenKind::Star => Some(BinaryOperator::Multiply),
        TokenKind::Slash => Some(BinaryOperator::Divide),
        TokenKind::Percent => Some(BinaryOperator::Remainder),
        TokenKind::StarStar => Some(BinaryOperator::Power),
        TokenKind::Shl => Some(BinaryOperator::ShiftLeft),
        TokenKind::Shr => Some(BinaryOperator::ShiftRight),
        _ => None,
    }
}

fn assignment_operator(kind: TokenKind) -> Option<AssignmentOperator> {
    match kind {
        TokenKind::Eq => Some(AssignmentOperator::Assign),
        TokenKind::QuestionQuestionEq => Some(AssignmentOperator::NullCoalesce),
        TokenKind::PlusEq => Some(AssignmentOperator::Add),
        TokenKind::MinusEq => Some(AssignmentOperator::Subtract),
        TokenKind::StarEq => Some(AssignmentOperator::Multiply),
        TokenKind::SlashEq => Some(AssignmentOperator::Divide),
        TokenKind::PercentEq => Some(AssignmentOperator::Remainder),
        TokenKind::StarStarEq => Some(AssignmentOperator::Power),
        TokenKind::ShlEq => Some(AssignmentOperator::ShiftLeft),
        TokenKind::ShrEq => Some(AssignmentOperator::ShiftRight),
        TokenKind::PipeEq => Some(AssignmentOperator::Or),
        TokenKind::CaretEq => Some(AssignmentOperator::Xor),
        TokenKind::AmpEq => Some(AssignmentOperator::And),
        _ => None,
    }
}
