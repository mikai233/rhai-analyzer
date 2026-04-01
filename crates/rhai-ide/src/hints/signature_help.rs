use rhai_db::{
    DatabaseSnapshot, LocatedNavigationTarget, best_matching_signature_index,
    builtin_universal_method_signature, specialize_signature_with_receiver_and_arg_types,
};
use rhai_hir::{CallSite, FileHir, FunctionTypeRef, ParameterHint, SymbolId, SymbolKind, TypeRef};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

use crate::support::convert::format_type_ref;
use crate::{SignatureHelp, SignatureInformation, SignatureParameter};

pub(crate) fn signature_help(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    offset: TextSize,
) -> Option<SignatureHelp> {
    let hir = snapshot.hir(file_id)?;
    let call_id = hir.call_at_offset(offset)?;
    let call = hir.call(call_id);
    let active_parameter = active_parameter_index(hir.as_ref(), call, offset)?;

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
        signature_help_from_local_method(snapshot, file_id, hir.as_ref(), call, active_parameter)
    {
        return Some(help);
    }

    if let Some(help) = signature_help_from_imported_global_method(
        snapshot,
        file_id,
        hir.as_ref(),
        call,
        active_parameter,
    ) {
        return Some(help);
    }

    if let Some(help) =
        signature_help_from_builtin_universal_method(hir.as_ref(), call, active_parameter)
    {
        return Some(help);
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

    let external = snapshot.effective_external_signatures(file_id);
    let signature = hir.call_signature(call_id, Some(&external))?;

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
    let candidates = host_method_candidates_for_type(
        snapshot.host_types(),
        receiver_ty,
        method_name,
        &arg_types,
    );

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
pub(crate) struct HostMethodCandidate {
    pub(crate) signature: FunctionTypeRef,
    pub(crate) docs: Option<String>,
}

#[derive(Clone)]
struct LocalMethodCandidate {
    symbol: SymbolId,
    signature: FunctionTypeRef,
    docs: Option<String>,
}

fn signature_help_from_local_method(
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
    let candidates = local_method_candidates_for_type(hir, receiver_ty, method_name);

    if candidates.is_empty() {
        return None;
    }

    let signatures = candidates
        .iter()
        .map(|candidate| {
            let parameters = hir
                .function_parameters(candidate.symbol)
                .into_iter()
                .map(|parameter_id| {
                    let parameter = hir.symbol(parameter_id);
                    SignatureParameter {
                        label: parameter.name.clone(),
                        annotation: parameter.annotation.as_ref().map(format_type_ref),
                    }
                })
                .collect::<Vec<_>>();

            SignatureInformation {
                label: format_function_signature_label(
                    method_name,
                    &parameters,
                    hir.symbol(candidate.symbol).annotation.as_ref(),
                ),
                docs: candidate.docs.clone(),
                parameters,
                file_id: Some(file_id),
            }
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

fn signature_help_from_imported_global_method(
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
    let candidates = snapshot.imported_global_method_symbols(file_id, receiver_ty, method_name);

    if candidates.is_empty() {
        return None;
    }

    let arg_types = call_argument_types(snapshot, file_id, hir, call);
    let mut signatures = Vec::new();
    let mut overloads = Vec::new();

    for candidate in candidates {
        let provider_hir = snapshot.hir(candidate.file_id)?;
        let symbol = provider_hir.symbol(candidate.symbol.symbol);
        let parameters = provider_hir
            .function_parameters(candidate.symbol.symbol)
            .into_iter()
            .map(|parameter_id| {
                let parameter = provider_hir.symbol(parameter_id);
                SignatureParameter {
                    label: parameter.name.clone(),
                    annotation: parameter.annotation.as_ref().map(format_type_ref),
                }
            })
            .collect::<Vec<_>>();
        let signature = match symbol.annotation.as_ref() {
            Some(TypeRef::Function(signature)) => signature.clone(),
            _ => FunctionTypeRef {
                params: parameters
                    .iter()
                    .map(|parameter| {
                        parameter
                            .annotation
                            .as_deref()
                            .and_then(rhai_hir::parse_type_ref)
                            .unwrap_or(TypeRef::Unknown)
                    })
                    .collect(),
                ret: Box::new(TypeRef::Unknown),
            },
        };

        overloads.push(signature);
        signatures.push(SignatureInformation {
            label: format_function_signature_label(
                method_name,
                &parameters,
                symbol.annotation.as_ref(),
            ),
            docs: symbol
                .docs
                .map(|docs| provider_hir.doc_block(docs).text.clone()),
            parameters,
            file_id: Some(candidate.file_id),
        });
    }

    let active_signature = best_matching_signature_index(overloads.iter(), &arg_types)
        .or_else(|| {
            overloads
                .iter()
                .position(|signature| signature.params.len() == call.arg_ranges.len())
        })
        .unwrap_or(0);

    Some(SignatureHelp {
        signatures,
        active_signature,
        active_parameter,
    })
}

fn signature_help_from_builtin_universal_method(
    hir: &FileHir,
    call: &CallSite,
    active_parameter: usize,
) -> Option<SignatureHelp> {
    let callee_expr = call.callee_range.and_then(|range| hir.expr_at(range))?;
    let access = hir.member_access(callee_expr)?;
    let method_name = hir.reference(access.field_reference).name.as_str();
    let signature = builtin_universal_method_signature(method_name)?;

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: format_signature_label(method_name, &signature.params, signature.ret.as_ref()),
            docs: None,
            parameters: Vec::new(),
            file_id: None,
        }],
        active_signature: 0,
        active_parameter,
    })
}

pub(crate) fn host_method_candidates_for_type(
    host_types: &[rhai_db::HostType],
    ty: &TypeRef,
    method_name: &str,
    arg_types: &[Option<TypeRef>],
) -> Vec<HostMethodCandidate> {
    let mut candidates = Vec::new();
    collect_host_method_candidates(&mut candidates, host_types, ty, method_name, arg_types);
    candidates
}

fn collect_host_method_candidates(
    candidates: &mut Vec<HostMethodCandidate>,
    host_types: &[rhai_db::HostType],
    ty: &TypeRef,
    method_name: &str,
    arg_types: &[Option<TypeRef>],
) {
    match ty {
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                collect_host_method_candidates(
                    candidates,
                    host_types,
                    item,
                    method_name,
                    arg_types,
                );
            }
        }
        _ if builtin_host_type_name(ty).is_some() => {
            let Some(host_type_name) = builtin_host_type_name(ty) else {
                return;
            };
            let Some(host_type) = host_types.iter().find(|ty| ty.name == host_type_name) else {
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
                let Some(signature) = overload.signature.as_ref() else {
                    continue;
                };
                let candidate = HostMethodCandidate {
                    signature: specialize_signature_with_receiver_and_arg_types(
                        signature,
                        Some(ty),
                        host_type.generic_params.as_slice(),
                        Some(arg_types),
                        host_types,
                    ),
                    docs: overload.docs.clone(),
                };
                if !candidates.iter().any(|existing| {
                    existing.signature == candidate.signature && existing.docs == candidate.docs
                }) {
                    candidates.push(candidate);
                }
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
                let Some(signature) = overload.signature.as_ref() else {
                    continue;
                };
                let candidate = HostMethodCandidate {
                    signature: specialize_signature_with_receiver_and_arg_types(
                        signature,
                        Some(ty),
                        host_type.generic_params.as_slice(),
                        Some(arg_types),
                        host_types,
                    ),
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

fn builtin_host_type_name(ty: &TypeRef) -> Option<&'static str> {
    match ty {
        TypeRef::Int => Some("int"),
        TypeRef::Float => Some("float"),
        TypeRef::Char => Some("char"),
        TypeRef::String => Some("string"),
        TypeRef::Array(_) => Some("array"),
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some("map"),
        TypeRef::Blob => Some("blob"),
        TypeRef::Timestamp => Some("timestamp"),
        TypeRef::Range => Some("range"),
        TypeRef::RangeInclusive => Some("range="),
        _ => None,
    }
}

fn local_method_candidates_for_type(
    hir: &FileHir,
    ty: &TypeRef,
    method_name: &str,
) -> Vec<LocalMethodCandidate> {
    let mut blanket = Vec::new();
    let mut typed = Vec::new();

    for symbol in &hir.symbols {
        if symbol.kind != SymbolKind::Function || symbol.name != method_name {
            continue;
        }

        let symbol_id = hir
            .symbol_at(symbol.range)
            .expect("function symbol should map back to a symbol id");
        let Some(signature) = symbol.annotation.as_ref().and_then(|ty| match ty {
            TypeRef::Function(signature) => Some(signature.clone()),
            _ => None,
        }) else {
            continue;
        };

        let candidate = LocalMethodCandidate {
            symbol: symbol_id,
            signature,
            docs: symbol.docs.map(|docs| hir.doc_block(docs).text.clone()),
        };

        match hir
            .function_info(symbol_id)
            .and_then(|info| info.this_type.as_ref())
        {
            Some(this_type) if receiver_matches_method_type(ty, this_type) => typed.push(candidate),
            Some(_) => {}
            None => blanket.push(candidate),
        }
    }

    if typed.is_empty() {
        return blanket;
    }
    if receiver_dispatch_is_precise(ty) {
        return typed;
    }

    typed.extend(blanket);
    typed
}

fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) | (TypeRef::Ambiguous(items), expected) => items
            .iter()
            .any(|item| receiver_matches_method_type(item, expected)),
        (TypeRef::Nullable(inner), expected) => receiver_matches_method_type(inner, expected),
        (TypeRef::Applied { name, .. }, TypeRef::Named(expected_name))
        | (
            TypeRef::Named(name),
            TypeRef::Applied {
                name: expected_name,
                ..
            },
        ) => name == expected_name,
        (
            TypeRef::Applied { name, args },
            TypeRef::Applied {
                name: expected_name,
                args: expected_args,
            },
        ) => {
            name == expected_name
                && args.len() == expected_args.len()
                && args
                    .iter()
                    .zip(expected_args.iter())
                    .all(|(arg, expected)| receiver_matches_method_type(arg, expected))
        }
        _ => false,
    }
}

