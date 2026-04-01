use anyhow::{Result, anyhow};
use lsp_server::{Connection, Notification};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles,
    DidChangeWorkspaceFolders, DidCloseTextDocument, DidOpenTextDocument, DidRenameFiles,
    Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    FileChangeType, PublishDiagnosticsParams, RenameFilesParams, TextDocumentContentChangeEvent,
};
use tracing::debug;

use crate::handlers::diagnostics::dedupe_diagnostic_updates;
use crate::protocol::{diagnostic_to_lsp, file_text_by_uri, open_document_text_by_uri};
use crate::state::{DiagnosticUpdate, ServerState};

use super::stdio::server_settings_from_value;
use super::util::position_to_offset_in_text;
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
            let Some(document) = server.open_documents.get(&params.text_document.uri) else {
                return Err(anyhow!(
                    "document `{}` is not open",
                    params.text_document.uri.as_str()
                ));
            };
            let next_text = apply_content_changes(&document.text, &params.content_changes)?;
            if params.content_changes.is_empty() {
                return Ok(());
            }
            let updates = server.change_document(
                params.text_document.uri,
                params.text_document.version,
                next_text,
            )?;
            publish_diagnostics_updates(connection, server, updates)?;
        }
        DidChangeConfiguration::METHOD => {
            let params: DidChangeConfigurationParams = serde_json::from_value(notification.params)?;
            debug!("updating server settings");
            server.configure_settings(server_settings_from_value(&params.settings));
        }
        DidChangeWorkspaceFolders::METHOD => {
            let params: DidChangeWorkspaceFoldersParams =
                serde_json::from_value(notification.params)?;
            let added = params
                .event
                .added
                .iter()
                .filter_map(|folder| crate::state::path_from_uri(&folder.uri).ok())
                .collect::<Vec<_>>();
            let removed = params
                .event
                .removed
                .iter()
                .filter_map(|folder| crate::state::path_from_uri(&folder.uri).ok())
                .collect::<Vec<_>>();
            debug!(added = ?added, removed = ?removed, "updating workspace folders");
            let load = server.update_workspace_folders(&added, &removed)?;
            publish_diagnostics_updates(connection, server, load.updates)?;
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

fn apply_content_changes(
    current_text: &str,
    content_changes: &[TextDocumentContentChangeEvent],
) -> Result<String> {
    let mut text = current_text.to_owned();

    for change in content_changes {
        match change.range {
            Some(range) => {
                let start = position_to_offset_in_text(&text, range.start)
                    .ok_or_else(|| anyhow!("change start is outside the current document"))?;
                let end = position_to_offset_in_text(&text, range.end)
                    .ok_or_else(|| anyhow!("change end is outside the current document"))?;
                if start > end || end > text.len() {
                    return Err(anyhow!("change range is invalid for the current document"));
                }
                text.replace_range(start..end, &change.text);
            }
            None => text = change.text.clone(),
        }
    }

    Ok(text)
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

#[cfg(test)]
mod tests {
    use lsp_server::{Connection, Notification};
    use lsp_types::notification::{
        DidChangeConfiguration, DidChangeWorkspaceFolders, Notification as LspNotification,
    };
    use lsp_types::{
        DidChangeConfigurationParams, DidChangeWorkspaceFoldersParams, Position, Range,
        TextDocumentContentChangeEvent, WorkspaceFolder, WorkspaceFoldersChangeEvent,
    };
    use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::runtime::notifications::{apply_content_changes, handle_notification};
    use crate::state::{ServerSettings, ServerState, uri_from_path};

    #[test]
    fn apply_content_changes_handles_incremental_edits() {
        let text = "let value = 1;\n";
        let changed = apply_content_changes(
            text,
            &[
                TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position {
                            line: 0,
                            character: 12,
                        },
                        end: Position {
                            line: 0,
                            character: 13,
                        },
                    }),
                    range_length: None,
                    text: "2".to_owned(),
                },
                TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position {
                            line: 0,
                            character: 13,
                        },
                        end: Position {
                            line: 0,
                            character: 13,
                        },
                    }),
                    range_length: None,
                    text: " + 3".to_owned(),
                },
            ],
        )
        .expect("expected content changes to apply");

        assert_eq!(changed, "let value = 2 + 3;\n");
    }

    #[test]
    fn did_change_configuration_updates_server_settings() {
        let (connection, _client) = Connection::memory();
        let mut server = ServerState::new();

        handle_notification(
            &connection,
            &mut server,
            Notification::new(
                DidChangeConfiguration::METHOD.to_owned(),
                DidChangeConfigurationParams {
                    settings: json!({
                        "rhai": {
                            "inlayHints": {
                                "variables": false,
                                "parameters": false,
                                "returnTypes": true
                            },
                            "formatting": {
                                "maxLineLength": 88,
                                "trailingCommas": false,
                                "finalNewline": false,
                                "containerLayout": "preferMultiLine",
                                "importSortOrder": "modulePath"
                            }
                        }
                    }),
                },
            ),
        )
        .expect("expected configuration change to succeed");

        assert_eq!(
            server.settings(),
            ServerSettings {
                inlay_hints: crate::state::InlayHintSettings {
                    variables: false,
                    parameters: false,
                    return_types: true,
                },
                formatter: crate::state::FormatterSettings {
                    max_line_length: 88,
                    trailing_commas: false,
                    final_newline: false,
                    container_layout: ContainerLayoutStyle::PreferMultiLine,
                    import_sort_order: ImportSortOrder::ModulePath,
                },
            }
        );
    }

    #[test]
    fn did_change_workspace_folders_updates_server_roots() {
        let (connection, _client) = Connection::memory();
        let mut server = ServerState::new();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("expected system time")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("rhai-lsp-workspace-folders-{unique}"));
        let first = base.join("first");
        let second = base.join("second");
        fs::create_dir_all(&first).expect("expected first workspace");
        fs::create_dir_all(&second).expect("expected second workspace");

        server
            .load_workspace_roots(std::slice::from_ref(&first))
            .expect("expected initial workspace load");

        handle_notification(
            &connection,
            &mut server,
            Notification::new(
                DidChangeWorkspaceFolders::METHOD.to_owned(),
                DidChangeWorkspaceFoldersParams {
                    event: WorkspaceFoldersChangeEvent {
                        added: vec![WorkspaceFolder {
                            uri: uri_from_path(&second).expect("expected second uri"),
                            name: "second".to_owned(),
                        }],
                        removed: vec![WorkspaceFolder {
                            uri: uri_from_path(&first).expect("expected first uri"),
                            name: "first".to_owned(),
                        }],
                    },
                },
            ),
        )
        .expect("expected workspace folder change to succeed");

        assert_eq!(server.workspace_roots, vec![second]);

        let _ = fs::remove_dir_all(&base);
    }
}
