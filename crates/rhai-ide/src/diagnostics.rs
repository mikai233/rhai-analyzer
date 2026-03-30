use rhai_db::DatabaseSnapshot;
use rhai_vfs::FileId;

use crate::support::convert::{document_symbol_from_db, workspace_symbol_from_db};
use crate::{Diagnostic, DocumentSymbol, WorkspaceSymbol};

pub(crate) fn diagnostics(snapshot: &DatabaseSnapshot, file_id: FileId) -> Vec<Diagnostic> {
    if snapshot.file_text(file_id).is_none() {
        return Vec::new();
    }

    snapshot
        .project_diagnostics(file_id)
        .into_iter()
        .map(|diagnostic| Diagnostic {
            message: diagnostic.message,
            range: diagnostic.range,
        })
        .collect()
}

pub(crate) fn document_symbols(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
) -> Vec<DocumentSymbol> {
    snapshot
        .document_symbols(file_id)
        .iter()
        .map(document_symbol_from_db)
        .collect()
}

pub(crate) fn workspace_symbols(snapshot: &DatabaseSnapshot) -> Vec<WorkspaceSymbol> {
    snapshot
        .workspace_symbols()
        .iter()
        .map(workspace_symbol_from_db)
        .collect()
}

pub(crate) fn workspace_symbols_matching(
    snapshot: &DatabaseSnapshot,
    query: &str,
) -> Vec<WorkspaceSymbol> {
    snapshot
        .workspace_symbols_matching(query)
        .iter()
        .map(workspace_symbol_from_db)
        .collect()
}
