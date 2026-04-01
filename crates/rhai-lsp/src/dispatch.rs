use lsp_types::{
    CallHierarchyServerCapability, CodeActionOptions, CodeActionProviderCapability,
    CompletionOptions, DeclarationCapability, DocumentFormattingOptions,
    DocumentOnTypeFormattingOptions, DocumentRangeFormattingOptions, FileOperationFilter,
    FileOperationPattern, FileOperationPatternKind, FileOperationRegistrationOptions,
    FoldingRangeProviderCapability, HoverProviderCapability, InitializeResult, InlayHintOptions,
    InlayHintServerCapabilities, LinkedEditingRangeServerCapabilities, OneOf, RenameOptions,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelpOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TypeDefinitionProviderCapability, WorkspaceFileOperationsServerCapabilities,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};

use crate::handlers::queries::semantic_token_legend;
use crate::state::ServerState;

impl ServerState {
    pub fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    will_save: None,
                    will_save_wait_until: None,
                    save: None,
                },
            )),
            declaration_provider: Some(DeclarationCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: Default::default(),
            })),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            document_highlight_provider: Some(OneOf::Left(true)),
            call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(completion_trigger_characters()),
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
                    range: Some(true),
                    full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
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
            document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                first_trigger_character: ";".to_owned(),
                more_trigger_character: Some(vec!["}".to_owned()]),
            }),
            selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
            linked_editing_range_provider: Some(LinkedEditingRangeServerCapabilities::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![
                    lsp_types::CodeActionKind::QUICKFIX,
                    lsp_types::CodeActionKind::REFACTOR,
                    lsp_types::CodeActionKind::SOURCE,
                    lsp_types::CodeActionKind::SOURCE_FIX_ALL,
                    lsp_types::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                ]),
                resolve_provider: Some(true),
                work_done_progress_options: Default::default(),
            })),
            workspace: Some(WorkspaceServerCapabilities {
                workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    change_notifications: Some(OneOf::Left(true)),
                }),
                file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                    did_create: None,
                    will_create: None,
                    did_rename: Some(rhai_file_operation_registration()),
                    will_rename: None,
                    did_delete: None,
                    will_delete: None,
                }),
            }),
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

fn completion_trigger_characters() -> Vec<String> {
    vec![".".to_owned(), ":".to_owned(), "@".to_owned()]
}

fn rhai_file_operation_registration() -> FileOperationRegistrationOptions {
    FileOperationRegistrationOptions {
        filters: vec![FileOperationFilter {
            scheme: Some("file".to_owned()),
            pattern: FileOperationPattern {
                glob: "**/*.rhai".to_owned(),
                matches: Some(FileOperationPatternKind::File),
                options: None,
            },
        }],
    }
}
