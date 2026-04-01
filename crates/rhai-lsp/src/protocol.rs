use lsp_types::{
    self, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, CodeAction,
    CodeActionKind, CodeActionOrCommand, CompletionItem, CompletionItemKind,
    CompletionItemLabelDetails, Diagnostic, DocumentChangeOperation, DocumentChanges,
    DocumentHighlight, DocumentHighlightKind, DocumentSymbol, Documentation,
    GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind, Location,
    MarkupContent, MarkupKind, OneOf, OptionalVersionedTextDocumentIdentifier, Position,
    PrepareRenameResponse, Range, RenameFile, ResourceOp, SemanticTokensFullDeltaResult,
    SemanticTokensResult, SignatureHelp, SignatureInformation, SymbolKind, TextDocumentEdit,
    TextEdit, WorkspaceEdit, WorkspaceSymbol, WorkspaceSymbolResponse,
};
use rhai_hir::SymbolKind as HirSymbolKind;
use rhai_ide::{
    CallHierarchyItem as IdeCallHierarchyItem, CompletionInsertFormat as IdeCompletionInsertFormat,
    CompletionItem as IdeCompletionItem, CompletionItemKind as IdeCompletionItemKind,
    CompletionItemSource as IdeCompletionItemSource, CompletionResolveData,
    Diagnostic as IdeDiagnostic, DiagnosticSeverity as IdeDiagnosticSeverity,
    DiagnosticTag as IdeDiagnosticTag, DocumentHighlight as IdeDocumentHighlight,
    DocumentHighlightKind as IdeDocumentHighlightKind, DocumentSymbol as IdeDocumentSymbol,
    HoverResult, HoverSignatureSource, IncomingCall as IdeIncomingCall, InlayHint as IdeInlayHint,
    InlayHintKind as IdeInlayHintKind, NavigationTarget as IdeNavigationTarget,
    OutgoingCall as IdeOutgoingCall, PreparedRename, ReferenceLocation, ReferencesResult,
    SignatureHelp as IdeSignatureHelp, SignatureInformation as IdeSignatureInformation,
    SignatureParameter as IdeSignatureParameter, SourceChange,
};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use serde::{Deserialize, Serialize};

