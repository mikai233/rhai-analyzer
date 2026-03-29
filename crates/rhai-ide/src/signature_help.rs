use rhai_db::{
    DatabaseSnapshot, LinkedModuleImport, LocatedNavigationTarget, best_matching_signature_index,
};
use rhai_hir::{CallSite, FileHir, FunctionTypeRef, ParameterHint, SymbolId, SymbolKind, TypeRef};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

use crate::convert::format_type_ref;
use crate::{SignatureHelp, SignatureInformation, SignatureParameter};

pub(crate) fn signature_help(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    offset: TextSize,
) -> Option<SignatureHelp> {
    let hir = snapshot.hir(file_id)?;
    let call_id = hir.call_at_offset(offset)?;
    let call = hir.call(call_id);
    let active_parameter = active_parameter_index(call, offset);

    if let Some(parameter_hint) = hir.parameter_hint_at(offset) {
        return Some(signature_help_from_parameter_hint(
            file_id,
            hir.as_ref(),
            parameter_hint,
        ));
    }

    if let Some(target) = resolved_signature_target(snapshot, file_id, hir.as_ref(), call) {
        return Some(signature_help_from_target(
            target.file_id,
            target.hir.as_ref(),
            target.symbol,
            active_parameter,
        ));
    }

    if let Some(help) =
        signature_help_from_host_method(snapshot, file_id, hir.as_ref(), call, active_parameter)
    {
        return Some(help);
    }

    let callee_name = call
        .callee_reference
        .map(|reference_id| hir.reference(reference_id).name.clone())
        .unwrap_or_else(|| "call".to_owned());

    if let Some(function) = snapshot.global_function(&callee_name) {
        let arg_types = call_argument_types(snapshot, file_id, hir.as_ref(), call);
        return signature_help_from_host_function(
            function,
            &callee_name,
            &arg_types,
            active_parameter,
        );
    }

    let signature = hir.call_signature(call_id, Some(snapshot.external_signatures()))?;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: format_signature_label(&callee_name, &signature.params, signature.ret.as_ref()),
            docs: None,
            parameters: signature
                .params
                .iter()
                .enumerate()
                .map(|(index, parameter)| SignatureParameter {
                    label: format!("arg{}", index + 1),
                    annotation: Some(format_type_ref(parameter)),
                })
                .collect(),
            file_id: None,
        }],
        active_signature: 0,
        active_parameter,
    })
}

fn signature_help_from_host_function(
    function: &rhai_db::HostFunction,
    name: &str,
    arg_types: &[Option<TypeRef>],
    active_parameter: usize,
) -> Option<SignatureHelp> {
    let overload_signatures = function
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .collect::<Vec<_>>();
    let signatures = function
        .overloads
        .iter()
        .filter_map(|overload| {
            let signature = overload.signature.as_ref()?;
            Some(SignatureInformation {
                label: format_signature_label(name, &signature.params, signature.ret.as_ref()),
                docs: overload.docs.clone(),
                parameters: signature
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| SignatureParameter {
                        label: format!("arg{}", index + 1),
                        annotation: Some(format_type_ref(parameter)),
                    })
                    .collect(),
                file_id: None,
            })
        })
        .collect::<Vec<_>>();

    if signatures.is_empty() {
        return None;
    }

    let active_signature =
        best_matching_signature_index(overload_signatures.iter().copied(), arg_types)
            .or_else(|| {
                signatures
                    .iter()
                    .position(|signature| signature.parameters.len() == arg_types.len())
            })
            .unwrap_or(0);

    Some(SignatureHelp {
        signatures,
        active_signature,
        active_parameter,
    })
}

