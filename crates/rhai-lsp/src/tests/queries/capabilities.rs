use crate::Server;
use lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionProviderCapability, OneOf,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};

#[test]
fn capabilities_expose_signature_help_inlay_hints_workspace_symbols_and_semantic_tokens() {
    let server = Server::new();
    let capabilities = server.capabilities();

    assert!(matches!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Options(ref options))
            if options.open_close == Some(true)
                && options.change == Some(TextDocumentSyncKind::INCREMENTAL)
    ));
    assert!(matches!(
        capabilities.document_highlight_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.call_hierarchy_provider,
        Some(CallHierarchyServerCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.folding_range_provider,
        Some(lsp_types::FoldingRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.declaration_provider,
        Some(lsp_types::DeclarationCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.type_definition_provider,
        Some(lsp_types::TypeDefinitionProviderCapability::Simple(true))
    ));
    assert!(capabilities.signature_help_provider.is_some());
    assert_eq!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.resolve_provider),
        Some(true)
    );
    assert!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.trigger_characters.as_ref())
            .is_some_and(|triggers| triggers.iter().any(|trigger| trigger == ":"))
    );
    assert!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.trigger_characters.as_ref())
            .is_some_and(|triggers| {
                !triggers
                    .iter()
                    .any(|trigger| trigger == " " || trigger == "a" || trigger == "A")
            }),
        "expected narrowed completion triggers"
    );
    assert!(matches!(
        capabilities.inlay_hint_provider,
        Some(OneOf::Right(_))
    ));
    assert!(matches!(
        capabilities.workspace_symbol_provider,
        Some(OneOf::Left(true))
    ));
    assert!(
        capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_folders.as_ref())
            .is_some_and(|folders| {
                folders.supported == Some(true)
                    && folders.change_notifications == Some(OneOf::Left(true))
            }),
        "expected workspace folder support in workspace capabilities"
    );
    assert!(
        capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.file_operations.as_ref())
            .and_then(|operations| operations.did_rename.as_ref())
            .is_some(),
        "expected didRenameFiles registration in workspace capabilities"
    );
    assert!(matches!(
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(ref options))
            if options.range == Some(true)
                && matches!(
                    options.full,
                    Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
                )
    ));
    assert!(capabilities.document_formatting_provider.is_some());
    assert!(capabilities.document_range_formatting_provider.is_some());
    assert!(
        capabilities
            .document_on_type_formatting_provider
            .as_ref()
            .is_some_and(|options| {
                options.first_trigger_character == ";"
                    && options
                        .more_trigger_character
                        .as_ref()
                        .is_some_and(|chars| chars.iter().any(|ch| ch == "}"))
            }),
        "expected on-type formatting triggers for ';' and '}}'"
    );
    assert!(matches!(
        capabilities.selection_range_provider,
        Some(SelectionRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.linked_editing_range_provider,
        Some(lsp_types::LinkedEditingRangeServerCapabilities::Simple(
            true
        ))
    ));
    assert!(matches!(
        capabilities.code_action_provider,
        Some(CodeActionProviderCapability::Options(ref options))
            if options.resolve_provider == Some(true)
                && options
                    .code_action_kinds
                    .as_ref()
                    .is_some_and(|kinds| kinds.contains(&CodeActionKind::SOURCE_ORGANIZE_IMPORTS))
    ));
    assert!(matches!(
        capabilities.rename_provider,
        Some(OneOf::Right(ref options)) if options.prepare_provider == Some(true)
    ));
}
