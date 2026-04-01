use crate::builtin::semantics::is_builtin_fn_call;
use crate::builtin::signatures::builtin_universal_method_signature;
use crate::infer::ImportedMethodSignature;
use crate::infer::calls::CallableTarget;
use crate::infer::calls::selection::{
    caller_scope_dispatches_via_first_arg, dedup_callable_targets, effective_arg_types_for_call,
    has_informative_arg_types, inferred_expr_type, select_best_callable_targets,
};
use crate::infer::calls::signatures::{
    global_signature_for_pointer, global_signatures_for_call, join_callable_target_signatures,
    signature_from_type,
};
use crate::infer::objects::{
    host_method_signature_for_expr, largest_inner_expr, receiver_dispatch_is_precise,
    receiver_matches_method_type, string_literal_value,
};
use crate::{FileTypeInference, HostFunction, HostType, best_matching_signature_indexes};
use rhai_hir::{
    ExprId, ExprKind, ExternalSignatureIndex, FileHir, FunctionTypeRef, SymbolId, SymbolKind,
    TypeRef,
};

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

    if let Some(reference) = call.callee_reference {
        let local_targets = local_function_targets_for_reference(
            hir,
            inference,
            reference,
            call.range.start(),
            external,
            globals,
            host_types,
            imported_methods,
            arg_types.as_deref(),
        );
        if !local_targets.is_empty() {
            return local_targets;
        }
    }

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
                local_function_targets_for_reference(
                    hir,
                    inference,
                    reference,
                    use_offset,
                    external,
                    globals,
                    host_types,
                    imported_methods,
                    arg_types,
                )
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
pub(crate) fn local_function_targets_for_reference(
    hir: &FileHir,
    inference: &FileTypeInference,
    reference: rhai_hir::ReferenceId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Vec<CallableTarget> {
    if hir.reference(reference).kind != rhai_hir::ReferenceKind::Name {
        return Vec::new();
    }
    let Some(symbol) = hir.definition_of(reference) else {
        return Vec::new();
    };
    if hir.symbol(symbol).kind != SymbolKind::Function {
        return Vec::new();
    }

    let overloads = hir.visible_function_overloads_for_reference(reference);
    if overloads.is_empty() {
        return Vec::new();
    }

    let mut targets = Vec::new();
    let mut visited_symbols = Vec::new();
    for overload in overloads {
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
            &mut visited_symbols,
        ));
    }

    select_best_callable_targets(dedup_callable_targets(targets), arg_types)
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
        return select_best_callable_targets(dedup_callable_targets(targets), arg_types);
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