fn signature_help_from_host_method(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    call: &CallSite,
    active_parameter: usize,
) -> Option<SignatureHelp> {
    let callee_expr = call.callee_range.and_then(|range| hir.expr_at(range))?;
    let access = hir.member_access(callee_expr)?;
    let method_name = hir.reference(access.field_reference).name.as_str();
    let inference = snapshot.type_inference(file_id)?;
    let receiver_ty = hir.expr_type(access.receiver, &inference.expr_types)?;
    let arg_types = call_argument_types(snapshot, file_id, hir, call);
    let candidates =
        host_method_candidates_for_type(snapshot.host_types(), receiver_ty, method_name);

    if candidates.is_empty() {
        return None;
    }

    let signatures = candidates
        .iter()
        .map(|candidate| SignatureInformation {
            label: format_signature_label(
                method_name,
                &candidate.signature.params,
                candidate.signature.ret.as_ref(),
            ),
            docs: candidate.docs.clone(),
            parameters: candidate
                .signature
                .params
                .iter()
                .enumerate()
                .map(|(index, parameter)| SignatureParameter {
                    label: format!("arg{}", index + 1),
                    annotation: Some(format_type_ref(parameter)),
                })
                .collect(),
            file_id: None,
        })
        .collect::<Vec<_>>();

    let active_signature = best_matching_signature_index(
        candidates.iter().map(|candidate| &candidate.signature),
        &arg_types,
    )
    .or_else(|| {
        candidates
            .iter()
            .position(|candidate| candidate.signature.params.len() == call.arg_ranges.len())
    })
    .unwrap_or(0);

    Some(SignatureHelp {
        signatures,
        active_signature,
        active_parameter,
    })
}

fn call_argument_types(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    call: &CallSite,
) -> Vec<Option<TypeRef>> {
    let Some(inference) = snapshot.type_inference(file_id) else {
        return vec![None; call.arg_exprs.len()];
    };

    call.arg_exprs
        .iter()
        .map(|expr| {
            inference
                .expr_types
                .get(hir.expr_result_slot(*expr))
                .cloned()
        })
        .collect()
}

#[derive(Clone)]
struct HostMethodCandidate {
    signature: FunctionTypeRef,
    docs: Option<String>,
}

fn host_method_candidates_for_type(
    host_types: &[rhai_db::HostType],
    ty: &TypeRef,
    method_name: &str,
) -> Vec<HostMethodCandidate> {
    let mut candidates = Vec::new();
    collect_host_method_candidates(&mut candidates, host_types, ty, method_name);
    candidates
}

fn collect_host_method_candidates(
    candidates: &mut Vec<HostMethodCandidate>,
    host_types: &[rhai_db::HostType],
    ty: &TypeRef,
    method_name: &str,
) {
    match ty {
        TypeRef::Union(items) => {
            for item in items {
                collect_host_method_candidates(candidates, host_types, item, method_name);
            }
        }
        TypeRef::Named(name) | TypeRef::Applied { name, .. } => {
            let Some(host_type) = host_types.iter().find(|ty| ty.name == *name) else {
                return;
            };
            let Some(method) = host_type
                .methods
                .iter()
                .find(|method| method.name == method_name)
            else {
                return;
            };

            for overload in &method.overloads {
                let Some(signature) = overload.signature.clone() else {
                    continue;
                };
                let candidate = HostMethodCandidate {
                    signature,
                    docs: overload.docs.clone(),
                };
                if !candidates.iter().any(|existing| {
                    existing.signature == candidate.signature && existing.docs == candidate.docs
                }) {
                    candidates.push(candidate);
                }
            }
        }
        _ => {}
    }
}

struct SignatureTarget {
    file_id: Option<FileId>,
    hir: std::sync::Arc<FileHir>,
    symbol: SymbolId,
}

fn resolved_signature_target(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    call: &CallSite,
) -> Option<SignatureTarget> {
    if let Some(symbol) = call.resolved_callee {
        match hir.symbol(symbol).kind {
            SymbolKind::Function => {
                return Some(SignatureTarget {
                    file_id: Some(file_id),
                    hir: snapshot.hir(file_id)?,
                    symbol,
                });
            }
            SymbolKind::ImportAlias => {
                if let Some(target) =
                    signature_target_for_import_alias(snapshot, file_id, hir, symbol)
                {
                    return Some(target);
                }
            }
            _ => {}
        }
    }

    let callee_range = call.callee_range?;
    let target = snapshot
        .goto_definition(file_id, callee_range.start())
        .into_iter()
        .next()?;
    signature_target_from_navigation(snapshot, target)
}

fn signature_target_for_import_alias(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &FileHir,
    alias_symbol: SymbolId,
) -> Option<SignatureTarget> {
    let import_index = hir
        .imports
        .iter()
        .position(|import| import.alias == Some(alias_symbol))?;
    let linked_import = snapshot.linked_import(file_id, import_index)?;
    export_signature_target(snapshot, linked_import)
}

