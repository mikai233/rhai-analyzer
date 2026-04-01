use anyhow::{Result, anyhow};
use lsp_server::{Connection, ErrorCode, Request};
use lsp_types::request::{
    CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
    CodeActionRequest, CodeActionResolveRequest, Completion, DocumentHighlightRequest,
    DocumentSymbolRequest, FoldingRangeRequest, Formatting, GotoDeclaration, GotoDefinition,
    GotoTypeDefinition, HoverRequest, InlayHintRequest, LinkedEditingRange, OnTypeFormatting,
    PrepareRenameRequest, RangeFormatting, References, Rename, Request as LspRequest,
    ResolveCompletionItem, SelectionRangeRequest, SemanticTokensFullDeltaRequest,
    SemanticTokensFullRequest, SemanticTokensRangeRequest, SignatureHelpRequest,
    WorkspaceSymbolRequest,
};
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionParams, CodeActionResponse, CompletionParams, CompletionResponse,
    DocumentHighlightParams, DocumentOnTypeFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, FoldingRangeParams, GotoDefinitionParams, HoverParams, InlayHintParams,
    LinkedEditingRangeParams, ReferenceParams, RenameParams, SelectionRangeParams,
    SemanticTokensDeltaParams, SemanticTokensParams, SemanticTokensRangeParams,
    SignatureHelpParams, WorkspaceEdit, WorkspaceSymbolParams,
};

use crate::protocol::{
    call_hierarchy_item_from_lsp, call_hierarchy_item_to_lsp, completion_item_from_lsp,
    completion_item_to_lsp, document_highlight_to_lsp, document_symbols_to_lsp,
    goto_definition_response, hover_to_lsp, incoming_call_to_lsp, inlay_hint_to_lsp,
    open_document_text_by_uri, outgoing_call_to_lsp, prepared_rename_to_lsp, references_to_lsp,
    rename_to_workspace_edit, resolve_code_action_payload, resolved_code_action_to_lsp,
    semantic_tokens_delta_result, semantic_tokens_result, signature_help_to_lsp,
    unresolved_code_action_to_lsp, workspace_symbols_to_lsp,
};
use crate::state::ServerState;

use super::util::{file_id_for_uri, send_error, send_ok, with_text_document_position};

