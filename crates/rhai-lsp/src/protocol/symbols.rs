use lsp_types::{
    self, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, CodeAction,
    CodeActionKind, CodeActionOrCommand, Diagnostic, DocumentChangeOperation, DocumentChanges,
    DocumentHighlight, DocumentHighlightKind, DocumentSymbol, GotoDefinitionResponse, InlayHint,
    InlayHintKind, Location, OneOf, OptionalVersionedTextDocumentIdentifier, PrepareRenameResponse,
    RenameFile, ResourceOp, SemanticTokensFullDeltaResult, SemanticTokensResult, SignatureHelp,
    SignatureInformation, TextDocumentEdit, TextEdit, WorkspaceEdit, WorkspaceSymbol,
    WorkspaceSymbolResponse,
};
use rhai_ide::{
    CallHierarchyItem as IdeCallHierarchyItem, DocumentHighlight as IdeDocumentHighlight,
    DocumentHighlightKind as IdeDocumentHighlightKind, DocumentSymbol as IdeDocumentSymbol,
    IncomingCall as IdeIncomingCall, InlayHint as IdeInlayHint, InlayHintKind as IdeInlayHintKind,
    NavigationTarget as IdeNavigationTarget, OutgoingCall as IdeOutgoingCall, PreparedRename,
    ReferenceLocation, ReferencesResult, SignatureHelp as IdeSignatureHelp,
    SignatureInformation as IdeSignatureInformation, SignatureParameter as IdeSignatureParameter,
    SourceChange,
};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use serde::{Deserialize, Serialize};

use crate::state::{ServerState, WorkspaceSymbolMatch};

use crate::protocol::{
    SymbolKindPayload, markdown_documentation, symbol_kind, symbol_kind_from_payload,
    symbol_kind_payload, text::prepare_rename_range, text::rename_placeholder, text::text_slice,
    text_range_to_lsp_range,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CallHierarchyPayload {
    pub file_id: u32,
    pub name: String,
    pub kind: SymbolKindPayload,
    pub full_start: u32,
    pub full_end: u32,
    pub focus_start: u32,
    pub focus_end: u32,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CodeActionResolvePayload {
    pub uri: String,
    pub request_range: lsp_types::Range,
    pub id: String,
    pub kind: String,
    pub title: String,
    pub target_start: u32,
    pub target_end: u32,
}

pub(crate) fn goto_definition_response(
    server: &ServerState,
    targets: Vec<IdeNavigationTarget>,
) -> Option<GotoDefinitionResponse> {
    let locations = targets
        .into_iter()
        .filter_map(|target| navigation_target_to_location(server, &target))
        .collect::<Vec<_>>();

    Some(GotoDefinitionResponse::Array(locations))
}

pub(crate) fn references_to_lsp(
    server: &ServerState,
    references: ReferencesResult,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();

    if include_declaration {
        locations.extend(
            references
                .targets
                .iter()
                .filter_map(|target| navigation_target_to_location(server, target)),
        );
    }

    locations.extend(
        references
            .references
            .iter()
            .filter(|reference| {
                include_declaration
                    || !matches!(reference.kind, rhai_ide::ReferenceKind::Definition)
            })
            .filter_map(|reference| reference_location_to_location(server, reference)),
    );

    locations
}

pub(crate) fn rename_to_workspace_edit(
    server: &ServerState,
    prepared: PreparedRename,
) -> Option<WorkspaceEdit> {
    let source_change = prepared.source_change?;
    source_change_to_workspace_edit(server, &source_change)
}

pub(crate) fn prepared_rename_to_lsp(
    text: &str,
    prepared: &PreparedRename,
    offset: u32,
) -> Option<PrepareRenameResponse> {
    let range = prepare_rename_range(prepared, offset)?;
    let range = text_range_to_lsp_range(text, range)?;
    let placeholder = text_slice(text, prepare_rename_range(prepared, offset)?)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range,
        placeholder: rename_placeholder(placeholder),
    })
}

