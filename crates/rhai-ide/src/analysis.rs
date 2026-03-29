use std::path::Path;

use rhai_db::{AnalyzerDatabase, ChangeImpact, ChangeSet, DatabaseSnapshot};
use rhai_hir::{CompletionSymbol, Symbol, TypeRef};
use rhai_syntax::TextRange;
use rhai_vfs::FileId;

use crate::TextEdit;
use crate::assists::{Assist, DiagnosticWithFixes, assists_for_range, diagnostics_with_fixes};
use crate::convert::{
    document_symbol_from_db, format_symbol_signature, format_type_ref, navigation_target_from_db,
    navigation_target_from_identity, reference_location_from_db, text_size,
    workspace_symbol_from_db,
};
use crate::imports::{organize_imports, remove_unused_imports};
use crate::rename::{PreparedRename, prepare_rename, rename_plan_from_db};
use crate::signature_help::signature_help;
use crate::{
    AutoImportAction, CompletionItem, CompletionItemKind, CompletionItemSource, Diagnostic,
    DocumentSymbol, FilePosition, HoverResult, NavigationTarget, ReferencesResult, RenamePlan,
    SignatureHelp, SourceChange, WorkspaceSymbol,
};

#[derive(Debug, Default)]
pub struct AnalysisHost {
    db: AnalyzerDatabase,
}

impl AnalysisHost {
    pub fn apply_change(&mut self, change_set: ChangeSet) {
        self.db.apply_change(change_set);
    }

    pub fn apply_change_report(&mut self, change_set: ChangeSet) -> ChangeImpact {
        self.db.apply_change_report(change_set)
    }

    pub fn warm_query_support(&mut self, file_ids: &[FileId]) -> usize {
        self.db.warm_query_support(file_ids)
    }

    pub fn set_query_support_budget(&mut self, budget: Option<usize>) -> Vec<FileId> {
        self.db.set_query_support_budget(budget)
    }

