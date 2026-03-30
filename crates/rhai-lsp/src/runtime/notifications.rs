use anyhow::{Result, anyhow};
use lsp_server::{Connection, Notification};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    DidRenameFiles, Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, FileChangeType, PublishDiagnosticsParams, RenameFilesParams,
};
use tracing::debug;

use crate::handlers::diagnostics::dedupe_diagnostic_updates;
use crate::protocol::{diagnostic_to_lsp, file_text_by_uri, open_document_text_by_uri};
use crate::state::{DiagnosticUpdate, ServerState};

use super::util::send_notification;

pub(crate) fn handle_notification(
    connection: &Connection,
    server: &mut ServerState,
    notification: Notification,
) -> Result<()> {
    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
            debug!(uri = ?params.text_document.uri, version = params.text_document.version, "opening document");
            let updates = server.open_document(
                params.text_document.uri,
                params.text_document.version,
                params.text_document.text,
            )?;
            publish_diagnostics_updates(connection, server, updates)?;
        }
        DidChangeTextDocument::METHOD => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
            debug!(uri = ?params.text_document.uri, version = params.text_document.version, "changing document");
            let Some(change) = params.content_changes.into_iter().next() else {
                return Ok(());
            };
            let updates = server.change_document(
                params.text_document.uri,
                params.text_document.version,
                change.text,
            )?;
            publish_diagnostics_updates(connection, server, updates)?;
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;
            debug!(uri = ?params.text_document.uri, "closing document");
            let updates = server.close_document(&params.text_document.uri);
            publish_diagnostics_updates(connection, server, updates)?;
        }
        DidChangeWatchedFiles::METHOD => {
            let params: DidChangeWatchedFilesParams = serde_json::from_value(notification.params)?;
            let mut updates = Vec::<DiagnosticUpdate>::new();

            for change in params.changes {
                debug!(uri = ?change.uri, kind = ?change.typ, "workspace file changed");
                let file_updates = match change.typ {
                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                        server.reload_workspace_file(&change.uri)?
                    }
                    FileChangeType::DELETED => server.remove_workspace_file(&change.uri)?,
                    _ => Vec::new(),
                };
                updates.extend(file_updates);
            }

            publish_diagnostics_updates(connection, server, dedupe_diagnostic_updates(updates))?;
        }
        DidRenameFiles::METHOD => {
            let params: RenameFilesParams = serde_json::from_value(notification.params)?;
            let mut updates = Vec::<DiagnosticUpdate>::new();

            for rename in params.files {
                let old_uri = rename.old_uri.parse()?;
                let new_uri = rename.new_uri.parse()?;
                debug!(old_uri = ?old_uri, new_uri = ?new_uri, "workspace file renamed");
                updates.extend(server.rename_workspace_file(&old_uri, &new_uri)?);
            }

            publish_diagnostics_updates(connection, server, dedupe_diagnostic_updates(updates))?;
        }
        _ => {}
    }

    Ok(())
}

pub(crate) fn publish_diagnostics_updates(
    connection: &Connection,
    server: &ServerState,
    updates: Vec<DiagnosticUpdate>,
) -> Result<()> {
    for update in updates {
        debug!(uri = ?update.uri, count = update.diagnostics.len(), "publishing diagnostics");
        let diagnostics = if update.diagnostics.is_empty() {
            Vec::new()
        } else {
            let text = open_document_text_by_uri(server, &update.uri)
                .or_else(|| file_text_by_uri(server, &update.uri))
                .ok_or_else(|| anyhow!("document `{}` is not loaded", update.uri.as_str()))?;
            update
                .diagnostics
                .iter()
                .filter_map(|diagnostic| diagnostic_to_lsp(text.as_ref(), diagnostic))
                .collect()
        };

        send_notification(
            connection,
            PublishDiagnostics::METHOD,
            PublishDiagnosticsParams {
                uri: update.uri,
                diagnostics,
                version: update.version,
            },
        )?;
    }

    Ok(())
}
