use std::collections::{BTreeMap, HashSet};

use rhai_syntax::{TextRange, TextSize};

use crate::{
    ArrayExprInfo, BinaryExprInfo, BlockExprInfo, BodyId, CallSite, CallSiteId, ClosureExprInfo,
    CompletionSymbol, ControlFlowEvent, ControlFlowMergePoint, DocBlock, DocBlockId, DocTag,
    DocumentedField, ExportDirective, ExprId, ExprKind, FileHir, FindReferencesResult,
    FunctionTypeRef, IfExprInfo, ImportDirective, IndexExprInfo, LiteralInfo, MemberAccess,
    MemberCompletion, MemberCompletionSource, NavigationTarget, ParameterHint,
    ParameterHintParameter, ReferenceId, ReferenceLocation, ScopeId, ScopeKind, SwitchExprInfo,
    SymbolId, SymbolKind, TypeRef, TypeSlotId, UnaryExprInfo, WorkspaceSymbol,
};

impl FileHir {
    pub fn literal(&self, expr: ExprId) -> Option<&LiteralInfo> {
        self.literals.iter().find(|literal| literal.owner == expr)
    }

    pub fn array_expr(&self, expr: ExprId) -> Option<&ArrayExprInfo> {
        self.array_exprs.iter().find(|array| array.owner == expr)
    }

    pub fn block_expr(&self, expr: ExprId) -> Option<&BlockExprInfo> {
        self.block_exprs.iter().find(|block| block.owner == expr)
    }

    pub fn if_expr(&self, expr: ExprId) -> Option<&IfExprInfo> {
        self.if_exprs.iter().find(|if_expr| if_expr.owner == expr)
    }

    pub fn switch_expr(&self, expr: ExprId) -> Option<&SwitchExprInfo> {
        self.switch_exprs
            .iter()
            .find(|switch_expr| switch_expr.owner == expr)
    }

    pub fn closure_expr(&self, expr: ExprId) -> Option<&ClosureExprInfo> {
        self.closure_exprs
            .iter()
            .find(|closure| closure.owner == expr)
    }

    pub fn for_expr(&self, expr: ExprId) -> Option<&crate::ForExprInfo> {
        self.for_exprs
            .iter()
            .find(|for_expr| for_expr.owner == expr)
    }

    pub fn function_info(&self, function: SymbolId) -> Option<&crate::FunctionInfo> {
        self.function_infos
            .iter()
            .find(|info| info.symbol == function)
    }

    pub fn enclosing_function_symbol_at(&self, offset: TextSize) -> Option<SymbolId> {
        let mut scope = self.find_scope_at(offset)?;

        loop {
            let scope_data = self.scope(scope);
            if scope_data.kind == ScopeKind::Function {
                return self
                    .bodies
                    .iter()
                    .find(|body| body.scope == scope && body.owner.is_some())
                    .and_then(|body| body.owner);
            }

            scope = scope_data.parent?;
        }
    }

    pub fn this_type_at(&self, offset: TextSize) -> Option<TypeRef> {
        let function = self.enclosing_function_symbol_at(offset)?;
        Some(
            self.function_info(function)
                .and_then(|info| info.this_type.clone())
                .unwrap_or(TypeRef::Unknown),
        )
    }

    pub fn unary_expr(&self, expr: ExprId) -> Option<&UnaryExprInfo> {
        self.unary_exprs.iter().find(|unary| unary.owner == expr)
    }

    pub fn binary_expr(&self, expr: ExprId) -> Option<&BinaryExprInfo> {
        self.binary_exprs.iter().find(|binary| binary.owner == expr)
    }

    pub fn assign_expr(&self, expr: ExprId) -> Option<&crate::AssignExprInfo> {
        self.assign_exprs.iter().find(|assign| assign.owner == expr)
    }

    pub fn index_expr(&self, expr: ExprId) -> Option<&IndexExprInfo> {
        self.index_exprs.iter().find(|index| index.owner == expr)
    }

    pub fn member_access(&self, expr: ExprId) -> Option<&MemberAccess> {
        self.member_accesses
            .iter()
            .find(|access| access.owner == expr)
    }

    pub fn body_of(&self, owner: SymbolId) -> Option<BodyId> {
        self.bodies
            .iter()
            .position(|body| body.owner == Some(owner))
            .map(|index| BodyId(index as u32))
    }