use crate::state::{ServerState, WorkspaceSymbolMatch};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompletionResolvePayload {
    pub label: String,
    pub kind: CompletionKindPayload,
    pub source: CompletionSourcePayload,
    pub origin: Option<String>,
    pub sort_text: String,
    pub file_id: Option<u32>,
    pub exported: bool,
    pub resolve_file_id: u32,
    pub resolve_offset: u32,
}

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
    pub request_range: Range,
    pub id: String,
    pub kind: String,
    pub title: String,
    pub target_start: u32,
    pub target_end: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum SymbolKindPayload {
    Variable,
    Parameter,
    Constant,
    Function,
    ImportAlias,
    ExportAlias,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum CompletionSourcePayload {
    Visible,
    Project,
    Module,
    Member,
    Builtin,
    Postfix,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum CompletionKindPayload {
    Member,
    Symbol(SymbolKindPayload),
    Keyword,
    Type,
}

pub(crate) fn diagnostic_to_lsp(text: &str, diagnostic: &IdeDiagnostic) -> Option<Diagnostic> {
    Some(Diagnostic {
        range: text_range_to_lsp_range(text, diagnostic.range)?,
        severity: Some(match diagnostic.severity {
            IdeDiagnosticSeverity::Error => lsp_types::DiagnosticSeverity::ERROR,
            IdeDiagnosticSeverity::Warning => lsp_types::DiagnosticSeverity::WARNING,
        }),
        code: None,
        code_description: None,
        source: Some("rhai-analyzer".to_owned()),
        message: diagnostic.message.clone(),
        related_information: None,
        tags: if diagnostic.tags.is_empty() {
            None
        } else {
            Some(
                diagnostic
                    .tags
                    .iter()
                    .map(|tag| match tag {
                        IdeDiagnosticTag::Unnecessary => lsp_types::DiagnosticTag::UNNECESSARY,
                    })
                    .collect(),
            )
        },
        data: None,
    })
}

pub(crate) fn completion_item_to_lsp(
    server: &ServerState,
    text: Option<&str>,
    item: IdeCompletionItem,
) -> CompletionItem {
    let label_details = completion_label_details(server, &item);
    let documentation = item.docs.map(markdown_documentation);
    let text_edit = text.zip(item.text_edit.as_ref()).and_then(|(text, edit)| {
        Some(TextEdit {
            range: text_range_to_lsp_range(text, edit.range)?,
            new_text: edit.new_text.clone(),
        })
    });
    let data = item.resolve_data.as_ref().map(|resolve_data| {
        serde_json::to_value(CompletionResolvePayload {
            label: item.label.clone(),
            kind: completion_kind_payload(item.kind),
            source: completion_source_payload(item.source),
            origin: item.origin.clone(),
            sort_text: item.sort_text.clone(),
            file_id: item.file_id.map(|file_id| file_id.0),
            exported: item.exported,
            resolve_file_id: resolve_data.file_id.0,
            resolve_offset: resolve_data.offset,
        })
        .expect("completion resolve payload should serialize")
    });

    CompletionItem {
        label: item.label,
        kind: Some(completion_item_kind(item.kind)),
        detail: item.detail,
        documentation,
        sort_text: Some(item.sort_text),
        insert_text: None,
        insert_text_format: Some(match item.insert_format {
            IdeCompletionInsertFormat::PlainText => lsp_types::InsertTextFormat::PLAIN_TEXT,
            IdeCompletionInsertFormat::Snippet => lsp_types::InsertTextFormat::SNIPPET,
        }),
        insert_text_mode: None,
        text_edit: text_edit.map(lsp_types::CompletionTextEdit::Edit),
        additional_text_edits: None,
        command: None,
        commit_characters: None,
        data,
        deprecated: None,
        preselect: None,
        filter_text: item.filter_text,
        label_details,
        tags: None,
    }
}

pub(crate) fn completion_item_from_lsp(item: CompletionItem) -> Option<IdeCompletionItem> {
    let payload = serde_json::from_value::<CompletionResolvePayload>(item.data.clone()?).ok()?;

    Some(IdeCompletionItem {
        label: payload.label,
        kind: completion_kind_from_payload(payload.kind),
        source: completion_source_from_payload(payload.source),
        origin: payload.origin,
        sort_text: payload.sort_text,
        detail: item.detail,
        docs: documentation_text(item.documentation),
        filter_text: item.filter_text,
        text_edit: None,
        insert_format: match item.insert_text_format {
            Some(lsp_types::InsertTextFormat::SNIPPET) => IdeCompletionInsertFormat::Snippet,
            _ => IdeCompletionInsertFormat::PlainText,
        },
        file_id: payload.file_id.map(FileId),
        exported: payload.exported,
        resolve_data: Some(CompletionResolveData {
            file_id: FileId(payload.resolve_file_id),
            offset: payload.resolve_offset,
        }),
    })
}

pub(crate) fn hover_to_lsp(hover: HoverResult) -> Hover {
    let mut lines = vec![rhai_code_block(hover.signature.as_str())];

    if let Some(docs) = hover.docs {
        lines.push(format_hover_docs(docs));
    }

    lines.push(format!(
        "### Source\n\n{}",
        hover_source_label(hover.source)
    ));

    if let Some(declared) = hover
        .declared_signature
        .as_ref()
        .filter(|declared| declared.as_str() != hover.signature)
    {
        lines.push(format!(
            "### Declared Signature\n\n{}",
            rhai_code_block(declared)
        ));
    }
    if let Some(inferred) = hover
        .inferred_signature
        .as_ref()
        .filter(|inferred| inferred.as_str() != hover.signature)
    {
        lines.push(format!(
            "### Inferred Signature\n\n{}",
            rhai_code_block(inferred)
        ));
    }
    if !hover.overload_signatures.is_empty() {
        lines.push(format!(
            "### Other Overloads\n\n{}",
            hover
                .overload_signatures
                .into_iter()
                .map(|signature| rhai_code_block(signature.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n")
        ));
    }
    if !hover.notes.is_empty() {
        lines.push(format!(
            "### Notes\n\n{}",
            hover
                .notes
                .into_iter()
                .map(|note| format!("- {note}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Hover {
        contents: HoverContents::Markup(markup(lines.join("\n\n"))),
        range: None,
    }
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
    let position = offset_to_position(text, hint.offset as usize)?;

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

fn markdown_documentation(value: String) -> Documentation {
    Documentation::MarkupContent(markup(value))
}

fn rhai_code_block(value: &str) -> String {
    format!("```rhai\n{value}\n```")
}

fn hover_source_label(source: HoverSignatureSource) -> &'static str {
    match source {
        HoverSignatureSource::Declared => "Declared",
        HoverSignatureSource::Inferred => "Inferred",
        HoverSignatureSource::Structural => "Structural",
    }
}

fn format_hover_docs(docs: String) -> String {
    let trimmed = docs.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.starts_with("```")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("1. ")
        || trimmed.starts_with('#')
    {
        trimmed.to_owned()
    } else {
        format!("### Documentation\n\n{trimmed}")
    }
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

fn completion_label_details(
    server: &ServerState,
    item: &IdeCompletionItem,
) -> Option<CompletionItemLabelDetails> {
    let description = completion_source_description(server, item);
    description.map(|description| CompletionItemLabelDetails {
        detail: None,
        description: Some(description),
    })
}

fn completion_source_description(server: &ServerState, item: &IdeCompletionItem) -> Option<String> {
    let source = match (item.source, item.exported) {
        (IdeCompletionItemSource::Visible, _) => "local".to_owned(),
        (IdeCompletionItemSource::Project, true) => "project export".to_owned(),
        (IdeCompletionItemSource::Project, false) => "project".to_owned(),
        (IdeCompletionItemSource::Module, true) => "module export".to_owned(),
        (IdeCompletionItemSource::Module, false) => "module".to_owned(),
        (IdeCompletionItemSource::Member, _) => "member".to_owned(),
        (IdeCompletionItemSource::Builtin, _) => "builtin".to_owned(),
        (IdeCompletionItemSource::Postfix, _) => "postfix template".to_owned(),
    };

    let module = item.origin.clone().or_else(|| {
        item.file_id
            .and_then(|file_id| completion_module_label(server, file_id))
    });

    Some(match module {
        Some(module)
            if matches!(
                item.source,
                IdeCompletionItemSource::Project | IdeCompletionItemSource::Module
            ) && !module.is_empty() =>
        {
            format!("{source} · {module}")
        }
        _ => source,
    })
}

fn completion_module_label(server: &ServerState, file_id: FileId) -> Option<String> {
    let snapshot = server.analysis_host().snapshot();
    let path = snapshot.normalized_path(file_id)?;
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .map(ToOwned::to_owned)
}

fn markup(value: String) -> MarkupContent {
    MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    }
}

fn documentation_text(documentation: Option<Documentation>) -> Option<String> {
    match documentation? {
        Documentation::String(text) => Some(text),
        Documentation::MarkupContent(markup) => Some(markup.value),
    }
}

fn completion_item_kind(kind: IdeCompletionItemKind) -> CompletionItemKind {
    match kind {
        IdeCompletionItemKind::Member => CompletionItemKind::METHOD,
        IdeCompletionItemKind::Keyword => CompletionItemKind::KEYWORD,
        IdeCompletionItemKind::Type => CompletionItemKind::CLASS,
        IdeCompletionItemKind::Symbol(HirSymbolKind::Variable | HirSymbolKind::Parameter) => {
            CompletionItemKind::VARIABLE
        }
        IdeCompletionItemKind::Symbol(HirSymbolKind::Constant) => CompletionItemKind::CONSTANT,
        IdeCompletionItemKind::Symbol(HirSymbolKind::Function) => CompletionItemKind::FUNCTION,
        IdeCompletionItemKind::Symbol(HirSymbolKind::ImportAlias | HirSymbolKind::ExportAlias) => {
            CompletionItemKind::MODULE
        }
    }
}

fn completion_kind_payload(kind: IdeCompletionItemKind) -> CompletionKindPayload {
    match kind {
        IdeCompletionItemKind::Member => CompletionKindPayload::Member,
        IdeCompletionItemKind::Keyword => CompletionKindPayload::Keyword,
        IdeCompletionItemKind::Type => CompletionKindPayload::Type,
        IdeCompletionItemKind::Symbol(kind) => {
            CompletionKindPayload::Symbol(symbol_kind_payload(kind))
        }
    }
}

fn completion_kind_from_payload(kind: CompletionKindPayload) -> IdeCompletionItemKind {
    match kind {
        CompletionKindPayload::Member => IdeCompletionItemKind::Member,
        CompletionKindPayload::Keyword => IdeCompletionItemKind::Keyword,
        CompletionKindPayload::Type => IdeCompletionItemKind::Type,
        CompletionKindPayload::Symbol(SymbolKindPayload::Variable) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::Variable)
        }
        CompletionKindPayload::Symbol(SymbolKindPayload::Parameter) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::Parameter)
        }
        CompletionKindPayload::Symbol(SymbolKindPayload::Constant) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::Constant)
        }
        CompletionKindPayload::Symbol(SymbolKindPayload::Function) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::Function)
        }
        CompletionKindPayload::Symbol(SymbolKindPayload::ImportAlias) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::ImportAlias)
        }
        CompletionKindPayload::Symbol(SymbolKindPayload::ExportAlias) => {
            IdeCompletionItemKind::Symbol(HirSymbolKind::ExportAlias)
        }
    }
}

