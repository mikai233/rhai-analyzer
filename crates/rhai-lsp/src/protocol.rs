use lsp_types::{
    self, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, CodeAction,
    CodeActionKind, CodeActionOrCommand, CompletionItem, CompletionItemKind, Diagnostic,
    DocumentChangeOperation, DocumentChanges, DocumentHighlight, DocumentHighlightKind,
    DocumentSymbol, Documentation, GotoDefinitionResponse, Hover, HoverContents, InlayHint,
    InlayHintKind, Location, MarkupContent, MarkupKind, OneOf,
    OptionalVersionedTextDocumentIdentifier, Position, Range, RenameFile, ResourceOp,
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
    SignatureHelp as IdeSignatureHelp, SignatureParameter as IdeSignatureParameter, SourceChange,
};
use rhai_syntax::{TextRange, TextSize};
use rhai_vfs::FileId;
use serde::{Deserialize, Serialize};

use crate::state::{CodeActionEdit, ServerState, WorkspaceSymbolMatch};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompletionResolvePayload {
    pub label: String,
    pub kind: CompletionKindPayload,
    pub source: CompletionSourcePayload,
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
    text: Option<&str>,
    item: IdeCompletionItem,
) -> CompletionItem {
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
        label_details: None,
        tags: None,
    }
}

pub(crate) fn completion_item_from_lsp(item: CompletionItem) -> Option<IdeCompletionItem> {
    let payload = serde_json::from_value::<CompletionResolvePayload>(item.data.clone()?).ok()?;

    Some(IdeCompletionItem {
        label: payload.label,
        kind: completion_kind_from_payload(payload.kind),
        source: completion_source_from_payload(payload.source),
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
    let mut lines = vec![format!("```rhai\n{}\n```", hover.signature)];

    if let Some(docs) = hover.docs {
        lines.push(docs);
    }

    lines.push(format!(
        "_Source: {}_",
        match hover.source {
            HoverSignatureSource::Declared => "declared",
            HoverSignatureSource::Inferred => "inferred",
            HoverSignatureSource::Structural => "structural",
        }
    ));

    if let Some(declared) = hover.declared_signature {
        lines.push(format!("Declared: `{declared}`"));
    }
    if let Some(inferred) = hover.inferred_signature {
        lines.push(format!("Inferred: `{inferred}`"));
    }
    for note in hover.notes {
        lines.push(format!("- {note}"));
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
    SignatureHelp {
        signatures: help
            .signatures
            .into_iter()
            .map(|signature| SignatureInformation {
                label: signature.label,
                documentation: signature.docs.map(markdown_documentation),
                parameters: Some(
                    signature
                        .parameters
                        .into_iter()
                        .map(signature_parameter_to_lsp)
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect(),
        active_signature: Some(help.active_signature as u32),
        active_parameter: Some(help.active_parameter as u32),
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

pub(crate) fn code_action_to_lsp(
    server: &ServerState,
    action: &CodeActionEdit,
) -> Option<CodeActionOrCommand> {
    let text = open_document_text_by_uri(server, &action.uri)?;
    let position = offset_to_position(text.as_ref(), action.insert_offset as usize)?;
    let range = Range {
        start: position,
        end: position,
    };
    let edit = TextEdit {
        range,
        new_text: action.insert_text.clone(),
    };

    Some(
        CodeAction {
            title: action.title.clone(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: None,
                document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: action.uri.clone(),
                        version: action.version,
                    },
                    edits: vec![OneOf::Left(edit)],
                }])),
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }
        .into(),
    )
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
        IdeCompletionItemSource::Member => CompletionSourcePayload::Member,
        IdeCompletionItemSource::Builtin => CompletionSourcePayload::Builtin,
        IdeCompletionItemSource::Postfix => CompletionSourcePayload::Postfix,
    }
}

fn completion_source_from_payload(source: CompletionSourcePayload) -> IdeCompletionItemSource {
    match source {
        CompletionSourcePayload::Visible => IdeCompletionItemSource::Visible,
        CompletionSourcePayload::Project => IdeCompletionItemSource::Project,
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
    let document = server.open_documents.get(uri)?;
    let file_id = server
        .analysis_host()
        .snapshot()
        .file_id_for_path(&document.normalized_path)?;
    server.analysis_host().snapshot().file_text(file_id)
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

fn text_range_to_lsp_range(text: &str, range: TextRange) -> Option<Range> {
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
        character: utf16_len(&text[line_start..offset]) as u32,
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
