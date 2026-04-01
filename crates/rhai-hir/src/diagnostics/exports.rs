use crate::model::{
    FileHir, ScopeKind, SemanticDiagnostic, SemanticDiagnosticCode, SemanticDiagnosticKind,
    SymbolKind,
};

impl FileHir {
    pub(crate) fn unresolved_import_export_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for import in &self.imports {
            if let Some(reference_id) = import.module_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedImport,
                        code: SemanticDiagnosticCode::UnresolvedImportModule,
                        range: reference.range,
                        message: format!("unresolved import module `{}`", reference.name),
                        related_range: Some(import.range),
                    });
                }
            }
        }

        for export in &self.exports {
            if export.target_symbol.is_some() {
                continue;
            }
            if let Some(reference_id) = export.target_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedExport,
                        code: SemanticDiagnosticCode::UnresolvedExportTarget,
                        range: reference.range,
                        message: format!("unresolved export target `{}`", reference.name),
                        related_range: Some(export.range),
                    });
                }
            }
        }

        diagnostics
    }

    pub(crate) fn invalid_export_target_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.exports
            .iter()
            .filter_map(|export| {
                let reference_id = export.target_reference?;
                let symbol_id = self.definition_of(reference_id)?;
                let symbol = self.symbol(symbol_id);
                (matches!(
                    symbol.kind,
                    SymbolKind::Function
                        | SymbolKind::ImportAlias
                        | SymbolKind::ExportAlias
                        | SymbolKind::Parameter
                ) || !matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Constant)
                    || self.scope(symbol.scope).kind != ScopeKind::File)
                    .then(|| SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InvalidExportTarget,
                        code: SemanticDiagnosticCode::InvalidExportTarget,
                        range: self.reference(reference_id).range,
                        message: format!(
                            "export target `{}` must refer to a global variable or constant",
                            self.reference(reference_id).name
                        ),
                        related_range: Some(export.range),
                    })
            })
            .collect()
    }
}