fn completion_source_payload(source: IdeCompletionItemSource) -> CompletionSourcePayload {
    match source {
        IdeCompletionItemSource::Visible => CompletionSourcePayload::Visible,
        IdeCompletionItemSource::Project => CompletionSourcePayload::Project,
        IdeCompletionItemSource::Module => CompletionSourcePayload::Module,
        IdeCompletionItemSource::Member => CompletionSourcePayload::Member,
        IdeCompletionItemSource::Builtin => CompletionSourcePayload::Builtin,
        IdeCompletionItemSource::Postfix => CompletionSourcePayload::Postfix,
    }
}

fn completion_source_from_payload(source: CompletionSourcePayload) -> IdeCompletionItemSource {
    match source {
        CompletionSourcePayload::Visible => IdeCompletionItemSource::Visible,
        CompletionSourcePayload::Project => IdeCompletionItemSource::Project,
        CompletionSourcePayload::Module => IdeCompletionItemSource::Module,
        CompletionSourcePayload::Member => IdeCompletionItemSource::Member,
        CompletionSourcePayload::Builtin => IdeCompletionItemSource::Builtin,
        CompletionSourcePayload::Postfix => IdeCompletionItemSource::Postfix,
    }
}

fn symbol_kind(kind: HirSymbolKind) -> SymbolKind {
    match kind {
        HirSymbolKind::Variable => SymbolKind::VARIABLE,
        HirSymbolKind::Parameter => SymbolKind::VARIABLE,
        HirSymbolKind::Constant => SymbolKind::CONSTANT,
        HirSymbolKind::Function => SymbolKind::FUNCTION,
        HirSymbolKind::ImportAlias => SymbolKind::MODULE,
        HirSymbolKind::ExportAlias => SymbolKind::MODULE,
    }
}