fn receiver_dispatch_is_precise(receiver: &TypeRef) -> bool {
    !matches!(
        receiver,
        TypeRef::Unknown
            | TypeRef::Any
            | TypeRef::Dynamic
            | TypeRef::Never
            | TypeRef::Union(_)
            | TypeRef::Ambiguous(_)
    )
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
    if let Some(symbol) = call.resolved_callee
        && hir.symbol(symbol).kind == SymbolKind::Function
    {
        return Some(SignatureTarget {
            file_id: Some(file_id),
            hir: snapshot.hir(file_id)?,
            symbol,
        });
    }

    let callee_range = call.callee_range?;
    let callee_offset = callee_range
        .end()
        .checked_sub(TextSize::from(1))
        .unwrap_or(callee_range.start());
    let target = snapshot
        .goto_definition(file_id, callee_offset)
        .into_iter()
        .next()?;
    signature_target_from_navigation(snapshot, target)
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

fn caller_scope_arg_offset(hir: &FileHir, call: &CallSite) -> usize {
    usize::from(
        call.caller_scope
            && call
                .callee_reference
                .map(|reference| hir.reference(reference).name.as_str())
                == Some("call"),
    )
}

fn active_parameter_index(hir: &FileHir, call: &CallSite, offset: TextSize) -> Option<usize> {
    if call.arg_ranges.is_empty() {
        return Some(0);
    }

    let arg_offset = caller_scope_arg_offset(hir, call);
    let mut index = 0usize;
    for (current, range) in call.arg_ranges.iter().enumerate() {
        if range.contains(offset) {
            return current.checked_sub(arg_offset);
        }

        if offset >= range.start() {
            index = current;
        }
    }

    index.checked_sub(arg_offset)
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
        Some(TypeRef::Function(signature))
            if !matches!(signature.ret.as_ref(), TypeRef::Unknown) =>
        {
            format!(" -> {}", format_type_ref(signature.ret.as_ref()))
        }
        _ => String::new(),
    };

    format!("fn {name}({params}){ret}")
}
