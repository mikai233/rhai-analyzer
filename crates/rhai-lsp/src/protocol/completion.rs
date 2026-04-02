use lsp_types::{
    self, CompletionItem, CompletionItemKind, CompletionItemLabelDetails, InsertReplaceEdit,
    TextEdit,
};
use rhai_hir::SymbolKind as HirSymbolKind;
use rhai_ide::{
    CompletionInsertFormat as IdeCompletionInsertFormat, CompletionItem as IdeCompletionItem,
    CompletionItemKind as IdeCompletionItemKind, CompletionItemSource as IdeCompletionItemSource,
    CompletionRelevance, CompletionResolveData,
};
use rhai_vfs::FileId;
use serde::{Deserialize, Serialize};

use crate::state::ServerState;

use crate::protocol::{
    SymbolKindPayload, documentation_text, symbol_kind_payload, text_range_to_lsp_range,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompletionResolvePayload {
    pub label: String,
    pub kind: CompletionKindPayload,
    pub source: CompletionSourcePayload,
    pub origin: Option<String>,
    pub signature_detail: Option<String>,
    pub sort_text: String,
    pub file_id: Option<u32>,
    pub exported: bool,
    pub resolve_file_id: u32,
    pub resolve_offset: u32,
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
    Snippet,
    Type,
}

pub(crate) fn completion_item_to_lsp(
    server: &ServerState,
    text: Option<&str>,
    item: IdeCompletionItem,
) -> CompletionItem {
    let fallback_filter_text = item.label.clone();
    let filter_text = match item.source {
        IdeCompletionItemSource::Postfix => Some(fallback_filter_text.clone()),
        _ => item
            .filter_text
            .clone()
            .or(Some(fallback_filter_text.clone())),
    };
    let label_details = completion_label_details(&item);
    let source_detail = completion_source_description(server, &item);
    let documentation = item.docs.map(crate::protocol::markdown_documentation);
    let mut additional_text_edits = None;
    let text_edit = text.zip(item.text_edit.as_ref()).and_then(|(text, edit)| {
        if matches!(item.source, IdeCompletionItemSource::Postfix)
            && let Some(insert_range) = edit.insert_range
        {
            let source_range = text_range_to_lsp_range(text, insert_range)?;
            if edit.replace_range.start() < insert_range.start() {
                let delete_prefix = text_range_to_lsp_range(
                    text,
                    rhai_syntax::TextRange::new(edit.replace_range.start(), insert_range.start()),
                )?;
                additional_text_edits = Some(vec![TextEdit {
                    range: delete_prefix,
                    new_text: String::new(),
                }]);
            }
            return Some(lsp_types::CompletionTextEdit::Edit(TextEdit {
                range: source_range,
                new_text: edit.new_text.clone(),
            }));
        }

        let replace = text_range_to_lsp_range(text, edit.replace_range)?;
        Some(match edit.insert_range {
            Some(insert_range) => {
                let insert = text_range_to_lsp_range(text, insert_range)?;
                lsp_types::CompletionTextEdit::InsertAndReplace(InsertReplaceEdit {
                    new_text: edit.new_text.clone(),
                    insert,
                    replace,
                })
            }
            None => lsp_types::CompletionTextEdit::Edit(TextEdit {
                range: replace,
                new_text: edit.new_text.clone(),
            }),
        })
    });
    let data = item.resolve_data.as_ref().map(|resolve_data| {
        serde_json::to_value(CompletionResolvePayload {
            label: item.label.clone(),
            kind: completion_kind_payload(item.kind),
            source: completion_source_payload(item.source),
            origin: item.origin.clone(),
            signature_detail: item.detail.clone(),
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
        detail: source_detail,
        documentation,
        sort_text: Some(item.sort_text),
        insert_text: None,
        insert_text_format: Some(match item.insert_format {
            IdeCompletionInsertFormat::PlainText => lsp_types::InsertTextFormat::PLAIN_TEXT,
            IdeCompletionInsertFormat::Snippet => lsp_types::InsertTextFormat::SNIPPET,
        }),
        insert_text_mode: None,
        text_edit,
        additional_text_edits,
        command: None,
        commit_characters: None,
        data,
        deprecated: None,
        preselect: None,
        filter_text,
        label_details,
        tags: None,
    }
}

pub(crate) fn completion_item_from_lsp(item: CompletionItem) -> Option<IdeCompletionItem> {
    let payload = serde_json::from_value::<CompletionResolvePayload>(item.data.clone()?).ok()?;
    let signature_detail = item
        .label_details
        .as_ref()
        .and_then(|details| details.detail.as_deref())
        .map(str::trim)
        .filter(|detail| !detail.is_empty())
        .map(ToOwned::to_owned);

    Some(IdeCompletionItem {
        label: payload.label,
        kind: completion_kind_from_payload(payload.kind),
        source: completion_source_from_payload(payload.source),
        origin: payload.origin,
        sort_text: payload.sort_text,
        detail: payload
            .signature_detail
            .or(signature_detail)
            .or(item.detail),
        docs: documentation_text(item.documentation),
        filter_text: item.filter_text,
        text_edit: None,
        insert_format: match item.insert_text_format {
            Some(lsp_types::InsertTextFormat::SNIPPET) => IdeCompletionInsertFormat::Snippet,
            _ => IdeCompletionInsertFormat::PlainText,
        },
        relevance: CompletionRelevance::default(),
        file_id: payload.file_id.map(FileId),
        exported: payload.exported,
        resolve_data: Some(CompletionResolveData {
            file_id: FileId(payload.resolve_file_id),
            offset: payload.resolve_offset,
        }),
    })
}

fn completion_label_details(item: &IdeCompletionItem) -> Option<CompletionItemLabelDetails> {
    let detail = item.detail.as_ref().map(|detail| format!(" {detail}"));
    detail.as_ref()?;

    Some(CompletionItemLabelDetails {
        detail,
        description: None,
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

fn completion_item_kind(kind: IdeCompletionItemKind) -> CompletionItemKind {
    match kind {
        IdeCompletionItemKind::Member => CompletionItemKind::METHOD,
        IdeCompletionItemKind::Keyword => CompletionItemKind::KEYWORD,
        IdeCompletionItemKind::Snippet => CompletionItemKind::SNIPPET,
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
        IdeCompletionItemKind::Snippet => CompletionKindPayload::Snippet,
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
        CompletionKindPayload::Snippet => IdeCompletionItemKind::Snippet,
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
