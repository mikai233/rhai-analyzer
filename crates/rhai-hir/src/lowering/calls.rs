use crate::lowering::ctx::LoweringContext;
use crate::model::ExprKind;
use rhai_syntax::{AstNode, CallExpr, TextRange, TextSize};

impl<'a> LoweringContext<'a> {
    pub(crate) fn lower_call_expr(&mut self, call: CallExpr<'_>, scope: crate::ScopeId) {
        let reference_start = self.file.references.len();
        let callee_range = call.callee().map(|callee| callee.syntax().range());
        if let Some(callee) = call.callee() {
            self.lower_expr(callee, scope);
        }
        let callee_reference =
            callee_range.and_then(|range| self.first_reference_from(reference_start, range));
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

    pub(crate) fn resolve_call_mappings(&mut self) {
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
        }
    }

    pub(crate) fn resolve_caller_scope_target(
        &self,
        first_arg_range: TextRange,
        call_start: TextSize,
    ) -> Option<crate::SymbolId> {
        if let Some(reference) = self.first_reference_in_range(first_arg_range) {
            let target = self.file.reference(reference).target?;
            if self.file.symbols[target.0 as usize].kind == crate::SymbolKind::Function {
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
            .filter(|symbol| {
                self.file.symbols[symbol.0 as usize].kind == crate::SymbolKind::Function
            })
    }
}