    pub fn body_control_flow(&self, body: BodyId) -> impl Iterator<Item = &ControlFlowEvent> + '_ {
        self.body(body).control_flow.iter()
    }

    pub fn body_return_values(&self, body: BodyId) -> impl Iterator<Item = ExprId> + '_ {
        self.body(body).return_values.iter().copied()
    }

    pub fn body_throw_values(&self, body: BodyId) -> impl Iterator<Item = ExprId> + '_ {
        self.body(body).throw_values.iter().copied()
    }

    pub fn body_tail_value(&self, body: BodyId) -> Option<ExprId> {
        self.body(body).tail_value
    }

    pub fn body_merge_points(
        &self,
        body: BodyId,
    ) -> impl Iterator<Item = &ControlFlowMergePoint> + '_ {
        self.body(body).merge_points.iter()
    }

    pub fn body_may_fall_through(&self, body: BodyId) -> bool {
        self.body(body).may_fall_through
    }

    pub fn body_unreachable_ranges(&self, body: BodyId) -> impl Iterator<Item = TextRange> + '_ {
        self.body(body).unreachable_ranges.iter().copied()
    }

    pub fn import(&self, index: usize) -> &ImportDirective {
        &self.imports[index]
    }

    pub fn export(&self, index: usize) -> &ExportDirective {
        &self.exports[index]
    }

    pub fn doc_block(&self, id: DocBlockId) -> &DocBlock {
        &self.docs[id.0 as usize]
    }

    pub fn documented_fields(&self, symbol: SymbolId) -> Vec<DocumentedField> {
        let Some(doc_id) = self.symbol(symbol).docs else {
            return Vec::new();
        };

        self.doc_block(doc_id)
            .tags
            .iter()
            .filter_map(|tag| match tag {
                DocTag::Field { name, ty } => Some(DocumentedField {
                    name: name.clone(),
                    annotation: ty.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    pub fn find_scope_at(&self, offset: TextSize) -> Option<ScopeId> {
        self.scopes
            .iter()
            .enumerate()
            .filter_map(|(index, scope)| {
                scope
                    .range
                    .contains(offset)
                    .then_some((ScopeId(index as u32), scope.range.len()))
            })
            .min_by_key(|(_, len)| *len)
            .map(|(id, _)| id)
    }

    pub fn symbol_at(&self, range: TextRange) -> Option<SymbolId> {
        self.symbols
            .iter()
            .position(|symbol| symbol.range == range)
            .map(|index| SymbolId(index as u32))
    }

    pub fn symbol_at_offset(&self, offset: TextSize) -> Option<SymbolId> {
        self.symbols
            .iter()
            .enumerate()
            .filter_map(|(index, symbol)| {
                symbol
                    .range
                    .contains(offset)
                    .then_some((SymbolId(index as u32), symbol.range.len()))
            })
            .min_by_key(|(_, len)| *len)
            .map(|(id, _)| id)
    }

    pub fn expr_at(&self, range: TextRange) -> Option<ExprId> {
        self.exprs
            .iter()
            .position(|expr| expr.range == range)
            .map(|index| ExprId(index as u32))
    }

    pub fn expr_at_offset(&self, offset: TextSize) -> Option<ExprId> {
        self.exprs
            .iter()
            .enumerate()
            .filter_map(|(index, expr)| {
                expr.range
                    .contains(offset)
                    .then_some((ExprId(index as u32), expr.range.len()))
            })
            .min_by_key(|(_, len)| *len)
            .map(|(id, _)| id)
    }

    pub fn expr_result_slot_at_offset(&self, offset: TextSize) -> Option<TypeSlotId> {
        let expr = self.expr_at_offset(offset)?;
        Some(self.expr_result_slot(expr))
    }

    pub fn references_to(&self, symbol: SymbolId) -> impl Iterator<Item = ReferenceId> + '_ {
        self.symbol(symbol).references.iter().copied()
    }

    pub fn definition_of(&self, reference: ReferenceId) -> Option<SymbolId> {
        self.reference(reference).target
    }

    pub fn definition_at_offset(&self, offset: TextSize) -> Option<SymbolId> {
        if let Some(reference) = self.reference_at_offset(offset) {
            return self.definition_of(reference);
        }

        self.symbol_at_offset(offset)
    }

    pub fn reference_at(&self, range: TextRange) -> Option<ReferenceId> {
        self.references
            .iter()
            .position(|reference| reference.range == range)
            .map(|index| ReferenceId(index as u32))
    }

    pub fn reference_at_offset(&self, offset: TextSize) -> Option<ReferenceId> {
        self.references
            .iter()
            .enumerate()
            .filter_map(|(index, reference)| {
                reference
                    .range
                    .contains(offset)
                    .then_some((ReferenceId(index as u32), reference.range.len()))
            })
            .min_by_key(|(_, len)| *len)
            .map(|(id, _)| id)
    }

    pub fn call_at_offset(&self, offset: TextSize) -> Option<CallSiteId> {
        self.calls
            .iter()
            .enumerate()
            .filter_map(|(index, call)| {
                call.range
                    .contains(offset)
                    .then_some((CallSiteId(index as u32), call.range.len()))
            })
            .min_by_key(|(_, len)| *len)
            .map(|(id, _)| id)
    }

    pub fn references_at_offset(&self, offset: TextSize) -> Vec<ReferenceId> {
        if let Some(reference) = self.reference_at_offset(offset) {
            if let Some(target) = self.definition_of(reference) {
                return self.references_to(target).collect();
            }
            return Vec::new();
        }

        if let Some(symbol) = self.symbol_at_offset(offset) {
            return self.references_to(symbol).collect();
        }

        Vec::new()
    }

    pub fn goto_definition(&self, offset: TextSize) -> Option<NavigationTarget> {
        let symbol = self.definition_at_offset(offset)?;
        Some(self.navigation_target(symbol))
    }

    pub fn find_references(&self, offset: TextSize) -> Option<FindReferencesResult> {
        let symbol = if let Some(reference) = self.reference_at_offset(offset) {
            self.definition_of(reference)?
        } else {
            self.symbol_at_offset(offset)?
        };

        let declaration = self.navigation_target(symbol);
        let references = self
            .references_to(symbol)
            .map(|reference_id| {
                let reference = self.reference(reference_id);
                ReferenceLocation {
                    reference: reference_id,
                    kind: reference.kind,
                    range: reference.range,
                    target: symbol,
                }
            })
            .collect();

        Some(FindReferencesResult {
            symbol,
            declaration,
            references,
        })
    }

    pub fn parameter_hint_at(&self, offset: TextSize) -> Option<ParameterHint> {
        let call_id = self.call_at_offset(offset)?;
        let call = self.call(call_id);
        let target = call.resolved_callee?;
        let function = self.symbol(target);
        if function.kind != SymbolKind::Function {
            return None;
        }
        let active_parameter = self.active_parameter_index(call, offset)?;

        let parameters = self
            .function_parameters(target)
            .into_iter()
            .map(|symbol_id| {
                let symbol = self.symbol(symbol_id);
                ParameterHintParameter {
                    symbol: Some(symbol_id),
                    name: symbol.name.clone(),
                    annotation: symbol.annotation.clone(),
                }
            })
            .collect::<Vec<_>>();

        let return_type = match &function.annotation {
            Some(TypeRef::Function(signature)) => Some((*signature.ret).clone()),
            _ => None,
        };

        Some(ParameterHint {
            call: call_id,
            callee: self.navigation_target(target),
            callee_name: function.name.clone(),
            active_parameter,
            parameters,
            return_type,
        })
    }

    pub fn visible_symbols_at(&self, offset: TextSize) -> Vec<SymbolId> {
        let mut visible = Vec::new();
        let mut hidden_names = HashSet::new();
        let mut scope = match self.find_scope_at(offset) {
            Some(scope) => scope,
            None => return visible,
        };
        let mut crossed_function_boundary = false;

        loop {
            let scope_data = self.scope(scope);
            for symbol_id in scope_data.symbols.iter().rev().copied() {
                let symbol = self.symbol(symbol_id);
                if hidden_names.contains(symbol.name.as_str()) {
                    continue;
                }
                if self.symbol_is_visible_at(symbol_id, offset, crossed_function_boundary) {
                    hidden_names.insert(symbol.name.clone());
                    visible.push(symbol_id);
                }
            }

            crossed_function_boundary |= scope_data.kind == ScopeKind::Function;
            match scope_data.parent {
                Some(parent) => scope = parent,
                None => break,
            }
        }

        visible
    }

    pub fn completion_symbols_at(&self, offset: TextSize) -> Vec<CompletionSymbol> {
        self.visible_symbols_at(offset)
            .into_iter()
            .map(|symbol_id| {
                let symbol = self.symbol(symbol_id);
                CompletionSymbol {
                    symbol: symbol_id,
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    range: symbol.range,
                    docs: symbol.docs,
                    annotation: symbol.annotation.clone(),
                }
            })
            .collect()
    }

    pub fn project_completion_symbols_at(
        &self,
        offset: TextSize,
        workspace: &[WorkspaceSymbol],
    ) -> Vec<WorkspaceSymbol> {
        let local_names = self
            .visible_symbols_at(offset)
            .into_iter()
            .map(|symbol| self.symbol(symbol).name.clone())
            .collect::<HashSet<_>>();

        workspace
            .iter()
            .filter(|symbol| !local_names.contains(symbol.name.as_str()))
            .cloned()
            .collect()
    }

    pub fn member_completion_at(&self, offset: TextSize) -> Vec<MemberCompletion> {
        let Some(access) = self.member_access_at_offset(offset) else {
            return Vec::new();
        };

        self.member_completions_for_expr(access.receiver)
    }

    fn symbol_is_visible_at(
        &self,
        symbol: SymbolId,
        offset: TextSize,
        crossed_function_boundary: bool,
    ) -> bool {
        let symbol = self.symbol(symbol);
        if crossed_function_boundary {
            let symbol_scope = self.scope(symbol.scope);
            return symbol.kind == SymbolKind::Function
                || (symbol.kind == SymbolKind::ImportAlias
                    && symbol_scope.kind == ScopeKind::File
                    && symbol.range.start() <= offset);
        }
        matches!(symbol.kind, SymbolKind::Function) || symbol.range.start() <= offset
    }

    fn member_access_at_offset(&self, offset: TextSize) -> Option<&MemberAccess> {
        self.member_accesses
            .iter()
            .filter(|access| {
                access.range.contains(offset)
                    || self
                        .reference(access.field_reference)
                        .range
                        .contains(offset)
            })
            .min_by_key(|access| access.range.len())
    }

    fn member_completions_for_expr(&self, expr: ExprId) -> Vec<MemberCompletion> {
        let mut members = BTreeMap::<String, MemberCompletion>::new();

        for field in self
            .object_fields
            .iter()
            .filter(|field| field.owner == expr)
        {
            members
                .entry(field.name.clone())
                .or_insert(MemberCompletion {
                    name: field.name.clone(),
                    annotation: field
                        .value
                        .and_then(|value| self.object_field_annotation_from_expr(value)),
                    range: Some(field.range),
                    source: MemberCompletionSource::ObjectLiteralField,
                });
        }

        if let Some(symbol) = self.symbol_for_expr(expr) {
            for field in self.documented_fields(symbol) {
                members
                    .entry(field.name.clone())
                    .or_insert(MemberCompletion {
                        name: field.name,
                        annotation: Some(field.annotation),
                        range: None,
                        source: MemberCompletionSource::DocumentedField,
                    });
            }

            for flow in self.value_flows_into(symbol) {
                for field in self
                    .object_fields
                    .iter()
                    .filter(|field| field.owner == flow.expr)
                {
                    members
                        .entry(field.name.clone())
                        .or_insert(MemberCompletion {
                            name: field.name.clone(),
                            annotation: field
                                .value
                                .and_then(|value| self.object_field_annotation_from_expr(value)),
                            range: Some(field.range),
                            source: MemberCompletionSource::ObjectLiteralField,
                        });
                }
            }
        }

        members.into_values().collect()
    }

    fn symbol_for_expr(&self, expr: ExprId) -> Option<SymbolId> {
        match self.expr(expr).kind {
            ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference)),
            _ => None,
        }
    }

    fn object_field_annotation_from_expr(&self, expr: ExprId) -> Option<TypeRef> {
        match self.expr(expr).kind {
            ExprKind::Literal => None,
            ExprKind::Object => Some(TypeRef::Object(
                self.object_fields
                    .iter()
                    .filter(|field| field.owner == expr)
                    .map(|field| {
                        (
                            field.name.clone(),
                            field
                                .value
                                .and_then(|value| self.object_field_annotation_from_expr(value))
                                .unwrap_or(TypeRef::Unknown),
                        )
                    })
                    .collect(),
            )),
            ExprKind::Array => Some(TypeRef::Array(Box::new(TypeRef::Unknown))),
            ExprKind::Closure => Some(TypeRef::Function(FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Unknown),
            })),
            ExprKind::Name => self
                .symbol_for_expr(expr)
                .and_then(|symbol| self.declared_symbol_type(symbol).cloned()),
            _ => None,
        }
    }

    fn caller_scope_arg_offset(&self, call: &CallSite) -> usize {
        usize::from(
            call.caller_scope
                && call
                    .callee_reference
                    .map(|reference| self.reference(reference).name.as_str())
                    == Some("call"),
        )
    }

    fn active_parameter_index(&self, call: &CallSite, offset: TextSize) -> Option<usize> {
        if call.arg_ranges.is_empty() {
            return Some(0);
        }

        let arg_offset = self.caller_scope_arg_offset(call);
        let mut index = 0usize;
        for (current, range) in call.arg_ranges.iter().enumerate() {
            if range.contains(offset) {
                return current.checked_sub(arg_offset);
            }

            if offset >= range.start() {
                index = current;
            }

            if let Some(next) = call.arg_ranges.get(current + 1)
                && offset >= range.end()
                && offset < next.start()
            {
                return (current + 1).checked_sub(arg_offset);
            }
        }

        index.checked_sub(arg_offset)
    }
}
