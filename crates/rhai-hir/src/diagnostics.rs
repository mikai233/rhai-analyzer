use std::collections::{HashMap, HashSet};

use crate::{
    DocTag, FileHir, ReferenceId, ReferenceKind, SemanticDiagnostic, SemanticDiagnosticKind,
    Symbol, SymbolId, SymbolKind, TypeRef,
};

impl FileHir {
    pub fn diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = self.unresolved_import_export_diagnostics();
        diagnostics.extend(self.unresolved_name_diagnostics());
        diagnostics.extend(self.duplicate_definition_diagnostics());
        diagnostics.extend(self.doc_type_consistency_diagnostics());
        diagnostics.extend(self.unused_symbol_diagnostics());
        diagnostics
    }

    fn unresolved_name_diagnostics(&self) -> Vec<SemanticDiagnostic> {
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

    fn unresolved_import_export_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for import in &self.imports {
            if let Some(reference_id) = import.module_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedImport,
                        range: reference.range,
                        message: format!("unresolved import module `{}`", reference.name),
                        related_range: Some(import.range),
                    });
                }
            }
        }

        for export in &self.exports {
            if let Some(reference_id) = export.target_reference {
                let reference = self.reference(reference_id);
                if reference.target.is_none() {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::UnresolvedExport,
                        range: reference.range,
                        message: format!("unresolved export target `{}`", reference.name),
                        related_range: Some(export.range),
                    });
                }
            }
        }

        diagnostics
    }

    fn duplicate_definition_diagnostics(&self) -> Vec<SemanticDiagnostic> {
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

    fn doc_type_consistency_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for (index, symbol) in self.symbols.iter().enumerate() {
            let Some(doc_id) = symbol.docs else {
                continue;
            };
            let symbol_id = SymbolId(index as u32);
            let docs = self.doc_block(doc_id);

            let mut param_tag_counts = HashMap::<&str, usize>::new();
            let mut return_tag_count = 0usize;

            for tag in &docs.tags {
                match tag {
                    DocTag::Param { name, .. } => {
                        *param_tag_counts.entry(name.as_str()).or_default() += 1;
                    }
                    DocTag::Return(_) => {
                        return_tag_count += 1;
                    }
                    _ => {}
                }
            }

            for (name, count) in param_tag_counts {
                if count > 1 {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InconsistentDocType,
                        range: docs.range,
                        message: format!("duplicate `@param` tag for `{name}`"),
                        related_range: Some(symbol.range),
                    });
                }
            }

            if return_tag_count > 1 {
                diagnostics.push(SemanticDiagnostic {
                    kind: SemanticDiagnosticKind::InconsistentDocType,
                    range: docs.range,
                    message: "duplicate `@return` tags".to_owned(),
                    related_range: Some(symbol.range),
                });
            }

            match symbol.kind {
                SymbolKind::Function => {
                    diagnostics.extend(self.function_doc_type_diagnostics(symbol_id, docs.range));
                }
                _ => {
                    if docs
                        .tags
                        .iter()
                        .any(|tag| matches!(tag, DocTag::Param { .. } | DocTag::Return(_)))
                    {
                        diagnostics.push(SemanticDiagnostic {
                            kind: SemanticDiagnosticKind::InconsistentDocType,
                            range: docs.range,
                            message: format!(
                                "function doc tags cannot be attached to `{}`",
                                symbol.name
                            ),
                            related_range: Some(symbol.range),
                        });
                    }
                }
            }
        }

        diagnostics
    }

    fn function_doc_type_diagnostics(
        &self,
        symbol: SymbolId,
        docs_range: rhai_syntax::TextRange,
    ) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();
        let function = self.symbol(symbol);

        if let Some(annotation) = &function.annotation
            && !matches!(annotation, TypeRef::Function(_))
        {
            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::InconsistentDocType,
                range: docs_range,
                message: format!(
                    "function `{}` has a non-function type annotation",
                    function.name
                ),
                related_range: Some(function.range),
            });
        }

        let Some(body_id) = self.body_of(symbol) else {
            return diagnostics;
        };
        let params = self
            .scope(self.body(body_id).scope)
            .symbols
            .iter()
            .copied()
            .filter(|symbol_id| self.symbol(*symbol_id).kind == SymbolKind::Parameter)
            .map(|symbol_id| self.symbol(symbol_id).name.as_str())
            .collect::<HashSet<_>>();

        if let Some(doc_id) = function.docs {
            for tag in &self.doc_block(doc_id).tags {
                if let DocTag::Param { name, .. } = tag
                    && !params.contains(name.as_str())
                {
                    diagnostics.push(SemanticDiagnostic {
                        kind: SemanticDiagnosticKind::InconsistentDocType,
                        range: docs_range,
                        message: format!(
                            "doc tag `@param {name}` does not match any parameter of `{}`",
                            function.name
                        ),
                        related_range: Some(function.range),
                    });
                }
            }
        }

        diagnostics
    }

    fn unused_symbol_diagnostics(&self) -> Vec<SemanticDiagnostic> {
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

    fn import_export_reference_ids(&self) -> HashSet<ReferenceId> {
        let mut references = HashSet::new();
        for import in &self.imports {
            if let Some(reference) = import.module_reference {
                references.insert(reference);
            }
        }
        for export in &self.exports {
            if let Some(reference) = export.target_reference {
                references.insert(reference);
            }
        }
        references
    }
}
