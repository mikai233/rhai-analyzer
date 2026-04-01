use lsp_types::{Documentation, MarkupContent, MarkupKind, SymbolKind};
use rhai_hir::SymbolKind as HirSymbolKind;
use serde::{Deserialize, Serialize};

mod completion;
mod diagnostics;
mod hover;
mod symbols;
mod text;

#[cfg(test)]
mod tests;

pub(crate) use completion::{completion_item_from_lsp, completion_item_to_lsp};
pub(crate) use diagnostics::diagnostic_to_lsp;
pub(crate) use hover::hover_to_lsp;
#[cfg(test)]
pub(crate) use symbols::source_change_code_action_to_lsp;
pub(crate) use symbols::{
    CodeActionResolvePayload, call_hierarchy_item_from_lsp, call_hierarchy_item_to_lsp,
    document_highlight_to_lsp, document_symbols_to_lsp, goto_definition_response,
    incoming_call_to_lsp, inlay_hint_to_lsp, outgoing_call_to_lsp, prepared_rename_to_lsp,
    references_to_lsp, rename_to_workspace_edit, resolve_code_action_payload,
    resolved_code_action_to_lsp, semantic_tokens_delta_result, semantic_tokens_result,
    signature_help_to_lsp, unresolved_code_action_to_lsp, workspace_symbols_to_lsp,
};
pub(crate) use text::{file_text_by_uri, open_document_text_by_uri, text_range_to_lsp_range};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum SymbolKindPayload {
    Variable,
    Parameter,
    Constant,
    Function,
    ImportAlias,
    ExportAlias,
}

fn markup(value: String) -> MarkupContent {
    MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    }
}

fn markdown_documentation(value: String) -> Documentation {
    Documentation::MarkupContent(markup(value))
}

fn documentation_text(documentation: Option<Documentation>) -> Option<String> {
    match documentation? {
        Documentation::String(text) => Some(text),
        Documentation::MarkupContent(markup) => Some(markup.value),
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
