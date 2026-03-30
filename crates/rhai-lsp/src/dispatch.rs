use lsp_types::{
    CallHierarchyServerCapability, CodeActionProviderCapability, CompletionOptions,
    DocumentFormattingOptions, DocumentRangeFormattingOptions, FoldingRangeProviderCapability,
    HoverProviderCapability, InitializeResult, InlayHintOptions, InlayHintServerCapabilities,
    OneOf, SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, SignatureHelpOptions, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};

use crate::handlers::queries::semantic_token_legend;
use crate::server::Server;

impl Server {
    pub fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Left(true)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![".".to_owned()]),
                resolve_provider: Some(true),
                ..CompletionOptions::default()
            }),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                retrigger_characters: Some(vec![",".to_owned()]),
                ..SignatureHelpOptions::default()
            }),
            inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                InlayHintOptions::default(),
            ))),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: semantic_token_legend(),
                    range: None,
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    work_done_progress_options: Default::default(),
                }),
            ),
            document_formatting_provider: Some(OneOf::Right(DocumentFormattingOptions {
                work_done_progress_options: Default::default(),
            })),
            document_range_formatting_provider: Some(OneOf::Right(
                DocumentRangeFormattingOptions {
                    work_done_progress_options: Default::default(),
                },
            )),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
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
