use std::collections::HashSet;

use crate::model::{
    FileHir, ReferenceId, ReferenceKind, SemanticDiagnostic, SemanticDiagnosticKind,
};

impl FileHir {
    pub fn diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = self.unresolved_import_export_diagnostics();
        diagnostics.extend(self.invalid_import_module_type_diagnostics());
        diagnostics.extend(self.invalid_export_target_diagnostics());
        diagnostics.extend(self.unresolved_name_diagnostics());
        diagnostics.extend(self.duplicate_definition_diagnostics());
        diagnostics.extend(self.doc_type_consistency_diagnostics());
        diagnostics.extend(self.unused_symbol_diagnostics());
        diagnostics
    }

    pub(crate) fn unresolved_name_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let handled = self.import_export_reference_ids();
        self.references
            .iter()
            .enumerate()
            .filter(|reference| {
                let (index, reference) = reference;
                matches!(reference.kind, ReferenceKind::Name | ReferenceKind::This)
                    && reference.target.is_none()
                    && reference.name != "this"
                    && !handled.contains(&ReferenceId(*index as u32))
            })
            .map(|(_, reference)| SemanticDiagnostic {
                kind: SemanticDiagnosticKind::UnresolvedName,
                range: reference.range,
                message: format!("unresolved name `{}`", reference.name),
                related_range: None,
            })
            .collect()
    }

    pub(crate) fn duplicate_definition_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        self.symbols
            .iter()
            .filter_map(|symbol| {
                let previous = symbol.duplicate_of?;
                Some(SemanticDiagnostic {
                    kind: SemanticDiagnosticKind::DuplicateDefinition,
                    range: symbol.range,
                    message: format!("duplicate definition of `{}`", symbol.name),
                    related_range: Some(self.symbol(previous).range),
                })
            })
            .collect()
    }

    pub(crate) fn import_export_reference_ids(&self) -> HashSet<ReferenceId> {
        let mut references = HashSet::new();
        for import in &self.imports {
            if let Some(reference) = import.module_reference {
                references.insert(reference);
            }
        }
        for export in &self.exports {
            if export.target_symbol.is_none()
                && let Some(reference) = export.target_reference
            {
                references.insert(reference);
            }
        }
        references
    }
}
