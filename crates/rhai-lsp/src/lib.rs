use lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use rhai_ide::AnalysisHost;

#[derive(Debug, Default)]
pub struct Server {
    analysis_host: AnalysisHost,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn analysis_host(&self) -> &AnalysisHost {
        &self.analysis_host
    }

    pub fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            definition_provider: Some(OneOf::Left(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions::default()),
            ..ServerCapabilities::default()
        }
    }

    pub fn initialize_result(&self) -> InitializeResult {
        InitializeResult {
            capabilities: self.capabilities(),
            server_info: Some(ServerInfo {
                name: "rhai-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        }
    }
}
