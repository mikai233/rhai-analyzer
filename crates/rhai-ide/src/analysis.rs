use std::path::Path;

use rhai_db::{AnalyzerDatabase, ChangeImpact, ChangeSet, DatabaseSnapshot};
use rhai_hir::Symbol;
use rhai_syntax::TextRange;
use rhai_vfs::FileId;

use crate::TextEdit;
use crate::assists::{Assist, DiagnosticWithFixes, assists_for_range, diagnostics_with_fixes};
use crate::completion::completions;
use crate::diagnostics::{
    diagnostics, document_symbols, workspace_symbols, workspace_symbols_matching,
};
use crate::hints::inlay_hints::inlay_hints;
use crate::hints::signature_help::signature_help;
use crate::hover::hover;
use crate::imports::{organize_imports, remove_unused_imports};
use crate::navigation::rename::{PreparedRename, prepare_rename, rename_plan_from_db};
use crate::support::convert::{
    navigation_target_from_db, navigation_target_from_identity, reference_location_from_db,
    text_size,
};
use crate::{
    AutoImportAction, CompletionItem, Diagnostic, DocumentSymbol, FilePosition, HoverResult,
    InlayHint, NavigationTarget, ReferencesResult, RenamePlan, SignatureHelp, SourceChange,
    WorkspaceSymbol,
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
        diagnostics(&self.db, file_id)
    }

    pub fn diagnostics_with_fixes(&self, file_id: FileId) -> Vec<DiagnosticWithFixes> {
        diagnostics_with_fixes(&self.db, file_id)
    }

    pub fn hover(&self, position: FilePosition) -> Option<HoverResult> {
        hover(&self.db, position)
    }

    pub fn signature_help(&self, position: FilePosition) -> Option<SignatureHelp> {
        signature_help(&self.db, position.file_id, text_size(position.offset))
    }

    pub fn inlay_hints(&self, file_id: FileId) -> Vec<InlayHint> {
        inlay_hints(&self.db, file_id)
    }

    pub fn symbols(&self, file_id: FileId) -> Vec<Symbol> {
        self.db
            .hir(file_id)
            .map_or_else(Vec::new, |hir| hir.symbols.clone())
    }

    pub fn document_symbols(&self, file_id: FileId) -> Vec<DocumentSymbol> {
        document_symbols(&self.db, file_id)
    }

    pub fn workspace_symbols(&self) -> Vec<WorkspaceSymbol> {
        workspace_symbols(&self.db)
    }

    pub fn workspace_symbols_matching(&self, query: &str) -> Vec<WorkspaceSymbol> {
        workspace_symbols_matching(&self.db, query)
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
        completions(&self.db, position)
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