pub(crate) fn document_symbols_to_lsp(
    server: &ServerState,
    file_id: FileId,
    symbols: Vec<IdeDocumentSymbol>,
) -> Option<Vec<DocumentSymbol>> {
    let text = server.analysis_host().snapshot().file_text(file_id)?;
    Some(
        symbols
            .iter()
            .filter_map(|symbol| document_symbol_to_lsp(text.as_ref(), symbol))
            .collect(),
    )
}

pub(crate) fn workspace_symbols_to_lsp(
    server: &ServerState,
    matches: Vec<WorkspaceSymbolMatch>,
) -> Option<WorkspaceSymbolResponse> {
    let symbols = matches
        .iter()
        .filter_map(|symbol| workspace_symbol_to_lsp(server, symbol))
        .collect::<Vec<_>>();
    Some(WorkspaceSymbolResponse::Nested(symbols))
}

pub(crate) fn signature_help_to_lsp(help: IdeSignatureHelp) -> SignatureHelp {
    let active_parameter = help.active_parameter as u32;
    SignatureHelp {
        signatures: help
            .signatures
            .into_iter()
            .map(|signature| {
                let signature_active_parameter =
                    signature_active_parameter(&signature, active_parameter);
                SignatureInformation {
                    label: signature.label,
                    documentation: signature
                        .docs
                        .and_then(non_empty_docs)
                        .map(markdown_documentation),
                    parameters: Some(
                        signature
                            .parameters
                            .into_iter()
                            .map(signature_parameter_to_lsp)
                            .collect(),
                    ),
                    active_parameter: signature_active_parameter,
                }
            })
            .collect(),
        active_signature: Some(help.active_signature as u32),
        active_parameter: Some(active_parameter),
    }
}

pub(crate) fn inlay_hint_to_lsp(text: &str, hint: &IdeInlayHint) -> Option<InlayHint> {
    let position = text_range_to_lsp_range(
        text,
        TextRange::new(TextSize::from(hint.offset), TextSize::from(hint.offset)),
    )?
    .start;

    Some(InlayHint {
        position,
        label: lsp_types::InlayHintLabel::String(hint.label.clone()),
        kind: Some(match hint.kind {
            IdeInlayHintKind::Type => InlayHintKind::TYPE,
        }),
        text_edits: None,
        tooltip: None,
        padding_left: Some(true),
        padding_right: Some(false),
        data: None,
    })
}

pub(crate) fn document_highlight_to_lsp(
    text: &str,
    highlight: &IdeDocumentHighlight,
) -> Option<DocumentHighlight> {
    Some(DocumentHighlight {
        range: text_range_to_lsp_range(text, highlight.range)?,
        kind: Some(match highlight.kind {
            IdeDocumentHighlightKind::Read => DocumentHighlightKind::READ,
            IdeDocumentHighlightKind::Write => DocumentHighlightKind::WRITE,
            IdeDocumentHighlightKind::Text => DocumentHighlightKind::TEXT,
        }),
    })
}

pub(crate) fn call_hierarchy_item_to_lsp(
    server: &ServerState,
    item: &IdeCallHierarchyItem,
) -> Option<CallHierarchyItem> {
    let snapshot = server.analysis_host().snapshot();
    let text = snapshot.file_text(item.file_id)?;
    let path = snapshot.normalized_path(item.file_id)?;

    Some(CallHierarchyItem {
        name: item.name.clone(),
        kind: symbol_kind(item.kind),
        tags: None,
        detail: item.container_name.clone(),
        uri: server.uri_for_path(path).ok()?,
        range: text_range_to_lsp_range(text.as_ref(), item.full_range)?,
        selection_range: text_range_to_lsp_range(text.as_ref(), item.focus_range)?,
        data: Some(
            serde_json::to_value(CallHierarchyPayload {
                file_id: item.file_id.0,
                name: item.name.clone(),
                kind: symbol_kind_payload(item.kind),
                full_start: u32::from(item.full_range.start()),
                full_end: u32::from(item.full_range.end()),
                focus_start: u32::from(item.focus_range.start()),
                focus_end: u32::from(item.focus_range.end()),
                container_name: item.container_name.clone(),
            })
            .ok()?,
        ),
    })
}

