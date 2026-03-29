use crate::model::{FileHir, SemanticDiagnostic, SemanticDiagnosticKind, Symbol, SymbolKind};

impl FileHir {
    pub(crate) fn unused_symbol_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.symbols
            .iter()
            .filter(|symbol| self.is_unused_symbol_candidate(symbol))
            .filter(|symbol| symbol.references.is_empty())
            .map(|symbol| SemanticDiagnostic {
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
}
