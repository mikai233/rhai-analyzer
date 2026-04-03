use rhai_syntax::TextSize;

use crate::model::{
    CallSite, CallSiteId, FileHir, ParameterHint, ParameterHintParameter, ScopeKind, SymbolId,
    SymbolKind,
};
use crate::ty::TypeRef;

impl FileHir {
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

    pub fn enclosing_function_symbol_for_cursor(&self, offset: TextSize) -> Option<SymbolId> {
        self.enclosing_function_symbol_at(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.enclosing_function_symbol_at(offset))
        })
    }

    pub fn this_type_at(&self, offset: TextSize) -> Option<TypeRef> {
        let function = self.enclosing_function_symbol_at(offset)?;
        Some(
            self.function_info(function)
                .and_then(|info| info.this_type.clone())
                .unwrap_or(TypeRef::Unknown),
        )
    }

    pub fn this_type_for_cursor(&self, offset: TextSize) -> Option<TypeRef> {
        self.this_type_at(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.this_type_at(offset))
        })
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

    pub fn call_at_cursor(&self, offset: TextSize) -> Option<CallSiteId> {
        self.call_at_offset(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.call_at_offset(offset))
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

    pub fn parameter_hint_at_cursor(&self, offset: TextSize) -> Option<ParameterHint> {
        self.parameter_hint_at(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.parameter_hint_at(offset))
        })
    }

    pub fn active_parameter_at_offset(&self, offset: TextSize) -> Option<usize> {
        let call_id = self.call_at_offset(offset)?;
        self.active_parameter_index(self.call(call_id), offset)
    }

    pub fn active_parameter_at_cursor(&self, offset: TextSize) -> Option<usize> {
        self.active_parameter_at_offset(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.active_parameter_at_offset(offset))
        })
    }

    pub(crate) fn caller_scope_arg_offset(&self, call: &CallSite) -> usize {
        usize::from(
            call.caller_scope
                && call
                    .callee_reference
                    .map(|reference| self.reference(reference).name.as_str())
                    == Some("call"),
        )
    }

    pub(crate) fn active_parameter_index(
        &self,
        call: &CallSite,
        offset: TextSize,
    ) -> Option<usize> {
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
