use crate::builtin::semantics::is_builtin_fn_call;
use crate::builtin::signatures::builtin_universal_method_signature;
use crate::infer::ImportedMethodSignature;
use crate::infer::generics::specialize_signature_with_arg_types;
use crate::infer::helpers::{join_types, make_ambiguous_type};
use crate::infer::objects::{
    host_method_signature_for_expr, largest_inner_expr, receiver_dispatch_is_precise,
    receiver_matches_method_type, string_literal_value,
};
use crate::{FileTypeInference, HostFunction, HostType, best_matching_signature_indexes};
use rhai_hir::{
    ExprId, ExprKind, ExternalSignatureIndex, FileHir, FunctionTypeRef, SymbolId, SymbolKind,
    TypeRef,
};

pub(crate) fn global_signatures_for_call(
    globals: &[HostFunction],
    name: &str,
    arg_types: &[Option<TypeRef>],
    host_types: &[HostType],
) -> Vec<FunctionTypeRef> {
    let Some(function) = globals.iter().find(|function| function.name == name) else {
        return Vec::new();
    };
    let matching = function
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .map(|signature| {
            specialize_signature_with_arg_types(signature, Some(arg_types), host_types)
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Vec::new();
    }

    if has_informative_arg_types(arg_types) {
        let indexes = best_matching_signature_indexes(matching.iter(), arg_types);
        if !indexes.is_empty() {
            return indexes
                .into_iter()
                .filter_map(|index| matching.get(index).cloned())
                .collect();
        }
    }

    matching
        .into_iter()
        .filter(|signature| signature.params.len() == arg_types.len())
        .collect()
}

#[derive(Clone)]
pub(crate) struct CallableTarget {
    pub(crate) signature: FunctionTypeRef,
    pub(crate) local_symbol: Option<SymbolId>,
}

pub(crate) fn call_builtin_fn_signature(globals: &[HostFunction]) -> Option<&FunctionTypeRef> {
    globals
        .iter()
        .find(|function| function.name == "Fn")?
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .find(|signature| signature.params.len() == 1)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn callable_targets_for_call(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Vec<CallableTarget> {
    let mut targets = Vec::new();
    let arg_types = effective_arg_types_for_call(hir, inference, call, arg_types);

    if caller_scope_dispatches_via_first_arg(hir, call)
        && call.resolved_callee.is_none()
        && let Some(target_expr) = call.arg_exprs.first().copied()
    {
        targets.extend(callable_targets_for_expr(
            hir,
            inference,
            target_expr,
            call.range.start(),
            external,
            globals,
            host_types,
            imported_methods,
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
        return dedup_callable_targets(targets);
    }

    if let Some(callee) = call.resolved_callee {
        targets.extend(callable_targets_for_symbol_use(
            hir,
            inference,
            callee,
            call.range.start(),
            external,
            globals,
            host_types,
            imported_methods,
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
    }

    if targets.is_empty()
        && let Some(callee_expr) = call.callee_range.and_then(|range| hir.expr_at(range))
    {
        if let Some(signature) = host_method_signature_for_expr(
            hir,
            inference,
            callee_expr,
            host_types,
            arg_types.as_deref(),
        ) {
            return vec![CallableTarget {
                signature,
                local_symbol: None,
            }];
        }

        targets.extend(callable_targets_for_expr(
            hir,
            inference,
            callee_expr,
            call.range.start(),
            external,
            globals,
            host_types,
            imported_methods,
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
    }

    if targets.is_empty()
        && let Some(callee_name) = call
            .callee_reference
            .map(|reference_id| hir.reference(reference_id).name.as_str())
    {
        targets.extend(named_callable_targets_at_offset(
            hir,
            inference,
            callee_name,
            call.range.start(),
            external,
            globals,
            host_types,
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
    }

    dedup_callable_targets(targets)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn callable_targets_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    match hir.expr(expr).kind {
        ExprKind::Name => {
            let Some(reference) = hir.reference_at(hir.expr(expr).range) else {
                return Vec::new();
            };
            let Some(symbol) = hir.definition_of(reference) else {
                return Vec::new();
            };

            if hir.symbol(symbol).kind == SymbolKind::Function {
                let mut targets = Vec::new();
                for overload in hir.visible_function_overloads_for_reference(reference) {
                    targets.extend(callable_targets_for_symbol_use(
                        hir,
                        inference,
                        overload,
                        use_offset,
                        external,
                        globals,
                        host_types,
                        imported_methods,
                        arg_types,
                        visited_symbols,
                    ));
                }
                dedup_callable_targets(targets)
            } else {
                callable_targets_for_symbol_use(
                    hir,
                    inference,
                    symbol,
                    use_offset,
                    external,
                    globals,
                    host_types,
                    imported_methods,
                    arg_types,
                    visited_symbols,
                )
            }
        }
        ExprKind::Field => local_method_targets_for_expr(
            hir,
            inference,
            expr,
            use_offset,
            external,
            globals,
            host_types,
            imported_methods,
            arg_types,
            visited_symbols,
        ),
        ExprKind::Paren => largest_inner_expr(hir, expr)
            .map(|inner| {
                callable_targets_for_expr(
                    hir,
                    inference,
                    inner,
                    use_offset,
                    external,
                    globals,
                    host_types,
                    imported_methods,
                    arg_types,
                    visited_symbols,
                )
            })
            .unwrap_or_default(),
        ExprKind::Closure => inferred_expr_type(hir, inference, expr)
            .and_then(|ty| signature_from_type(&ty, arg_types, host_types))
            .map(|signature| {
                vec![CallableTarget {
                    signature,
                    local_symbol: None,
                }]
            })
            .unwrap_or_default(),
        ExprKind::Call => hir
            .calls
            .iter()
            .find(|call| call.range == hir.expr(expr).range)
            .filter(|call| is_builtin_fn_call(hir, call))
            .and_then(|call| {
                let name_expr = call.arg_exprs.first().copied()?;
                let name = string_literal_value(hir, name_expr)?;
                Some(named_callable_targets_at_offset(
                    hir,
                    inference,
                    name,
                    use_offset,
                    external,
                    globals,
                    host_types,
                    arg_types,
                    visited_symbols,
                ))
            })
            .unwrap_or_default(),
        _ => inferred_expr_type(hir, inference, expr)
            .and_then(|ty| signature_from_type(&ty, arg_types, host_types))
            .map(|signature| {
                vec![CallableTarget {
                    signature,
                    local_symbol: None,
                }]
            })
            .unwrap_or_default(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn callable_targets_for_symbol_use(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    if visited_symbols.contains(&symbol) {
        return Vec::new();
    }
    visited_symbols.push(symbol);

    let signature_target =
        callable_signature_for_symbol(hir, inference, symbol, external, arg_types, host_types)
            .map(|signature| {
                vec![CallableTarget {
                    signature,
                    local_symbol: (hir.symbol(symbol).kind == SymbolKind::Function)
                        .then_some(symbol),
                }]
            })
            .unwrap_or_default();
    let mut flow_targets = Vec::new();

    for flow in hir
        .value_flows_into(symbol)
        .filter(|flow| flow.range.start() < use_offset)
    {
        flow_targets.extend(callable_targets_for_expr(
            hir,
            inference,
            flow.expr,
            use_offset,
            external,
            globals,
            host_types,
            imported_methods,
            arg_types,
            visited_symbols,
        ));
    }

    visited_symbols.pop();
    let mut targets = if hir.symbol(symbol).kind != SymbolKind::Function && !flow_targets.is_empty()
    {
        std::mem::take(&mut flow_targets)
    } else {
        signature_target
    };
    if hir.symbol(symbol).kind == SymbolKind::Function || targets.is_empty() {
        targets.extend(flow_targets);
    }
    dedup_callable_targets(targets)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn local_method_targets_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    let access = match hir.member_access(expr) {
        Some(access) => access,
        None => return Vec::new(),
    };
    let receiver_ty = match inferred_expr_type(hir, inference, access.receiver) {
        Some(ty) => ty,
        None => return Vec::new(),
    };
    let method_name = hir.reference(access.field_reference).name.as_str();
    local_method_targets_for_name(
        hir,
        inference,
        method_name,
        &receiver_ty,
        use_offset,
        external,
        globals,
        host_types,
        imported_methods,
        arg_types,
        visited_symbols,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn local_method_targets_for_name(
    hir: &FileHir,
    inference: &FileTypeInference,
    name: &str,
    receiver_ty: &TypeRef,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    let mut blanket = Vec::new();
    let mut typed = Vec::new();

    for (index, symbol_data) in hir.symbols.iter().enumerate() {
        let symbol = SymbolId(index as u32);
        if symbol_data.kind != SymbolKind::Function || symbol_data.name != name {
            continue;
        }

        let targets = callable_targets_for_symbol_use(
            hir,
            inference,
            symbol,
            use_offset,
            external,
            globals,
            host_types,
            imported_methods,
            arg_types,
            visited_symbols,
        );
        if targets.is_empty() {
            continue;
        }

        match hir
            .function_info(symbol)
            .and_then(|info| info.this_type.as_ref())
        {
            Some(this_type) if receiver_matches_method_type(receiver_ty, this_type) => {
                typed.extend(targets);
            }
            Some(_) => {}
            None => blanket.extend(targets),
        }
    }

    if typed.is_empty() {
        return dedup_callable_targets(builtin_universal_method_targets(
            name,
            arg_types,
            imported_method_targets_for_name(
                name,
                receiver_ty,
                imported_methods,
                arg_types,
                blanket,
            ),
        ));
    }

    if receiver_dispatch_is_precise(receiver_ty) {
        typed = builtin_universal_method_targets(
            name,
            arg_types,
            imported_method_targets_for_name(name, receiver_ty, imported_methods, arg_types, typed),
        );
        return dedup_callable_targets(typed);
    }

    typed.extend(blanket);
    dedup_callable_targets(builtin_universal_method_targets(
        name,
        arg_types,
        imported_method_targets_for_name(name, receiver_ty, imported_methods, arg_types, typed),
    ))
}

pub(crate) fn imported_method_signature_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let access = hir.member_access(expr)?;
    let receiver_ty = inferred_expr_type(hir, inference, access.receiver)?;
    let method_name = hir.reference(access.field_reference).name.as_str();
    let targets = imported_method_targets_for_name(
        method_name,
        &receiver_ty,
        imported_methods,
        arg_types,
        Vec::new(),
    );
    join_callable_target_signatures(&targets, arg_types.map(|items| items.len()))
}

pub(crate) fn imported_method_targets_for_name(
    name: &str,
    receiver_ty: &TypeRef,
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    mut targets: Vec<CallableTarget>,
) -> Vec<CallableTarget> {
    let matching = imported_methods
        .iter()
        .filter(|method| {
            method.name == name && receiver_matches_method_type(receiver_ty, &method.receiver)
        })
        .filter(|method| {
            arg_types.is_none_or(|arg_types| method.signature.params.len() == arg_types.len())
        })
        .cloned()
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return targets;
    }

    if let Some(arg_types) = arg_types
        && has_informative_arg_types(arg_types)
    {
        let indexes = best_matching_signature_indexes(
            matching.iter().map(|method| &method.signature),
            arg_types,
        );
        if !indexes.is_empty() {
            targets.extend(indexes.into_iter().filter_map(|index| {
                matching.get(index).map(|method| CallableTarget {
                    signature: method.signature.clone(),
                    local_symbol: None,
                })
            }));
            return targets;
        }
    }

    targets.extend(matching.into_iter().map(|method| CallableTarget {
        signature: method.signature,
        local_symbol: None,
    }));
    targets
}

pub(crate) fn builtin_universal_method_targets(
    method_name: &str,
    arg_types: Option<&[Option<TypeRef>]>,
    mut targets: Vec<CallableTarget>,
) -> Vec<CallableTarget> {
    let Some(signature) = builtin_universal_method_signature(method_name) else {
        return targets;
    };

    if arg_types.is_some_and(|arg_types| signature.params.len() != arg_types.len()) {
        return targets;
    }

    targets.push(CallableTarget {
        signature,
        local_symbol: None,
    });
    targets
}

pub(crate) fn callable_signature_for_symbol(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    external: &ExternalSignatureIndex,
    arg_types: Option<&[Option<TypeRef>]>,
    host_types: &[HostType],
) -> Option<FunctionTypeRef> {
    inference
        .symbol_types
        .get(&symbol)
        .or_else(|| hir.declared_symbol_type(symbol))
        .or_else(|| external.get(hir.symbol(symbol).name.as_str()))
        .and_then(|ty| signature_from_type(ty, arg_types, host_types))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn named_callable_targets_at_offset(
    hir: &FileHir,
    inference: &FileTypeInference,
    name: &str,
    offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    let visible = hir
        .visible_symbols_at(offset)
        .into_iter()
        .filter(|symbol| hir.symbol(*symbol).name == name)
        .collect::<Vec<_>>();

    if !visible.is_empty() {
        let mut targets = Vec::new();
        for symbol in visible {
            targets.extend(callable_targets_for_symbol_use(
                hir,
                inference,
                symbol,
                offset,
                external,
                globals,
                host_types,
                &[],
                arg_types,
                visited_symbols,
            ));
        }
        return dedup_callable_targets(targets);
    }

    let mut targets = Vec::new();

    if let Some(arg_types) = arg_types {
        for signature in global_signatures_for_call(globals, name, arg_types, host_types) {
            targets.push(CallableTarget {
                signature,
                local_symbol: None,
            });
        }
    } else if let Some(signature) = global_signature_for_pointer(globals, name) {
        targets.push(CallableTarget {
            signature,
            local_symbol: None,
        });
    }

    if let Some(ty) = external.get(name)
        && let Some(signature) = signature_from_type(ty, arg_types, host_types)
    {
        targets.push(CallableTarget {
            signature,
            local_symbol: None,
        });
    }

    dedup_callable_targets(targets)
}

pub(crate) fn global_signature_for_pointer(
    globals: &[HostFunction],
    name: &str,
) -> Option<FunctionTypeRef> {
    let signatures = globals
        .iter()
        .find(|function| function.name == name)?
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref().cloned())
        .collect::<Vec<_>>();

    merge_function_candidate_signatures(signatures, None)
}

pub(crate) fn join_callable_target_signatures(
    targets: &[CallableTarget],
    arg_count: Option<usize>,
) -> Option<FunctionTypeRef> {
    merge_function_candidate_signatures(
        targets
            .iter()
            .map(|target| target.signature.clone())
            .collect(),
        arg_count,
    )
}

pub(crate) fn merge_function_candidate_signatures(
    signatures: Vec<FunctionTypeRef>,
    arg_count: Option<usize>,
) -> Option<FunctionTypeRef> {
    let signatures = signatures
        .into_iter()
        .filter(|signature| arg_count.is_none_or(|count| signature.params.len() == count))
        .collect::<Vec<_>>();
    let first = signatures.first()?.clone();
    let param_len = first.params.len();
    if signatures
        .iter()
        .any(|signature| signature.params.len() != param_len)
    {
        return None;
    }

    if signatures.len() == 1 {
        return Some(first);
    }

    let params = (0..param_len)
        .map(|index| {
            make_ambiguous_type(
                signatures
                    .iter()
                    .map(|signature| signature.params[index].clone())
                    .collect(),
            )
        })
        .collect();
    let ret = make_ambiguous_type(
        signatures
            .iter()
            .map(|signature| signature.ret.as_ref().clone())
            .collect(),
    );

    Some(FunctionTypeRef {
        params,
        ret: Box::new(ret),
    })
}

pub(crate) fn dedup_callable_targets(targets: Vec<CallableTarget>) -> Vec<CallableTarget> {
    let mut deduped = Vec::new();
    for target in targets {
        if deduped.iter().any(|existing: &CallableTarget| {
            existing.local_symbol == target.local_symbol && existing.signature == target.signature
        }) {
            continue;
        }
        deduped.push(target);
    }
    deduped
}

pub(crate) fn expected_call_signature(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
) -> Option<FunctionTypeRef> {
    if is_builtin_fn_call(hir, call) {
        return call_builtin_fn_signature(globals).cloned();
    }

    let arg_types = effective_call_argument_types(hir, inference, call);
    let targets = callable_targets_for_call(
        hir,
        inference,
        call,
        external,
        globals,
        host_types,
        imported_methods,
        Some(&arg_types),
    );
    join_callable_target_signatures(&targets, Some(arg_types.len()))
}

pub(crate) fn signature_from_type(
    ty: &TypeRef,
    arg_types: Option<&[Option<TypeRef>]>,
    host_types: &[HostType],
) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Function(signature) => {
            if arg_types.is_some_and(|arg_types| signature.params.len() != arg_types.len()) {
                return None;
            }
            Some(specialize_signature_with_arg_types(
                signature, arg_types, host_types,
            ))
        }
        TypeRef::Ambiguous(items) => merge_function_candidate_signatures(
            items
                .iter()
                .filter_map(|item| signature_from_type(item, arg_types, host_types))
                .collect(),
            None,
        ),
        _ => None,
    }
}

pub(crate) fn inferred_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    inference
        .expr_types
        .get(hir.expr_result_slot(expr))
        .cloned()
}

pub(crate) fn call_argument_types(
    hir: &FileHir,
    inference: &FileTypeInference,
    arg_exprs: &[ExprId],
) -> Vec<Option<TypeRef>> {
    arg_exprs
        .iter()
        .map(|expr| {
            inference
                .expr_types
                .get(hir.expr_result_slot(*expr))
                .cloned()
        })
        .collect()
}

pub(crate) fn caller_scope_dispatches_via_first_arg(
    hir: &FileHir,
    call: &rhai_hir::CallSite,
) -> bool {
    call.caller_scope
        && call
            .callee_reference
            .map(|reference| hir.reference(reference).name.as_str())
            == Some("call")
}

pub(crate) fn effective_call_argument_types(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
) -> Vec<Option<TypeRef>> {
    let arg_offset =
        usize::from(caller_scope_dispatches_via_first_arg(hir, call)).min(call.arg_exprs.len());
    call_argument_types(hir, inference, &call.arg_exprs[arg_offset..])
}

pub(crate) fn effective_arg_types_for_call(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<Vec<Option<TypeRef>>> {
    arg_types
        .map(|arg_types| {
            let arg_offset = usize::from(caller_scope_dispatches_via_first_arg(hir, call));
            if arg_types.len() == call.arg_exprs.len() {
                arg_types[arg_offset.min(arg_types.len())..].to_vec()
            } else {
                arg_types.to_vec()
            }
        })
        .or_else(|| Some(effective_call_argument_types(hir, inference, call)))
}

pub(crate) fn for_binding_types_from_iterable(
    ty: &TypeRef,
    binding_count: usize,
) -> Option<Vec<TypeRef>> {
    if binding_count == 0 {
        return Some(Vec::new());
    }

    match ty {
        TypeRef::Array(inner) => Some(loop_binding_types(
            inner.as_ref().clone(),
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::String => Some(loop_binding_types(
            TypeRef::Char,
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::Range | TypeRef::RangeInclusive => Some(loop_binding_types(
            TypeRef::Int,
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::Union(items) => {
            let mut merged = None;
            for item in items {
                let Some(next) = for_binding_types_from_iterable(item, binding_count) else {
                    continue;
                };
                merged = Some(match merged {
                    Some(current) => join_binding_type_sets(current, next),
                    None => next,
                });
            }
            merged
        }
        TypeRef::Ambiguous(items) => {
            let mut merged = None;
            for item in items {
                let Some(next) = for_binding_types_from_iterable(item, binding_count) else {
                    continue;
                };
                merged = Some(match merged {
                    Some(current) => join_binding_type_sets(current, next),
                    None => next,
                });
            }
            merged
        }
        _ => None,
    }
}

pub(crate) fn loop_binding_types(
    item_ty: TypeRef,
    binding_count: usize,
    counter_ty: TypeRef,
) -> Vec<TypeRef> {
    let mut binding_types = vec![TypeRef::Unknown; binding_count];
    if let Some(first) = binding_types.first_mut() {
        *first = item_ty;
    }
    if binding_count > 1 {
        binding_types[1] = counter_ty;
    }
    binding_types
}

pub(crate) fn join_binding_type_sets(left: Vec<TypeRef>, right: Vec<TypeRef>) -> Vec<TypeRef> {
    let len = left.len().max(right.len());
    (0..len)
        .map(|index| match (left.get(index), right.get(index)) {
            (Some(left), Some(right)) => join_types(left, right),
            (Some(left), None) => left.clone(),
            (None, Some(right)) => right.clone(),
            (None, None) => TypeRef::Unknown,
        })
        .collect()
}

pub(crate) fn has_informative_arg_types(arg_types: &[Option<TypeRef>]) -> bool {
    arg_types.iter().flatten().any(|ty| {
        !matches!(
            ty,
            TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never
        )
    })
}