pub(crate) fn call_hierarchy_item_from_lsp(
    item: &CallHierarchyItem,
) -> Option<IdeCallHierarchyItem> {
    let payload = serde_json::from_value::<CallHierarchyPayload>(item.data.clone()?).ok()?;

    Some(IdeCallHierarchyItem {
        file_id: FileId(payload.file_id),
        name: payload.name,
        kind: symbol_kind_from_payload(payload.kind),
        full_range: TextRange::new(
            TextSize::from(payload.full_start),
            TextSize::from(payload.full_end),
        ),
        focus_range: TextRange::new(
            TextSize::from(payload.focus_start),
            TextSize::from(payload.focus_end),
        ),
        container_name: payload.container_name,
    })
}

pub(crate) fn incoming_call_to_lsp(
    server: &ServerState,
    call: &IdeIncomingCall,
) -> Option<CallHierarchyIncomingCall> {
    let from = call_hierarchy_item_to_lsp(server, &call.from)?;
    let text = server
        .analysis_host()
        .snapshot()
        .file_text(call.from.file_id)?;
    let from_ranges = call
        .from_ranges
        .iter()
        .filter_map(|range| text_range_to_lsp_range(text.as_ref(), *range))
        .collect::<Vec<_>>();

    Some(CallHierarchyIncomingCall { from, from_ranges })
}

pub(crate) fn outgoing_call_to_lsp(
    server: &ServerState,
    call: &IdeOutgoingCall,
) -> Option<CallHierarchyOutgoingCall> {
    let to = call_hierarchy_item_to_lsp(server, &call.to)?;
    let text = server
        .analysis_host()
        .snapshot()
        .file_text(call.to.file_id)?;
    let from_ranges = call
        .from_ranges
        .iter()
        .filter_map(|range| text_range_to_lsp_range(text.as_ref(), *range))
        .collect::<Vec<_>>();

    Some(CallHierarchyOutgoingCall { to, from_ranges })
}

pub(crate) fn semantic_tokens_result(tokens: lsp_types::SemanticTokens) -> SemanticTokensResult {
    SemanticTokensResult::Tokens(tokens)
}

pub(crate) fn semantic_tokens_delta_result(
    tokens: SemanticTokensFullDeltaResult,
) -> SemanticTokensFullDeltaResult {
    tokens
}

#[cfg(test)]
pub(crate) fn source_change_code_action_to_lsp(
    server: &ServerState,
    title: impl Into<String>,
    kind: CodeActionKind,
    source_change: &SourceChange,
) -> Option<CodeActionOrCommand> {
    let edit = source_change_to_workspace_edit(server, source_change)?;

    Some(
        CodeAction {
            title: title.into(),
            kind: Some(kind),
            diagnostics: None,
            edit: Some(edit),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }
        .into(),
    )
}

pub(crate) fn unresolved_code_action_to_lsp(
    title: impl Into<String>,
    kind: CodeActionKind,
    diagnostics: Vec<Diagnostic>,
    is_preferred: bool,
    payload: CodeActionResolvePayload,
) -> Option<CodeActionOrCommand> {
    Some(
        CodeAction {
            title: title.into(),
            kind: Some(kind),
            diagnostics: (!diagnostics.is_empty()).then_some(diagnostics),
            edit: None,
            command: None,
            is_preferred: is_preferred.then_some(true),
            disabled: None,
            data: Some(serde_json::to_value(payload).ok()?),
        }
        .into(),
    )
}

pub(crate) fn resolve_code_action_payload(action: &CodeAction) -> Option<CodeActionResolvePayload> {
    serde_json::from_value(action.data.clone()?).ok()
}

pub(crate) fn resolved_code_action_to_lsp(
    server: &ServerState,
    mut action: CodeAction,
    source_change: &SourceChange,
) -> Option<CodeAction> {
    action.edit = Some(source_change_to_workspace_edit(server, source_change)?);
    Some(action)
}

fn signature_parameter_to_lsp(parameter: IdeSignatureParameter) -> lsp_types::ParameterInformation {
    let label = match parameter.annotation {
        Some(annotation) => format!("{}: {annotation}", parameter.label),
        None => parameter.label,
    };

    lsp_types::ParameterInformation {
        label: lsp_types::ParameterLabel::Simple(label),
        documentation: None,
    }
}