pub(crate) fn handle_request(
    connection: &Connection,
    server: &mut ServerState,
    request: Request,
) -> Result<()> {
    match request.method.as_str() {
        HoverRequest::METHOD => {
            let params: HoverParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.hover(&uri, offset),
            )?
            .map(hover_to_lsp);
            send_ok(connection, request.id, result)?;
        }
        GotoDefinition::METHOD => {
            let params: GotoDefinitionParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.goto_definition(&uri, offset),
            )?;
            send_ok(
                connection,
                request.id,
                goto_definition_response(server, result),
            )?;
        }
        GotoTypeDefinition::METHOD => {
            let params: GotoDefinitionParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.goto_type_definition(&uri, offset),
            )?;
            send_ok(
                connection,
                request.id,
                goto_definition_response(server, result),
            )?;
        }
        GotoDeclaration::METHOD => {
            let params: GotoDefinitionParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.goto_declaration(&uri, offset),
            )?;
            send_ok(
                connection,
                request.id,
                goto_definition_response(server, result),
            )?;
        }
        References::METHOD => {
            let params: ReferenceParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position,
                |uri, offset| server.find_references(&uri, offset),
            )?;
            let locations = result.map(|references| {
                references_to_lsp(server, references, params.context.include_declaration)
            });
            send_ok(connection, request.id, locations)?;
        }
        DocumentSymbolRequest::METHOD => {
            let params: DocumentSymbolParams = serde_json::from_value(request.params)?;
            let symbols = server.document_symbols(&params.text_document.uri)?;
            let file_id = file_id_for_uri(server, &params.text_document.uri)?;
            let result = document_symbols_to_lsp(server, file_id, symbols)
                .map(DocumentSymbolResponse::Nested);
            send_ok(connection, request.id, result)?;
        }
        WorkspaceSymbolRequest::METHOD => {
            let params: WorkspaceSymbolParams = serde_json::from_value(request.params)?;
            let symbols = server.workspace_symbols(&params.query)?;
            send_ok(
                connection,
                request.id,
                workspace_symbols_to_lsp(server, symbols),
            )?;
        }
        Completion::METHOD => {
            let params: CompletionParams = serde_json::from_value(request.params)?;
            let completion_position = params.text_document_position.clone();
            let text = open_document_text_by_uri(server, &completion_position.text_document.uri);
            let items =
                with_text_document_position(server, completion_position.clone(), |uri, offset| {
                    server.completions(&uri, offset)
                })?;
            let response = CompletionResponse::List(lsp_types::CompletionList {
                is_incomplete: true,
                items: items
                    .into_iter()
                    .map(|item| completion_item_to_lsp(text.as_deref(), item))
                    .collect(),
            });
            send_ok(connection, request.id, Some(response))?;
        }
        ResolveCompletionItem::METHOD => {
            let params: lsp_types::CompletionItem = serde_json::from_value(request.params)?;
            let resolved = match completion_item_from_lsp(params.clone()) {
                Some(item) => {
                    let text = item.resolve_data.as_ref().and_then(|resolve_data| {
                        server
                            .analysis_host()
                            .snapshot()
                            .file_text(resolve_data.file_id)
                    });
                    completion_item_to_lsp(text.as_deref(), server.resolve_completion(item))
                }
                None => params,
            };
            send_ok(connection, request.id, resolved)?;
        }
        SignatureHelpRequest::METHOD => {
            let params: SignatureHelpParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.signature_help(&uri, offset),
            )?
            .map(signature_help_to_lsp);
            send_ok(connection, request.id, result)?;
        }
        InlayHintRequest::METHOD => {
            let params: InlayHintParams = serde_json::from_value(request.params)?;
            let hints = server.inlay_hints(&params.text_document.uri, Some(params.range))?;
            let text =
                open_document_text_by_uri(server, &params.text_document.uri).ok_or_else(|| {
                    anyhow!(
                        "document `{}` is not open",
                        params.text_document.uri.as_str()
                    )
                })?;
            let result = hints
                .iter()
                .filter_map(|hint| inlay_hint_to_lsp(text.as_ref(), hint))
                .collect::<Vec<_>>();
            send_ok(connection, request.id, Some(result))?;
        }
        DocumentHighlightRequest::METHOD => {
            let params: DocumentHighlightParams = serde_json::from_value(request.params)?;
            let uri = params
                .text_document_position_params
                .text_document
                .uri
                .clone();
            let highlights = with_text_document_position(
                server,
                params.text_document_position_params,
                |query_uri, offset| server.document_highlights(&query_uri, offset),
            )?;
            let text = open_document_text_by_uri(server, &uri)
                .ok_or_else(|| anyhow!("document `{}` is not open", uri.as_str()))?;
            let result = highlights
                .iter()
                .filter_map(|highlight| document_highlight_to_lsp(text.as_ref(), highlight))
                .collect::<Vec<_>>();
            send_ok(connection, request.id, Some(result))?;
        }
        CallHierarchyPrepare::METHOD => {
            let params: CallHierarchyPrepareParams = serde_json::from_value(request.params)?;
            let items = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.prepare_call_hierarchy(&uri, offset),
            )?;
            let result = items
                .iter()
                .filter_map(|item| call_hierarchy_item_to_lsp(server, item))
                .collect::<Vec<_>>();
            send_ok(connection, request.id, Some(result))?;
        }
        CallHierarchyIncomingCalls::METHOD => {
            let params: CallHierarchyIncomingCallsParams = serde_json::from_value(request.params)?;
            let item = call_hierarchy_item_from_lsp(&params.item)
                .ok_or_else(|| anyhow!("missing call hierarchy item payload"))?;
            let calls = server.incoming_calls(&item)?;
            let result = calls
                .iter()
                .filter_map(|call| incoming_call_to_lsp(server, call))
                .collect::<Vec<_>>();
            send_ok(connection, request.id, Some(result))?;
        }
        CallHierarchyOutgoingCalls::METHOD => {
            let params: CallHierarchyOutgoingCallsParams = serde_json::from_value(request.params)?;
            let item = call_hierarchy_item_from_lsp(&params.item)
                .ok_or_else(|| anyhow!("missing call hierarchy item payload"))?;
            let calls = server.outgoing_calls(&item)?;
            let result = calls
                .iter()
                .filter_map(|call| outgoing_call_to_lsp(server, call))
                .collect::<Vec<_>>();
            send_ok(connection, request.id, Some(result))?;
        }
        FoldingRangeRequest::METHOD => {
            let params: FoldingRangeParams = serde_json::from_value(request.params)?;
            let result = server.folding_ranges(&params.text_document.uri)?;
            send_ok(connection, request.id, Some(result))?;
        }
        SemanticTokensFullRequest::METHOD => {
            let params: SemanticTokensParams = serde_json::from_value(request.params)?;
            let result = server.semantic_tokens(&params.text_document.uri, None)?;
            let result = server.semantic_tokens_full(&params.text_document.uri, result);
            send_ok(connection, request.id, Some(semantic_tokens_result(result)))?;
        }
        SemanticTokensFullDeltaRequest::METHOD => {
            let params: SemanticTokensDeltaParams = serde_json::from_value(request.params)?;
            let result = server.semantic_tokens(&params.text_document.uri, None)?;
            let result = server.semantic_tokens_delta(
                &params.text_document.uri,
                &params.previous_result_id,
                result,
            );
            send_ok(
                connection,
                request.id,
                Some(semantic_tokens_delta_result(result)),
            )?;
        }
        SemanticTokensRangeRequest::METHOD => {
            let params: SemanticTokensRangeParams = serde_json::from_value(request.params)?;
            let result = server.semantic_tokens(&params.text_document.uri, Some(params.range))?;
            send_ok(connection, request.id, Some(semantic_tokens_result(result)))?;
        }
        Formatting::METHOD => {
            let params: lsp_types::DocumentFormattingParams =
                serde_json::from_value(request.params)?;
            let result = server.format_document(&params.text_document.uri, params.options)?;
            send_ok(connection, request.id, result)?;
        }
        RangeFormatting::METHOD => {
            let params: lsp_types::DocumentRangeFormattingParams =
                serde_json::from_value(request.params)?;
            let result =
                server.format_range(&params.text_document.uri, params.range, params.options)?;
            send_ok(connection, request.id, result)?;
        }
        OnTypeFormatting::METHOD => {
            let params: DocumentOnTypeFormattingParams = serde_json::from_value(request.params)?;
            let result = server.format_on_type(
                &params.text_document_position.text_document.uri,
                params.text_document_position.position,
                &params.ch,
                params.options,
            )?;
            send_ok(connection, request.id, result)?;
        }
        CodeActionRequest::METHOD => {
            let params: CodeActionParams = serde_json::from_value(request.params)?;
            let requested_kinds = params.context.only.as_deref();
            let result = server
                .code_actions(
                    &params.text_document.uri,
                    params.range,
                    &params.context.diagnostics,
                    requested_kinds,
                )?
                .into_iter()
                .filter_map(|action| {
                    unresolved_code_action_to_lsp(
                        action.title.clone(),
                        action.kind.clone(),
                        action.diagnostics,
                        action.is_preferred,
                        crate::protocol::CodeActionResolvePayload {
                            uri: params.text_document.uri.to_string(),
                            request_range: params.range,
                            id: action.id,
                            kind: action.kind.as_str().to_owned(),
                            title: action.title,
                            target_start: u32::from(action.target.start()),
                            target_end: u32::from(action.target.end()),
                        },
                    )
                })
                .collect::<CodeActionResponse>();
            send_ok(connection, request.id, Some(result))?;
        }
        CodeActionResolveRequest::METHOD => {
            let action: lsp_types::CodeAction = serde_json::from_value(request.params)?;
            let payload = resolve_code_action_payload(&action)
                .ok_or_else(|| anyhow!("missing code action resolve payload"))?;
            let resolved = server.resolve_code_action(&payload)?.and_then(|resolved| {
                resolved_code_action_to_lsp(server, action, &resolved.source_change)
            });
            send_ok(connection, request.id, resolved)?;
        }
        SelectionRangeRequest::METHOD => {
            let params: SelectionRangeParams = serde_json::from_value(request.params)?;
            let result = server.selection_ranges(&params.text_document.uri, &params.positions)?;
            send_ok(connection, request.id, Some(result))?;
        }
        LinkedEditingRange::METHOD => {
            let params: LinkedEditingRangeParams = serde_json::from_value(request.params)?;
            let result = with_text_document_position(
                server,
                params.text_document_position_params,
                |uri, offset| server.linked_editing_ranges(&uri, offset),
            )?;
            send_ok(connection, request.id, result)?;
        }
        PrepareRenameRequest::METHOD => {
            let params: lsp_types::TextDocumentPositionParams =
                serde_json::from_value(request.params)?;
            let result = with_text_document_position(server, params, |query_uri, offset| {
                let prepared = server.prepare_rename(&query_uri, offset)?;
                let text = open_document_text_by_uri(server, &query_uri)
                    .ok_or_else(|| anyhow!("document `{}` is not open", query_uri.as_str()))?;
                Ok(prepared
                    .and_then(|prepared| prepared_rename_to_lsp(text.as_ref(), &prepared, offset)))
            })?;
            send_ok(connection, request.id, result)?;
        }
        Rename::METHOD => {
            let params: RenameParams = serde_json::from_value(request.params)?;
            let prepared = with_text_document_position(
                server,
                params.text_document_position,
                |uri, offset| server.rename(&uri, offset, params.new_name.clone()),
            )?;
            match prepared {
                Some(prepared) if prepared.source_change.is_some() => {
                    send_ok(
                        connection,
                        request.id,
                        rename_to_workspace_edit(server, prepared),
                    )?;
                }
                Some(prepared) => {
                    let message = prepared
                        .plan
                        .issues
                        .iter()
                        .map(|issue| issue.message.clone())
                        .collect::<Vec<_>>()
                        .join("; ");
                    send_error(connection, request.id, ErrorCode::InvalidParams, message)?;
                }
                None => send_ok::<Option<WorkspaceEdit>>(connection, request.id, None)?,
            }
        }
        _ => {
            send_error(
                connection,
                request.id,
                ErrorCode::MethodNotFound,
                format!("unhandled method `{}`", request.method),
            )?;
        }
    }

    Ok(())
}
