use rhai_db::DatabaseSnapshot;
use rhai_hir::{FunctionTypeRef, SymbolId, SymbolKind, TypeRef};
use rhai_vfs::FileId;

use crate::CompletionItemKind;
use crate::completion::CompletionContext;
use crate::support::convert::format_type_ref;
use crate::types::CompletionTextEdit;

pub(super) fn callable_completion_text_edit(
    context: &CompletionContext,
    label: &str,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
    parameter_names: Option<&[String]>,
) -> Option<CompletionTextEdit> {
    callable_completion_text_edit_with_base_text(context, label, annotation, kind, parameter_names)
}

pub(super) fn callable_completion_text_edit_with_base_text(
    context: &CompletionContext,
    base_text: &str,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
    parameter_names: Option<&[String]>,
) -> Option<CompletionTextEdit> {
    if context.next_char_is_open_paren {
        return None;
    }

    let snippet = completion_call_snippet(base_text, annotation, kind, parameter_names)?;
    Some(CompletionTextEdit {
        replace_range: context.replace_range,
        insert_range: None,
        new_text: snippet,
        additional_edits: Vec::new(),
    })
}

fn completion_call_snippet(
    label: &str,
    annotation: Option<&TypeRef>,
    kind: CompletionItemKind,
    parameter_names: Option<&[String]>,
) -> Option<String> {
    match annotation {
        Some(TypeRef::Function(signature)) => {
            Some(function_call_snippet(label, signature, parameter_names))
        }
        _ if matches!(kind, CompletionItemKind::Symbol(SymbolKind::Function)) => {
            Some(format!("{label}()$0"))
        }
        _ => None,
    }
}

fn function_call_snippet(
    label: &str,
    signature: &FunctionTypeRef,
    parameter_names: Option<&[String]>,
) -> String {
    if signature.params.is_empty() {
        return format!("{label}()$0");
    }

    let placeholders = signature
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let tabstop = index + 1;
            let placeholder = snippet_placeholder(
                parameter,
                parameter_names.and_then(|names| names.get(index).map(String::as_str)),
                tabstop,
            );
            format!("${{{tabstop}:{placeholder}}}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{label}({placeholders})$0")
}

fn snippet_placeholder(parameter: &TypeRef, parameter_name: Option<&str>, index: usize) -> String {
    if let Some(name) = parameter_name
        && !name.is_empty()
    {
        return name.to_owned();
    }

    let label = format_type_ref(parameter);
    if label.is_empty() || label == "unknown" {
        format!("arg{index}")
    } else {
        label
    }
}

pub(super) fn callable_parameter_names(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    symbol: SymbolId,
    annotation: Option<&TypeRef>,
) -> Option<Vec<String>> {
    let TypeRef::Function(signature) = annotation? else {
        return None;
    };
    let hir = snapshot.hir(file_id)?;
    let symbol_data = hir.symbol(symbol);
    if symbol_data.kind != SymbolKind::Function {
        return None;
    }

    let names = hir
        .function_parameters(symbol)
        .into_iter()
        .map(|parameter| hir.symbol(parameter).name.clone())
        .collect::<Vec<_>>();
    (names.len() == signature.params.len()).then_some(names)
}
