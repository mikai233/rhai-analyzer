use std::collections::HashMap;

use crate::lowering::ctx::{LoweringContext, PendingMutationKind};
use crate::model::{
    CallSiteId, ExpectedTypeSite, ExpectedTypeSource, FileHir, ReferenceKind, ScopeId, ScopeKind,
    Symbol, SymbolId, SymbolKind, SymbolMutation, SymbolMutationKind, SymbolRead, SymbolReadKind,
};
use rhai_syntax::TextSize;

impl<'a> LoweringContext<'a> {
    pub(crate) fn finish(mut self) -> FileHir {
        self.resolve_references();
        self.annotate_imports();
        self.resolve_reads();
        self.resolve_call_mappings();
        self.resolve_value_flows();
        self.resolve_mutations();
        self.annotate_symbol_relationships();
        self.annotate_expected_type_sites();
        self.file
    }

    pub(crate) fn resolve_references(&mut self) {
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
                    .push(crate::ReferenceId(index as u32));
            }
        }
    }

    pub(crate) fn resolve_value_flows(&mut self) {
        let pending_flows = std::mem::take(&mut self.pending_value_flows);
        for pending in pending_flows {
            let Some(symbol) = self.file.references[pending.reference.0 as usize].target else {
                continue;
            };
            self.push_value_flow(symbol, pending.expr, pending.kind, pending.range);
        }
    }

    pub(crate) fn resolve_mutations(&mut self) {
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

    pub(crate) fn resolve_reads(&mut self) {
        let pending_reads = std::mem::take(&mut self.pending_reads);
        for pending in pending_reads {
            let Some(symbol) = self.file.references[pending.root_reference.0 as usize].target
            else {
                continue;
            };
            self.file.symbol_reads.push(SymbolRead {
                symbol,
                owner: pending.owner,
                kind: SymbolReadKind::Path {
                    segments: pending.segments,
                },
                range: pending.range,
            });
        }
    }

    pub(crate) fn first_reference_in_range(
        &self,
        range: rhai_syntax::TextRange,
    ) -> Option<crate::ReferenceId> {
        self.file
            .references
            .iter()
            .enumerate()
            .find_map(|(index, reference)| {
                (reference.range.start() >= range.start() && reference.range.end() <= range.end())
                    .then_some(crate::ReferenceId(index as u32))
            })
    }

    pub(crate) fn file_scope_id(&self) -> Option<ScopeId> {
        self.file
            .scopes
            .iter()
            .enumerate()
            .find_map(|(index, scope)| {
                (scope.kind == ScopeKind::File).then_some(ScopeId(index as u32))
            })
    }

    pub(crate) fn resolve_name_at(
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

    pub(crate) fn resolve_name_in_scope(
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

    pub(crate) fn symbol_is_visible_at(
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

    pub(crate) fn annotate_symbol_relationships(&mut self) {
        for symbol in &mut self.file.symbols {
            symbol.shadowed = None;
            symbol.duplicate_of = None;
        }

        for scope_index in 0..self.file.scopes.len() {
            let symbols = self.file.scopes[scope_index].symbols.clone();
            let mut seen = HashMap::<crate::model::SymbolConflictKey, SymbolId>::new();

            for symbol_id in symbols {
                let (key, name, start) = {
                    let symbol = &self.file.symbols[symbol_id.0 as usize];
                    (
                        self.file.symbol_conflict_key(symbol_id),
                        symbol.name.clone(),
                        symbol.range.start(),
                    )
                };

                if let Some(previous) = seen.get(&key).copied() {
                    self.file.symbols[symbol_id.0 as usize].duplicate_of = Some(previous);
                    continue;
                }

                seen.insert(key, symbol_id);

                let shadowed = self.file.scopes[scope_index]
                    .parent
                    .and_then(|parent| self.resolve_name_at(parent, &name, start));
                self.file.symbols[symbol_id.0 as usize].shadowed = shadowed;
            }
        }
    }

    pub(crate) fn annotate_imports(&mut self) {
        for import in &mut self.file.imports {
            if import.linkage == crate::ImportLinkageKind::StaticText {
                continue;
            }
            import.linkage = match import
                .module_reference
                .and_then(|reference| self.file.references[reference.0 as usize].target)
            {
                Some(_) => crate::ImportLinkageKind::LocalSymbol,
                None => crate::ImportLinkageKind::DynamicExpr,
            };
        }
    }

    pub(crate) fn annotate_expected_type_sites(&mut self) {
        self.file.expected_type_sites.clear();

        for flow in &self.file.value_flows {
            self.file.expected_type_sites.push(ExpectedTypeSite {
                expr: flow.expr,
                source: ExpectedTypeSource::Symbol(flow.symbol),
            });
        }

        for (index, symbol) in self.file.symbols.iter().enumerate() {
            if symbol.kind != SymbolKind::Function {
                continue;
            }
            let symbol_id = SymbolId(index as u32);
            let Some(body) = self.file.body_of(symbol_id) else {
                continue;
            };

            let return_values = self.file.body_return_values(body).collect::<Vec<_>>();
            for expr in return_values {
                self.file.expected_type_sites.push(ExpectedTypeSite {
                    expr,
                    source: ExpectedTypeSource::FunctionReturn(symbol_id),
                });
            }

            if self.file.body_may_fall_through(body)
                && let Some(expr) = self.file.body_tail_value(body)
            {
                self.file.expected_type_sites.push(ExpectedTypeSite {
                    expr,
                    source: ExpectedTypeSource::FunctionReturn(symbol_id),
                });
            }
        }

        for (index, call) in self.file.calls.iter().enumerate() {
            let call_id = CallSiteId(index as u32);
            let arg_offset = self.file.caller_scope_arg_offset(call);
            for (argument_index, expr) in
                call.arg_exprs.iter().copied().enumerate().skip(arg_offset)
            {
                self.file.expected_type_sites.push(ExpectedTypeSite {
                    expr,
                    source: ExpectedTypeSource::CallArgument {
                        call: call_id,
                        parameter_index: argument_index - arg_offset,
                    },
                });
            }
        }
    }
}
