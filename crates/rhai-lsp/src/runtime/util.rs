use anyhow::{Result, anyhow};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::request::Request as LspRequest;
use lsp_types::{TextDocumentPositionParams, Uri};
use serde::Serialize;

use crate::protocol::open_document_text_by_uri;
use crate::state::ServerState;

pub(crate) fn with_text_document_position<T>(
    server: &ServerState,
    params: TextDocumentPositionParams,
    f: impl FnOnce(Uri, u32) -> Result<T>,
) -> Result<T> {
    let offset = position_to_offset_for_uri(server, &params.text_document.uri, params.position)?;
    f(params.text_document.uri, offset)
}

pub(crate) fn file_id_for_uri(server: &ServerState, uri: &Uri) -> Result<rhai_vfs::FileId> {
    let document = server
        .open_documents
        .get(uri)
        .ok_or_else(|| anyhow!("document `{}` is not open", uri.as_str()))?;
    server
        .analysis_host()
        .snapshot()
        .file_id_for_path(&document.normalized_path)
        .ok_or_else(|| {
            anyhow!(
                "document `{}` is not loaded in the analysis host",
                uri.as_str()
            )
        })
}

pub(crate) fn position_to_offset_for_uri(
    server: &ServerState,
    uri: &Uri,
    position: lsp_types::Position,
) -> Result<u32> {
    let text = open_document_text_by_uri(server, uri)
        .ok_or_else(|| anyhow!("document `{}` is not open", uri.as_str()))?;
    let offset = position_to_offset_in_text(text.as_ref(), position)
        .ok_or_else(|| anyhow!("position is outside document `{}`", uri.as_str()))?;
    u32::try_from(offset).map_err(|_| anyhow!("document offset does not fit in u32"))
}

pub(crate) fn position_to_offset_in_text(
    text: &str,
    position: lsp_types::Position,
) -> Option<usize> {
    let line_starts = line_start_offsets(text);
    let line_start = *line_starts.get(position.line as usize)?;
    let line_end = line_starts
        .get(position.line as usize + 1)
        .copied()
        .unwrap_or(text.len());
    let line_text = text.get(line_start..line_end)?;

    let mut utf16_units = 0_u32;
    for (byte_offset, ch) in line_text.char_indices() {
        if utf16_units == position.character {
            return Some(line_start + byte_offset);
        }
        utf16_units += ch.len_utf16() as u32;
        if utf16_units > position.character {
            return None;
        }
    }

    (utf16_units == position.character).then_some(line_start + line_text.len())
}

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (offset, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(offset + ch.len_utf8());
        }
    }
    starts
}

pub(crate) fn send_ok<T: Serialize>(
    connection: &Connection,
    id: RequestId,
    result: T,
) -> Result<()> {
    connection.sender.send(Message::Response(Response::new_ok(
        id,
        serde_json::to_value(result)?,
    )))?;
    Ok(())
}

pub(crate) fn send_error(
    connection: &Connection,
    id: RequestId,
    code: ErrorCode,
    message: impl Into<String>,
) -> Result<()> {
    connection.sender.send(Message::Response(Response::new_err(
        id,
        code as i32,
        message.into(),
    )))?;
    Ok(())
}

pub(crate) fn send_notification<T: Serialize>(
    connection: &Connection,
    method: &str,
    params: T,
) -> Result<()> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            method.to_owned(),
            params,
        )))?;
    Ok(())
}

pub(crate) fn send_request<R: LspRequest>(
    connection: &Connection,
    id: RequestId,
    params: R::Params,
) -> Result<()>
where
    R::Params: Serialize,
{
    connection.sender.send(Message::Request(Request::new(
        id,
        R::METHOD.to_owned(),
        serde_json::to_value(params)?,
    )))?;
    Ok(())
}