    pub fn snapshot(&self) -> Analysis {
        Analysis {
            db: self.db.snapshot(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub(crate) db: DatabaseSnapshot,
}

impl Analysis {
    pub fn file_id_for_path(&self, path: &Path) -> Option<FileId> {
        self.db.vfs().file_id(path)
    }

    pub fn normalized_path(&self, file_id: FileId) -> Option<&Path> {
        self.db.normalized_path(file_id)
    }

    pub fn has_query_support(&self, file_id: FileId) -> bool {
        self.db.query_support(file_id).is_some()
    }

    pub fn diagnostics(&self, file_id: FileId) -> Vec<Diagnostic> {
        if self.db.file_text(file_id).is_none() {
            return Vec::new();
        }

        self.db
            .project_diagnostics(file_id)
            .into_iter()
            .map(|diagnostic| Diagnostic {
                message: diagnostic.message,
                range: diagnostic.range,
            })
            .collect()
    }

    pub fn diagnostics_with_fixes(&self, file_id: FileId) -> Vec<DiagnosticWithFixes> {
        diagnostics_with_fixes(&self.db, file_id)
    }

    pub fn hover(&self, position: FilePosition) -> Option<HoverResult> {
        let (file_id, symbol_id) =
            if let Some(target) = self.goto_definition(position).into_iter().next() {
                let hir = self.db.hir(target.file_id)?;
                (target.file_id, hir.symbol_at(target.full_range)?)
            } else {
                let hir = self.db.hir(position.file_id)?;
                (
                    position.file_id,
                    hir.definition_at_offset(text_size(position.offset))?,
                )
            };
        let hir = self.db.hir(file_id)?;
        let symbol = hir.symbol(symbol_id);
        let docs = symbol.docs.map(|docs| hir.doc_block(docs).text.clone());
        let annotation = self
            .db
            .inferred_symbol_type(file_id, symbol_id)
            .or(symbol.annotation.as_ref());

        Some(HoverResult {
            signature: format_symbol_signature(symbol.name.as_str(), symbol.kind, annotation),
            docs,
        })
    }

    pub fn signature_help(&self, position: FilePosition) -> Option<SignatureHelp> {
        signature_help(&self.db, position.file_id, text_size(position.offset))
    }

    pub fn symbols(&self, file_id: FileId) -> Vec<Symbol> {
        self.db
            .hir(file_id)
            .map_or_else(Vec::new, |hir| hir.symbols.clone())
    }

    pub fn document_symbols(&self, file_id: FileId) -> Vec<DocumentSymbol> {
        self.db
            .document_symbols(file_id)
            .iter()
            .map(document_symbol_from_db)
            .collect()
    }

    pub fn workspace_symbols(&self) -> Vec<WorkspaceSymbol> {
        self.db
            .workspace_symbols()
            .iter()
            .map(workspace_symbol_from_db)
            .collect()
    }

    pub fn workspace_symbols_matching(&self, query: &str) -> Vec<WorkspaceSymbol> {
        self.db
            .workspace_symbols_matching(query)
            .iter()
            .map(workspace_symbol_from_db)
            .collect()
    }

    pub fn goto_definition(&self, position: FilePosition) -> Vec<NavigationTarget> {
        self.db
            .goto_definition(position.file_id, text_size(position.offset))
            .into_iter()
            .map(navigation_target_from_db)
            .collect()
    }

    pub fn find_references(&self, position: FilePosition) -> Option<ReferencesResult> {
        let result = self
            .db
            .find_references(position.file_id, text_size(position.offset))?;

        Some(ReferencesResult {
            targets: result
                .targets
                .iter()
                .map(navigation_target_from_identity)
                .collect(),
            references: result
                .references
                .iter()
                .map(reference_location_from_db)
                .collect(),
        })
    }

    pub fn rename_plan(
        &self,
        position: FilePosition,
        new_name: impl Into<String>,
    ) -> Option<RenamePlan> {
        let plan = self
            .db
            .rename_plan(position.file_id, text_size(position.offset), new_name)?;

        Some(rename_plan_from_db(&plan))
    }

    pub fn rename(
        &self,
        position: FilePosition,
        new_name: impl Into<String>,
    ) -> Option<PreparedRename> {
        prepare_rename(&self.db, position, new_name)
    }

    pub fn completions(&self, position: FilePosition) -> Vec<CompletionItem> {
        let Some(inputs) = self
            .db
            .completion_inputs(position.file_id, text_size(position.offset))
        else {
            return Vec::new();
        };

        let mut items = Vec::new();
        let hir = self.db.hir(position.file_id);

        items.extend(inputs.visible_symbols.iter().map(|symbol| {
            let docs = match (&hir, symbol.docs) {
                (Some(hir), Some(docs)) => Some(hir.doc_block(docs).text.clone()),
                _ => None,
            };

            CompletionItem {
                label: symbol.name.clone(),
                kind: CompletionItemKind::Symbol(symbol.kind),
                source: CompletionItemSource::Visible,
                detail: completion_detail(&self.db, position.file_id, symbol),
                docs,
                file_id: Some(position.file_id),
                exported: false,
            }
        }));

        items.extend(inputs.project_symbols.iter().map(|symbol| CompletionItem {
            label: symbol.symbol.name.clone(),
            kind: CompletionItemKind::Symbol(symbol.symbol.kind),
            source: CompletionItemSource::Project,
            detail: None,
            docs: None,
            file_id: Some(symbol.file_id),
            exported: symbol.symbol.exported,
        }));

        items.extend(inputs.member_symbols.iter().map(|member| CompletionItem {
            label: member.name.clone(),
            kind: CompletionItemKind::Member,
            source: CompletionItemSource::Member,
            detail: member.annotation.as_ref().map(format_type_ref),
            docs: None,
            file_id: None,
            exported: false,
        }));

        items
    }

    pub fn auto_import_actions(&self, position: FilePosition) -> Vec<AutoImportAction> {
        self.db
            .auto_import_candidates(position.file_id, text_size(position.offset))
            .into_iter()
            .map(|candidate| {
                let module_name = candidate.module_name;

                AutoImportAction {
                    label: format!("Import `{module_name}`"),
                    module_name,
                    provider_file_id: candidate.provider_file_id,
                    source_change: SourceChange::from_text_edit(
                        position.file_id,
                        TextEdit::insert(candidate.insertion_offset, candidate.insert_text),
                    ),
                }
            })
            .collect()
    }

    pub fn assists(&self, file_id: FileId, range: TextRange) -> Vec<Assist> {
        assists_for_range(&self.db, file_id, range)
    }

    pub fn remove_unused_imports(&self, file_id: FileId) -> Option<SourceChange> {
        remove_unused_imports(&self.db, file_id)
    }

    pub fn organize_imports(&self, file_id: FileId) -> Option<SourceChange> {
        organize_imports(&self.db, file_id)
    }
}

fn completion_detail(
    db: &DatabaseSnapshot,
    file_id: FileId,
    symbol: &CompletionSymbol,
) -> Option<String> {
    symbol
        .annotation
        .as_ref()
        .or_else(|| inferred_completion_type(db, file_id, symbol.symbol))
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
        .map(format_type_ref)
}

fn inferred_completion_type(
    db: &DatabaseSnapshot,
    file_id: FileId,
    symbol: rhai_hir::SymbolId,
) -> Option<&TypeRef> {
    db.inferred_symbol_type(file_id, symbol)
        .filter(|ty| !matches!(ty, TypeRef::Unknown))
}
