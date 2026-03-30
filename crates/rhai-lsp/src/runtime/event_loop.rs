use anyhow::Result;
use lsp_server::{Connection, Message};
use tracing::{debug, trace};

use crate::state::ServerState;

use super::notifications::handle_notification;
use super::requests::handle_request;

pub(crate) fn event_loop(connection: &Connection, server: &mut ServerState) -> Result<()> {
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                debug!(method = %request.method, "received lsp request");
                if connection.handle_shutdown(&request)? {
                    debug!("received shutdown request");
                    break;
                }
                handle_request(connection, server, request)?;
            }
            Message::Notification(notification) => {
                debug!(method = %notification.method, "received lsp notification");
                handle_notification(connection, server, notification)?;
            }
            Message::Response(response) => {
                trace!(id = ?response.id, "received lsp response");
            }
        }
    }

    Ok(())
}
