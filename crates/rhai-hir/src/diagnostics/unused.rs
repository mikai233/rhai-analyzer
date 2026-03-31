use crate::model::{
    FileHir, ReferenceKind, ScopeId, ScopeKind, SemanticDiagnostic, SemanticDiagnosticKind, Symbol,
    SymbolId, SymbolKind,
};
use rhai_syntax::TextSize;

impl FileHir {
    pub(crate) fn unused_symbol_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| (SymbolId(index as u32), symbol))
            .filter(|(_, symbol)| self.is_unused_symbol_candidate(symbol))
            .filter(|(symbol_id, symbol)| {
                symbol.references.is_empty()
                    && !self.has_unresolved_caller_scope_reference(*symbol_id, symbol)
            })
            .map(|(_, symbol)| SemanticDiagnostic {
                kind: SemanticDiagnosticKind::UnusedSymbol,
                range: symbol.range,
                message: format!("unused symbol `{}`", symbol.name),
                related_range: None,
            })
            .collect()
    }

    fn is_unused_symbol_candidate(&self, symbol: &Symbol) -> bool {
        matches!(
            symbol.kind,
            SymbolKind::Variable
                | SymbolKind::Parameter
                | SymbolKind::Constant
                | SymbolKind::ImportAlias
        ) && !symbol.name.starts_with('_')
    }

    fn has_unresolved_caller_scope_reference(&self, symbol_id: SymbolId, symbol: &Symbol) -> bool {
        self.references.iter().any(|reference| {
            if reference.kind != ReferenceKind::Name
                || reference.target.is_some()
                || reference.name != symbol.name
            {
                return false;
            }

            let Some(function_scope) = self.enclosing_function_scope(reference.scope) else {
                return false;
            };
            let Some(parent_scope) = self.scope(function_scope).parent else {
                return false;
            };

            self.resolve_name_in_outer_scopes(
                parent_scope,
                symbol.name.as_str(),
                reference.range.start(),
            ) == Some(symbol_id)
        })
    }

    fn enclosing_function_scope(&self, mut scope: ScopeId) -> Option<ScopeId> {
        loop {
            let scope_data = self.scope(scope);
            if scope_data.kind == ScopeKind::Function {
                return Some(scope);
            }
            scope = scope_data.parent?;
        }
    }

    fn resolve_name_in_outer_scopes(
        &self,
        mut scope: ScopeId,
        name: &str,
        reference_start: TextSize,
    ) -> Option<SymbolId> {
        loop {
            if let Some(symbol) =
                self.scope(scope)
                    .symbols
                    .iter()
                    .rev()
                    .copied()
                    .find(|symbol_id| {
                        let symbol = self.symbol(*symbol_id);
                        symbol.name == name && symbol.range.start() <= reference_start
                    })
            {
                return Some(symbol);
            }
            scope = self.scope(scope).parent?;
        }
    }
}