fn symbol_kind_payload(kind: HirSymbolKind) -> SymbolKindPayload {
    match kind {
        HirSymbolKind::Variable => SymbolKindPayload::Variable,
        HirSymbolKind::Parameter => SymbolKindPayload::Parameter,
        HirSymbolKind::Constant => SymbolKindPayload::Constant,
        HirSymbolKind::Function => SymbolKindPayload::Function,
        HirSymbolKind::ImportAlias => SymbolKindPayload::ImportAlias,
        HirSymbolKind::ExportAlias => SymbolKindPayload::ExportAlias,
    }
}

fn symbol_kind_from_payload(kind: SymbolKindPayload) -> HirSymbolKind {
    match kind {
        SymbolKindPayload::Variable => HirSymbolKind::Variable,
        SymbolKindPayload::Parameter => HirSymbolKind::Parameter,
        SymbolKindPayload::Constant => HirSymbolKind::Constant,
        SymbolKindPayload::Function => HirSymbolKind::Function,
        SymbolKindPayload::ImportAlias => HirSymbolKind::ImportAlias,
        SymbolKindPayload::ExportAlias => HirSymbolKind::ExportAlias,
    }
}

pub(crate) fn open_document_text_by_uri(
    server: &ServerState,
    uri: &lsp_types::Uri,
) -> Option<std::sync::Arc<str>> {
    server
        .open_documents
        .get(uri)
        .map(|document| std::sync::Arc::<str>::from(document.text.as_str()))
}