#[allow(deprecated)]
fn document_symbol_to_lsp(text: &str, symbol: &IdeDocumentSymbol) -> Option<DocumentSymbol> {
    Some(DocumentSymbol {
        name: symbol.name.clone(),
        detail: None,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: text_range_to_lsp_range(text, symbol.full_range)?,
        selection_range: text_range_to_lsp_range(text, symbol.focus_range)?,
        children: Some(
            symbol
                .children
                .iter()
                .filter_map(|child| document_symbol_to_lsp(text, child))
                .collect(),
        ),
    })
}

fn workspace_symbol_to_lsp(
    server: &ServerState,
    symbol: &WorkspaceSymbolMatch,
) -> Option<WorkspaceSymbol> {
    let text = server
        .analysis_host()
        .snapshot()
        .file_text(symbol.symbol.file_id)?;

    Some(WorkspaceSymbol {
        name: symbol.symbol.name.clone(),
        kind: symbol_kind(symbol.symbol.kind),
        tags: None,
        container_name: symbol.symbol.container_name.clone(),
        location: OneOf::Left(Location {
            uri: symbol.uri.clone(),
            range: text_range_to_lsp_range(text.as_ref(), symbol.symbol.focus_range)?,
        }),
        data: None,
    })
}

fn navigation_target_to_location(
    server: &ServerState,
    target: &IdeNavigationTarget,
) -> Option<Location> {
    let snapshot = server.analysis_host().snapshot();
    let path = snapshot.normalized_path(target.file_id)?;
    let text = snapshot.file_text(target.file_id)?;

    Some(Location {
        uri: server.uri_for_path(path).ok()?,
        range: text_range_to_lsp_range(text.as_ref(), target.focus_range)?,
    })
}

fn reference_location_to_location(
    server: &ServerState,
    reference: &ReferenceLocation,
) -> Option<Location> {
    let snapshot = server.analysis_host().snapshot();
    let path = snapshot.normalized_path(reference.file_id)?;
    let text = snapshot.file_text(reference.file_id)?;

    Some(Location {
        uri: server.uri_for_path(path).ok()?,
        range: text_range_to_lsp_range(text.as_ref(), reference.range)?,
    })
}

fn source_change_to_workspace_edit(
    server: &ServerState,
    change: &SourceChange,
) -> Option<WorkspaceEdit> {
    let snapshot = server.analysis_host().snapshot();
    let mut document_changes = Vec::<DocumentChangeOperation>::new();

    for file_edit in &change.file_edits {
        let path = snapshot.normalized_path(file_edit.file_id)?;
        let text = snapshot.file_text(file_edit.file_id)?;
        let uri = server.uri_for_path(path).ok()?;
        let edits = file_edit
            .edits
            .iter()
            .map(|edit| {
                Some(OneOf::Left(TextEdit {
                    range: text_range_to_lsp_range(text.as_ref(), edit.range)?,
                    new_text: edit.new_text.clone(),
                }))
            })
            .collect::<Option<Vec<_>>>()?;
        let version = server
            .open_documents
            .values()
            .find(|document| document.normalized_path == path)
            .map(|document| document.version);

        document_changes.push(DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier { uri, version },
            edits,
        }));
    }

    for file_rename in &change.file_renames {
        let old_path = snapshot.normalized_path(file_rename.file_id)?;
        let old_uri = server.uri_for_path(old_path).ok()?;
        let new_uri = server.uri_for_path(&file_rename.new_path).ok()?;
        document_changes.push(DocumentChangeOperation::Op(ResourceOp::Rename(
            RenameFile {
                old_uri,
                new_uri,
                options: None,
                annotation_id: None,
            },
        )));
    }

    Some(WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Operations(document_changes)),
        change_annotations: None,
    })
}

fn non_empty_docs(docs: String) -> Option<String> {
    let trimmed = docs.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn signature_active_parameter(
    signature: &IdeSignatureInformation,
    active_parameter: u32,
) -> Option<u32> {
    let parameter_count = u32::try_from(signature.parameters.len()).ok()?;
    (parameter_count > 0).then_some(active_parameter.min(parameter_count.saturating_sub(1)))
}
