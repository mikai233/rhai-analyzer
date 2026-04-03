use std::collections::HashSet;

use rhai_syntax::{TextRange, TextSize};

use crate::model::{ExprId, FileHir, ReferenceId, ScopeId, ScopeKind, SymbolId, SymbolKind};

impl FileHir {
    pub fn find_scope_at(&self, offset: TextSize) -> Option<ScopeId> {
        self.find_scope_containing(offset)
    }

    fn find_scope_containing(&self, offset: TextSize) -> Option<ScopeId> {
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

    pub fn find_scope_for_cursor(&self, offset: TextSize) -> Option<ScopeId> {
        self.find_scope_at(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.find_scope_at(offset))
        })
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

    pub fn expr_at_cursor(&self, offset: TextSize) -> Option<ExprId> {
        self.expr_at_offset(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.expr_at_offset(offset))
        })
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

    pub fn reference_at_cursor(&self, offset: TextSize) -> Option<ReferenceId> {
        self.reference_at_offset(offset).or_else(|| {
            self.previous_cursor_offset(offset)
                .and_then(|offset| self.reference_at_offset(offset))
        })
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

    pub fn visible_symbols_at(&self, offset: TextSize) -> Vec<SymbolId> {
        self.visible_symbols_with_scope_distance_at(offset)
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect()
    }

    pub fn visible_symbols_at_cursor(&self, offset: TextSize) -> Vec<SymbolId> {
        self.visible_symbols_with_scope_distance_at_cursor(offset)
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect()
    }

    pub fn visible_symbols_with_scope_distance_at(&self, offset: TextSize) -> Vec<(SymbolId, u8)> {
        let mut visible = Vec::new();
        let mut hidden_symbols = HashSet::new();
        let mut scope = match self.find_scope_at(offset) {
            Some(scope) => scope,
            None => return visible,
        };
        let mut crossed_function_boundary = false;
        let mut scope_distance = 0u8;

        loop {
            let scope_data = self.scope(scope);
            for symbol_id in scope_data.symbols.iter().rev().copied() {
                let key = self.symbol_conflict_key(symbol_id);
                if hidden_symbols.contains(&key) {
                    continue;
                }
                if self.symbol_is_visible_at(symbol_id, offset, crossed_function_boundary) {
                    hidden_symbols.insert(key);
                    visible.push((symbol_id, scope_distance));
                }
            }

            crossed_function_boundary |= scope_data.kind == ScopeKind::Function;
            match scope_data.parent {
                Some(parent) => {
                    scope = parent;
                    scope_distance = scope_distance.saturating_add(1);
                }
                None => break,
            }
        }

        visible
    }

    pub fn visible_symbols_with_scope_distance_at_cursor(
        &self,
        offset: TextSize,
    ) -> Vec<(SymbolId, u8)> {
        self.find_scope_at(offset)
            .map(|_| self.visible_symbols_with_scope_distance_at(offset))
            .or_else(|| {
                self.previous_cursor_offset(offset)
                    .map(|offset| self.visible_symbols_with_scope_distance_at(offset))
            })
            .unwrap_or_default()
    }

    pub(crate) fn symbol_is_visible_at(
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

    pub(crate) fn previous_cursor_offset(&self, offset: TextSize) -> Option<TextSize> {
        let root_end = self.root_range.end();
        let offset = offset.min(root_end);
        u32::from(offset).checked_sub(1).map(TextSize::from)
    }
}