fn export_signature_target(
    snapshot: &DatabaseSnapshot,
    linked_import: &LinkedModuleImport,
) -> Option<SignatureTarget> {
    let export = linked_import.exports.first()?;
    let identity = export
        .export
        .target
        .as_ref()
        .or(export.export.alias.as_ref())?;
    let location = snapshot.locate_symbol(identity).first()?.clone();
    let hir = snapshot.hir(location.file_id)?;
    let symbol = hir.symbol_at(location.symbol.declaration_range)?;
    (hir.symbol(symbol).kind == SymbolKind::Function).then_some(SignatureTarget {
        file_id: Some(location.file_id),
        hir,
        symbol,
    })
}

fn signature_target_from_navigation(
    snapshot: &DatabaseSnapshot,
    target: LocatedNavigationTarget,
) -> Option<SignatureTarget> {
    let hir = snapshot.hir(target.file_id)?;
    let symbol = hir.symbol_at(target.target.full_range)?;
    match hir.symbol(symbol).kind {
        SymbolKind::Function => Some(SignatureTarget {
            file_id: Some(target.file_id),
            hir,
            symbol,
        }),
        SymbolKind::ImportAlias => {
            signature_target_for_import_alias(snapshot, target.file_id, hir.as_ref(), symbol)
        }
        _ => None,
    }
}

fn signature_help_from_target(
    file_id: Option<FileId>,
    hir: &FileHir,
    symbol: SymbolId,
    active_parameter: usize,
) -> SignatureHelp {
    let function = hir.symbol(symbol);
    let docs = function.docs.map(|docs| hir.doc_block(docs).text.clone());
    let parameters = hir
        .function_parameters(symbol)
        .into_iter()
        .map(|parameter_id| {
            let parameter = hir.symbol(parameter_id);
            SignatureParameter {
                label: parameter.name.clone(),
                annotation: parameter.annotation.as_ref().map(format_type_ref),
            }
        })
        .collect::<Vec<_>>();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: format_function_signature_label(
                function.name.as_str(),
                &parameters,
                function.annotation.as_ref(),
            ),
            docs,
            parameters,
            file_id,
        }],
        active_signature: 0,
        active_parameter,
    }
}

fn signature_help_from_parameter_hint(
    file_id: FileId,
    hir: &FileHir,
    parameter_hint: ParameterHint,
) -> SignatureHelp {
    let function = hir.symbol(parameter_hint.callee.symbol);
    let docs = function.docs.map(|docs| hir.doc_block(docs).text.clone());
    let parameters = parameter_hint
        .parameters
        .into_iter()
        .map(|parameter| SignatureParameter {
            label: parameter.name,
            annotation: parameter.annotation.as_ref().map(format_type_ref),
        })
        .collect::<Vec<_>>();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: format_function_signature_label(
                parameter_hint.callee_name.as_str(),
                &parameters,
                function.annotation.as_ref(),
            ),
            docs,
            parameters,
            file_id: Some(file_id),
        }],
        active_signature: 0,
        active_parameter: parameter_hint.active_parameter,
    }
}

fn active_parameter_index(call: &CallSite, offset: TextSize) -> usize {
    if call.arg_ranges.is_empty() {
        return 0;
    }

    let mut index = 0usize;
    for (current, range) in call.arg_ranges.iter().enumerate() {
        if range.contains(offset) {
            return current;
        }

        if offset >= range.start() {
            index = current;
        }
    }

    index
}

fn format_signature_label(name: &str, params: &[TypeRef], ret: &TypeRef) -> String {
    format!(
        "fn {name}({}) -> {}",
        params
            .iter()
            .map(format_type_ref)
            .collect::<Vec<_>>()
            .join(", "),
        format_type_ref(ret)
    )
}

fn format_function_signature_label(
    name: &str,
    parameters: &[SignatureParameter],
    annotation: Option<&TypeRef>,
) -> String {
    let params = parameters
        .iter()
        .map(|parameter| match &parameter.annotation {
            Some(annotation) => format!("{}: {annotation}", parameter.label),
            None => parameter.label.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret = match annotation {
        Some(TypeRef::Function(signature)) => {
            format!(" -> {}", format_type_ref(signature.ret.as_ref()))
        }
        _ => String::new(),
    };

    format!("fn {name}({params}){ret}")
}
