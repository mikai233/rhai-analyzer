use rhai_syntax::TextSize;

use crate::{
    FileHir, LinkedAlias, LinkedAliasKind, RenameOccurrence, RenameOccurrenceKind, RenamePlan,
    RenamePreflightIssue, RenamePreflightIssueKind, SymbolId,
};

impl FileHir {
    pub fn editable_rename_occurrence_at(&self, offset: TextSize) -> Option<RenameOccurrence> {
        if let Some(reference) = self.reference_at_offset(offset)
            && let Some(symbol) = self.definition_of(reference)
        {
            return Some(RenameOccurrence {
                symbol,
                range: self.reference(reference).range,
                kind: RenameOccurrenceKind::Reference,
            });
        }

        let symbol = self.symbol_at_offset(offset)?;
        Some(RenameOccurrence {
            symbol,
            range: self.symbol(symbol).range,
            kind: RenameOccurrenceKind::Definition,
        })
    }

    pub fn rename_plan(&self, symbol: SymbolId, new_name: impl Into<String>) -> RenamePlan {
        let new_name = new_name.into();
        let mut occurrences = Vec::new();
        occurrences.push(RenameOccurrence {
            symbol,
            range: self.symbol(symbol).range,
            kind: RenameOccurrenceKind::Definition,
        });
        occurrences.extend(
            self.references_to(symbol)
                .map(|reference| RenameOccurrence {
                    symbol,
                    range: self.reference(reference).range,
                    kind: RenameOccurrenceKind::Reference,
                }),
        );
        occurrences.sort_by_key(|occurrence| occurrence.range.start());

        RenamePlan {
            target: self.file_backed_symbol_identity(symbol),
            new_name: new_name.clone(),
            occurrences,
            linked_aliases: self.linked_aliases_for(symbol),
            issues: self.rename_preflight_issues(symbol, &new_name),
        }
    }

    fn linked_aliases_for(&self, symbol: SymbolId) -> Vec<LinkedAlias> {
        let mut aliases = Vec::new();

        for import in &self.imports {
            if import
                .module_reference
                .and_then(|reference| self.definition_of(reference))
                == Some(symbol)
                && let Some(alias) = import.alias
            {
                aliases.push(LinkedAlias {
                    kind: LinkedAliasKind::ImportAlias,
                    symbol: self.file_backed_symbol_identity(alias),
                });
            }
        }

        for export in &self.exports {
            if export
                .target_reference
                .and_then(|reference| self.definition_of(reference))
                == Some(symbol)
                && let Some(alias) = export.alias
            {
                aliases.push(LinkedAlias {
                    kind: LinkedAliasKind::ExportAlias,
                    symbol: self.file_backed_symbol_identity(alias),
                });
            }
        }

        aliases
    }

    fn rename_preflight_issues(
        &self,
        symbol: SymbolId,
        new_name: &str,
    ) -> Vec<RenamePreflightIssue> {
        let mut issues = Vec::new();

        if new_name.is_empty() {
            issues.push(RenamePreflightIssue {
                kind: RenamePreflightIssueKind::EmptyName,
                message: "new name must not be empty".to_owned(),
                range: self.symbol(symbol).range,
                related_symbol: None,
            });
            return issues;
        }

        let scope = self.symbol(symbol).scope;
        for other in self.scope(scope).symbols.iter().copied() {
            if other != symbol && self.symbol(other).name == new_name {
                issues.push(RenamePreflightIssue {
                    kind: RenamePreflightIssueKind::DuplicateDefinition,
                    message: format!(
                        "renaming `{}` to `{new_name}` would duplicate a definition in the same scope",
                        self.symbol(symbol).name
                    ),
                    range: self.symbol(symbol).range,
                    related_symbol: Some(self.file_backed_symbol_identity(other)),
                });
                break;
            }
        }

        for reference in self.references_to(symbol) {
            let reference = self.reference(reference);
            if let Some(conflict) =
                self.first_conflicting_visible_symbol(symbol, new_name, reference.range.start())
            {
                issues.push(RenamePreflightIssue {
                    kind: RenamePreflightIssueKind::ReferenceCollision,
                    message: format!(
                        "renaming `{}` to `{new_name}` would change resolution at one or more references",
                        self.symbol(symbol).name
                    ),
                    range: reference.range,
                    related_symbol: Some(self.file_backed_symbol_identity(conflict)),
                });
            }
        }

        issues
    }

    fn first_conflicting_visible_symbol(
        &self,
        target: SymbolId,
        new_name: &str,
        offset: TextSize,
    ) -> Option<SymbolId> {
        let visible = self.visible_symbols_at(offset);
        let target_position = visible.iter().position(|symbol| *symbol == target)?;

        visible[..target_position]
            .iter()
            .copied()
            .find(|symbol| self.symbol(*symbol).name == new_name)
    }
}
