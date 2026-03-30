use anyhow::Result;
use lsp_server::Connection;
use lsp_types::notification::{Notification as LspNotification, Progress};
use lsp_types::request::WorkDoneProgressCreate;
use lsp_types::{
    NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressCreateParams, WorkDoneProgressEnd, WorkDoneProgressReport,
};

use crate::state::ServerState;

use super::util::{send_notification, send_request};

pub(crate) struct WorkDoneProgressHandle {
    token: NumberOrString,
}

impl WorkDoneProgressHandle {
    pub(crate) fn begin_workspace_warmup(
        connection: &Connection,
        server: &mut ServerState,
    ) -> Result<Option<Self>> {
        if !server.supports_work_done_progress() {
            return Ok(None);
        }

        let token = NumberOrString::String(format!(
            "rhai/workspace-warmup/{}",
            server.next_server_request_id()
        ));
        send_request::<WorkDoneProgressCreate>(
            connection,
            server.next_server_request_id().into(),
            WorkDoneProgressCreateParams {
                token: token.clone(),
            },
        )?;
        send_notification(
            connection,
            Progress::METHOD,
            ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: "Loading Rhai workspace".to_owned(),
                        cancellable: Some(false),
                        message: Some("Scanning workspace files...".to_owned()),
                        percentage: Some(0),
                    },
                )),
            },
        )?;

        Ok(Some(Self { token }))
    }

    pub(crate) fn report(&self, connection: &Connection, message: impl Into<String>) -> Result<()> {
        send_notification(
            connection,
            Progress::METHOD,
            ProgressParams {
                token: self.token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: Some(false),
                        message: Some(message.into()),
                        percentage: Some(100),
                    },
                )),
            },
        )
    }

    pub(crate) fn end(&self, connection: &Connection, message: impl Into<String>) -> Result<()> {
        send_notification(
            connection,
            Progress::METHOD,
            ProgressParams {
                token: self.token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: Some(message.into()),
                })),
            },
        )
    }
}