pub(crate) fn file_text_by_uri(
    server: &ServerState,
    uri: &lsp_types::Uri,
) -> Option<std::sync::Arc<str>> {
    let normalized_path = crate::state::path_from_uri(uri).ok()?;
    let snapshot = server.analysis_host().snapshot();
    let file_id = snapshot.file_id_for_path(&normalized_path)?;
    snapshot.file_text(file_id)
}

pub(crate) fn text_range_to_lsp_range(text: &str, range: TextRange) -> Option<Range> {
    Some(Range {
        start: offset_to_position(text, u32::from(range.start()) as usize)?,
        end: offset_to_position(text, u32::from(range.end()) as usize)?,
    })
}

fn offset_to_position(text: &str, offset: usize) -> Option<Position> {
    if offset > text.len() {
        return None;
    }

    let line_starts = line_start_offsets(text);
    let line_index = line_index(&line_starts, offset);
    let line_start = *line_starts.get(line_index)?;

    Some(Position {
        line: line_index as u32,
        character: utf16_len(text.get(line_start..offset)?) as u32,
    })
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

fn line_index(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

fn prepare_rename_range(prepared: &PreparedRename, offset: u32) -> Option<TextRange> {
    let offset = TextSize::from(offset);

    prepared
        .plan
        .targets
        .iter()
        .map(|target| target.focus_range)
        .chain(
            prepared
                .plan
                .occurrences
                .iter()
                .map(|occurrence| occurrence.range),
        )
        .find(|range| range.contains(offset))
        .or_else(|| {
            prepared
                .plan
                .targets
                .first()
                .map(|target| target.focus_range)
        })
        .or_else(|| {
            prepared
                .plan
                .occurrences
                .first()
                .map(|occurrence| occurrence.range)
        })
}

fn text_slice(text: &str, range: TextRange) -> Option<&str> {
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    text.get(start..end)
}

fn rename_placeholder(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let Some(last) = text.chars().last() else {
        return String::new();
    };

    if matches!(first, '"' | '\'' | '`') && first == last && text.len() >= first.len_utf8() * 2 {
        text.strip_prefix(first)
            .and_then(|text| text.strip_suffix(last))
            .unwrap_or(text)
            .to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::{CodeActionKind, DocumentChangeOperation, DocumentChanges, ResourceOp};
    use rhai_hir::SymbolKind;
    use rhai_ide::{
        CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource,
        FileRename, FileTextEdit, HoverResult, HoverSignatureSource,
        SignatureHelp as IdeSignatureHelp, SignatureInformation as IdeSignatureInformation,
        SignatureParameter as IdeSignatureParameter, SourceChange, TextEdit,
    };
    use rhai_syntax::{TextRange, TextSize};

    use crate::protocol::{
        completion_item_to_lsp, hover_to_lsp, prepared_rename_to_lsp, signature_help_to_lsp,
        source_change_code_action_to_lsp,
    };
    use crate::tests::file_url;
    use crate::{Server, ServerState};

    #[test]
    fn code_action_conversion_supports_multi_file_edits_and_file_renames() {
        let mut server = Server::new();
        let provider_uri = file_url("provider.rhai");
        let consumer_uri = file_url("consumer.rhai");
        let provider_text = "fn helper() {}\n";
        let consumer_text = "import \"provider\" as p;\np::helper();\n";

        server
            .open_document(provider_uri.clone(), 1, provider_text)
            .expect("expected provider open to succeed");
        server
            .open_document(consumer_uri.clone(), 1, consumer_text)
            .expect("expected consumer open to succeed");

        let snapshot = server.analysis_host().snapshot();
        let provider_file_id = snapshot
            .file_id_for_path(&std::env::current_dir().expect("cwd").join("provider.rhai"))
            .expect("expected provider file id");
        let consumer_file_id = snapshot
            .file_id_for_path(&std::env::current_dir().expect("cwd").join("consumer.rhai"))
            .expect("expected consumer file id");
        let change = SourceChange::new(vec![
            FileTextEdit::new(
                provider_file_id,
                vec![TextEdit::replace(
                    TextRange::new(TextSize::from(3), TextSize::from(9)),
                    "renamed".to_owned(),
                )],
            ),
            FileTextEdit::new(
                consumer_file_id,
                vec![TextEdit::replace(
                    TextRange::new(TextSize::from(24), TextSize::from(30)),
                    "renamed".to_owned(),
                )],
            ),
        ])
        .with_file_renames(vec![FileRename::new(
            provider_file_id,
            std::env::current_dir()
                .expect("cwd")
                .join("renamed_provider.rhai"),
        )]);

        let action = source_change_code_action_to_lsp(
            &server,
            "Apply import fix",
            CodeActionKind::QUICKFIX,
            &change,
        )
        .expect("expected code action conversion");

        let lsp_types::CodeActionOrCommand::CodeAction(action) = action else {
            panic!("expected code action");
        };
        let document_changes = action
            .edit
            .expect("expected workspace edit")
            .document_changes
            .expect("expected document changes");
        let DocumentChanges::Operations(operations) = document_changes else {
            panic!("expected operation-based workspace edit");
        };

        assert!(
            operations
                .iter()
                .filter(|operation| matches!(operation, DocumentChangeOperation::Edit(_)))
                .count()
                >= 2
        );
        assert!(operations.iter().any(|operation| matches!(
            operation,
            DocumentChangeOperation::Op(ResourceOp::Rename(rename))
                if rename.new_uri.as_str().ends_with("/renamed_provider.rhai")
                    || rename.new_uri.as_str().ends_with("\\renamed_provider.rhai")
        )));
    }

    #[test]
    fn prepare_rename_placeholder_strips_surrounding_quotes() {
        let prepared = rhai_ide::PreparedRename {
            plan: rhai_ide::RenamePlan {
                new_name: String::new(),
                targets: Vec::new(),
                occurrences: vec![rhai_ide::ReferenceLocation {
                    file_id: rhai_vfs::FileId(0),
                    range: TextRange::new(TextSize::from(7), TextSize::from(13)),
                    kind: rhai_ide::ReferenceKind::Reference,
                }],
                issues: Vec::new(),
            },
            source_change: None,
        };

        let response = prepared_rename_to_lsp("import \"demo\";\n", &prepared, 8)
            .expect("expected prepare rename response");

        match response {
            lsp_types::PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
                assert_eq!(placeholder, "demo");
            }
            other => panic!("expected placeholder response, got {other:?}"),
        }
    }

    #[test]
    fn hover_conversion_omits_duplicate_signature_lines() {
        let hover = hover_to_lsp(HoverResult {
            signature: "let result: any".to_owned(),
            docs: Some("hover docs".to_owned()),
            source: HoverSignatureSource::Declared,
            declared_signature: Some("let result: any".to_owned()),
            inferred_signature: Some("let result: any | blob".to_owned()),
            overload_signatures: vec!["fn result(blob) -> blob".to_owned()],
            notes: vec!["A note".to_owned()],
        });

        let lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markdown hover");
        };
        assert!(markup.value.contains("### Documentation"));
        assert!(markup.value.contains("hover docs"));
        assert!(markup.value.contains("### Source"));
        assert!(markup.value.contains("Declared"));
        assert!(!markup.value.contains("### Declared Signature"));
        assert!(markup.value.contains("### Inferred Signature"));
        assert!(
            markup
                .value
                .contains("```rhai\nlet result: any | blob\n```")
        );
        assert!(markup.value.contains("### Other Overloads"));
        assert!(
            markup
                .value
                .contains("```rhai\nfn result(blob) -> blob\n```")
        );
        assert!(markup.value.contains("### Notes"));
        assert!(markup.value.contains("- A note"));
    }

    #[test]
    fn completion_conversion_surfaces_source_descriptions() {
        let server = ServerState::new();
        let item = completion_item_to_lsp(
            &server,
            None,
            CompletionItem {
                label: "shared_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Project,
                origin: None,
                sort_text: "0".to_owned(),
                detail: Some("fun() -> ()".to_owned()),
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: true,
                resolve_data: None,
            },
        );

        assert_eq!(item.detail.as_deref(), Some("fun() -> ()"));
        assert_eq!(
            item.label_details
                .as_ref()
                .and_then(|details| details.description.as_deref()),
            Some("project export")
        );
    }

    #[test]
    fn completion_conversion_includes_project_module_name() {
        let mut server = ServerState::new();
        server
            .open_document(file_url("support.rhai"), 1, "fn shared_helper() {}")
            .expect("expected support.rhai to open");
        let file_id = server
            .analysis_host()
            .snapshot()
            .file_id_for_path(&std::env::current_dir().expect("cwd").join("support.rhai"))
            .expect("expected support.rhai");

        let item = completion_item_to_lsp(
            &server,
            None,
            CompletionItem {
                label: "shared_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Project,
                origin: Some("support".to_owned()),
                sort_text: "0".to_owned(),
                detail: Some("fun() -> ()".to_owned()),
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: Some(file_id),
                exported: true,
                resolve_data: None,
            },
        );

        assert_eq!(
            item.label_details
                .as_ref()
                .and_then(|details| details.description.as_deref()),
            Some("project export · support")
        );
    }

    #[test]
    fn completion_conversion_includes_module_origin_name() {
        let server = ServerState::new();

        let item = completion_item_to_lsp(
            &server,
            None,
            CompletionItem {
                label: "shared_helper".to_owned(),
                kind: CompletionItemKind::Symbol(SymbolKind::Function),
                source: CompletionItemSource::Module,
                origin: Some("demo".to_owned()),
                sort_text: "0".to_owned(),
                detail: Some("fun() -> ()".to_owned()),
                docs: None,
                filter_text: None,
                text_edit: None,
                insert_format: CompletionInsertFormat::PlainText,
                file_id: None,
                exported: true,
                resolve_data: None,
            },
        );

        assert_eq!(
            item.label_details
                .as_ref()
                .and_then(|details| details.description.as_deref()),
            Some("module export · demo")
        );
    }

    #[test]
    fn signature_help_conversion_sets_active_parameter_per_signature() {
        let help = signature_help_to_lsp(IdeSignatureHelp {
            signatures: vec![IdeSignatureInformation {
                label: "fn check(left: int, right: string) -> bool".to_owned(),
                docs: Some("check docs".to_owned()),
                parameters: vec![
                    IdeSignatureParameter {
                        label: "left".to_owned(),
                        annotation: Some("int".to_owned()),
                    },
                    IdeSignatureParameter {
                        label: "right".to_owned(),
                        annotation: Some("string".to_owned()),
                    },
                ],
                file_id: None,
            }],
            active_signature: 0,
            active_parameter: 1,
        });

        assert_eq!(help.active_parameter, Some(1));
        assert_eq!(help.signatures[0].active_parameter, Some(1));
    }
}
