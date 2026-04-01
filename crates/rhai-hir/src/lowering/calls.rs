use crate::lowering::ctx::LoweringContext;
use crate::model::ExprKind;
use rhai_syntax::{AstNode, CallExpr, TextRange, TextSize};

impl<'a> LoweringContext<'a> {
    pub(crate) fn lower_call_expr(&mut self, call: CallExpr, scope: crate::ScopeId) {
        let reference_start = self.file.references.len();
        let callee_range = call.callee().map(|callee| callee.syntax().text_range());
        if let Some(callee) = call.callee() {
            self.lower_expr(callee, scope);
        }
        let callee_reference =
            callee_range.and_then(|range| self.first_reference_from(reference_start, range));
        let mut arg_ranges = Vec::new();
        let mut arg_exprs = Vec::new();
        if let Some(args) = call.args() {
            for arg in args.args() {
                arg_ranges.push(arg.syntax().text_range());
                arg_exprs.push(self.lower_expr(arg, scope));
            }
        }
        self.new_call(
            call.syntax().text_range(),
            scope,
            call.uses_caller_scope(),
            callee_range,
            callee_reference,
            arg_ranges,
            arg_exprs,
        );
    }

    pub(crate) fn resolve_call_mappings(&mut self) {
        for index in 0..self.file.calls.len() {
            let (
                call_scope,
                caller_scope,
                callee_reference,
                arg_count,
                first_arg_range,
                call_start,
            ) = {
                let call = &self.file.calls[index];
                (
                    call.scope,
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
                first_arg_range.and_then(|range| {
                    self.resolve_caller_scope_target(
                        range,
                        call_start,
                        arg_count.saturating_sub(caller_scope_arg_offset),
                    )
                })
            } else {
                callee_reference.and_then(|reference| {
                    let current_target = self.file.references[reference.0 as usize].target;
                    let current_is_function = current_target.is_some_and(|symbol| {
                        self.file.symbols[symbol.0 as usize].kind == crate::SymbolKind::Function
                    });
                    if current_target.is_none() || current_is_function {
                        self.resolve_function_overload_at(
                            call_scope,
                            self.file.references[reference.0 as usize].name.as_str(),
                            call_start,
                            arg_count,
                        )
                        .or(current_target)
                    } else {
                        current_target
                    }
                })
            };

            let parameter_bindings = resolved_callee
                .filter(|symbol| {
                    self.file.symbols[symbol.0 as usize].kind == crate::SymbolKind::Function
                })
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
            if let Some(reference) = callee_reference
                && let Some(symbol) = resolved_callee
                && self.file.symbols[symbol.0 as usize].kind == crate::SymbolKind::Function
            {
                self.file.references[reference.0 as usize].target = Some(symbol);
            }
        }
    }

    pub(crate) fn resolve_caller_scope_target(
        &self,
        first_arg_range: TextRange,
        call_start: TextSize,
        arg_count: usize,
    ) -> Option<crate::SymbolId> {
        if let Some(reference) = self.first_reference_in_range(first_arg_range) {
            let name = self.file.reference(reference).name.as_str();
            if let Some(target) = self.resolve_function_overload_at(
                self.file_scope_id()?,
                name,
                call_start,
                arg_count,
            ) {
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
        let name = name
            .strip_prefix('"')
            .and_then(|name| name.strip_suffix('"'))
            .unwrap_or(name);
        self.resolve_function_overload_at(self.file_scope_id()?, name, call_start, arg_count)
    }

    fn resolve_function_overload_at(
        &self,
        mut scope: crate::ScopeId,
        name: &str,
        reference_start: TextSize,
        arg_count: usize,
    ) -> Option<crate::SymbolId> {
        let mut crossed_function_boundary = false;
        loop {
            if let Some(symbol) = self.resolve_function_overload_in_scope(
                scope,
                name,
                reference_start,
                crossed_function_boundary,
                arg_count,
            ) {
                return Some(symbol);
            }

            crossed_function_boundary |=
                self.file.scopes[scope.0 as usize].kind == crate::ScopeKind::Function;
            scope = self.file.scopes[scope.0 as usize].parent?;
        }
    }

    fn resolve_function_overload_in_scope(
        &self,
        scope: crate::ScopeId,
        name: &str,
        reference_start: TextSize,
        crossed_function_boundary: bool,
        arg_count: usize,
    ) -> Option<crate::SymbolId> {
        self.file.scopes[scope.0 as usize]
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = &self.file.symbols[symbol_id.0 as usize];
                symbol.kind == crate::SymbolKind::Function
                    && symbol.name == name
                    && self.symbol_is_visible_at(symbol, reference_start, crossed_function_boundary)
                    && self.file.function_parameters(*symbol_id).len() == arg_count
            })
    }
}
