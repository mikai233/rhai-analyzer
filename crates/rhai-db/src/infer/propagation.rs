use crate::infer::ImportedMethodSignature;
use crate::infer::calls::{
    callable_targets_for_call, effective_call_argument_types, expected_call_signature,
    for_binding_types_from_iterable, inferred_expr_type,
};
use crate::infer::helpers::{
    can_refine_with_expected, informative_expected_type, join_types, nested_mutation_container_type,
};
use crate::infer::loops::function_like_body_result_type;
use crate::infer::objects::{largest_inner_expr, symbol_for_expr};
use crate::{FileTypeInference, HostFunction, HostType};
use rhai_hir::{
    ExpectedTypeSource, ExprId, ExprKind, ExternalSignatureIndex, FileHir, FunctionTypeRef,
    SymbolId, SymbolKind, SymbolMutationKind, TypeRef,
};

pub(crate) fn propagate_call_argument_types(
    hir: &FileHir,
    imported_methods: &[ImportedMethodSignature],
    inference: &mut FileTypeInference,
) -> bool {
    let mut changed = false;

    for call in &hir.calls {
        for (index, parameter) in call.parameter_bindings.iter().copied().enumerate() {
            let Some(parameter) = parameter else {
                continue;
            };
            let Some(arg_expr) = call.arg_exprs.get(index).copied() else {
                continue;
            };
            let Some(arg_ty) = inference
                .expr_types
                .get(hir.expr_result_slot(arg_expr))
                .cloned()
            else {
                continue;
            };
            changed |= merge_symbol_type(inference, parameter, arg_ty);
        }

        let arg_types = effective_call_argument_types(hir, inference, call);
        for target in callable_targets_for_call(
            hir,
            inference,
            call,
            &ExternalSignatureIndex::default(),
            &[],
            &[],
            imported_methods,
            Some(&arg_types),
        ) {
            let Some(function_symbol) = target.local_symbol else {
                continue;
            };

            for (parameter, arg_ty) in hir
                .function_parameters(function_symbol)
                .into_iter()
                .zip(arg_types.iter().flatten().cloned())
            {
                changed |= merge_symbol_type(inference, parameter, arg_ty);
            }
        }
    }

    changed
}

pub(crate) fn propagate_expected_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    inference: &mut FileTypeInference,
) -> bool {
    let mut changed = false;

    for site in &hir.expected_type_sites {
        let expected = match site.source {
            ExpectedTypeSource::Symbol(symbol) => inference.symbol_types.get(&symbol).cloned(),
            ExpectedTypeSource::FunctionReturn(function) => inference
                .symbol_types
                .get(&function)
                .cloned()
                .and_then(|ty| match ty {
                    TypeRef::Function(signature) => Some((*signature.ret).clone()),
                    _ => None,
                }),
            ExpectedTypeSource::CallArgument {
                call,
                parameter_index,
            } => {
                let call = hir.call(call);
                expected_call_signature(
                    hir,
                    inference,
                    call,
                    external,
                    globals,
                    host_types,
                    imported_methods,
                )
                .and_then(|signature| signature.params.get(parameter_index).cloned())
            }
        };

        if let Some(expected) = expected.and_then(|expected| informative_expected_type(&expected)) {
            changed |= propagate_expected_type_to_expr(hir, inference, site.expr, &expected);
        }
    }

    changed
}

pub(crate) fn propagate_value_flows(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
    let mut changed = false;

    for flow in &hir.value_flows {
        let Some(expr_ty) = inference
            .expr_types
            .get(hir.expr_result_slot(flow.expr))
            .cloned()
        else {
            continue;
        };
        changed |= if should_overwrite_symbol_from_single_flow(hir, flow.symbol, flow.expr) {
            set_symbol_type(inference, flow.symbol, expr_ty)
        } else {
            merge_symbol_type(inference, flow.symbol, expr_ty)
        };
    }

    changed
}

fn should_overwrite_symbol_from_single_flow(hir: &FileHir, symbol: SymbolId, expr: ExprId) -> bool {
    if hir.declared_symbol_type(symbol).is_some() {
        return false;
    }

    if !matches!(
        hir.expr(expr).kind,
        ExprKind::Block
            | ExprKind::If
            | ExprKind::Switch
            | ExprKind::Call
            | ExprKind::Name
            | ExprKind::Field
            | ExprKind::Index
            | ExprKind::Paren
    ) {
        return false;
    }

    let kind = hir.symbol(symbol).kind;
    if !matches!(kind, SymbolKind::Variable | SymbolKind::Constant) {
        return false;
    }

    if hir.symbol_mutations_into(symbol).next().is_some() {
        return false;
    }

    let mut flows = hir.value_flows_into(symbol);
    let Some(first) = flows.next() else {
        return false;
    };

    flows.next().is_none() && first.expr == expr
}

pub(crate) fn propagate_for_binding_types(
    hir: &FileHir,
    inference: &mut FileTypeInference,
) -> bool {
    let mut changed = false;

    for for_expr in &hir.for_exprs {
        let Some(iterable) = for_expr.iterable else {
            continue;
        };
        let Some(iterable_ty) = inferred_expr_type(hir, inference, iterable) else {
            continue;
        };
        let Some(binding_types) =
            for_binding_types_from_iterable(&iterable_ty, for_expr.bindings.len())
        else {
            continue;
        };

        for (binding, binding_ty) in for_expr.bindings.iter().copied().zip(binding_types) {
            changed |= merge_symbol_type(inference, binding, binding_ty);
        }
    }

    changed
}

pub(crate) fn propagate_symbol_mutations(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
    let mut changed = false;

    for mutation in &hir.symbol_mutations {
        let Some(value_ty) = inference
            .expr_types
            .get(hir.expr_result_slot(mutation.value))
            .cloned()
        else {
            continue;
        };

        let container_ty = match &mutation.kind {
            SymbolMutationKind::Path { segments } => {
                nested_mutation_container_type(hir, inference, segments, value_ty)
            }
        };

        changed |= merge_symbol_type(inference, mutation.symbol, container_ty);
    }

    changed
}

pub(crate) fn infer_function_signatures(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
    let mut changed = false;

    for (index, symbol) in hir.symbols.iter().enumerate() {
        if symbol.kind != SymbolKind::Function {
            continue;
        }

        let symbol_id = SymbolId(index as u32);
        let params = hir
            .function_parameters(symbol_id)
            .into_iter()
            .map(|param| {
                inference
                    .symbol_types
                    .get(&param)
                    .cloned()
                    .or_else(|| hir.declared_symbol_type(param).cloned())
                    .unwrap_or(TypeRef::Unknown)
            })
            .collect::<Vec<_>>();

        let ret = hir
            .body_of(symbol_id)
            .and_then(|body| function_like_body_result_type(hir, inference, body))
            .unwrap_or(TypeRef::Unknown);

        changed |= merge_symbol_type(
            inference,
            symbol_id,
            TypeRef::Function(FunctionTypeRef {
                params,
                ret: Box::new(ret),
            }),
        );
    }

    changed
}

pub(crate) fn merge_expr_type(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    expr: ExprId,
    next: TypeRef,
) -> bool {
    let slot = hir.expr_result_slot(expr);
    let merged = match inference.expr_types.get(slot) {
        Some(current) => join_types(current, &next),
        None => next,
    };

    if inference.expr_types.get(slot) == Some(&merged) {
        return false;
    }

    inference.expr_types.set(slot, merged);
    true
}

pub(crate) fn set_expr_type(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    expr: ExprId,
    next: TypeRef,
) -> bool {
    let slot = hir.expr_result_slot(expr);
    if inference.expr_types.get(slot) == Some(&next) {
        return false;
    }

    inference.expr_types.set(slot, next);
    true
}

pub(crate) fn merge_symbol_type(
    inference: &mut FileTypeInference,
    symbol: SymbolId,
    next: TypeRef,
) -> bool {
    let merged = match inference.symbol_types.get(&symbol) {
        Some(current) => join_types(current, &next),
        None => next,
    };

    if inference.symbol_types.get(&symbol) == Some(&merged) {
        return false;
    }

    inference.symbol_types.insert(symbol, merged);
    true
}

pub(crate) fn set_symbol_type(
    inference: &mut FileTypeInference,
    symbol: SymbolId,
    next: TypeRef,
) -> bool {
    if inference.symbol_types.get(&symbol) == Some(&next) {
        return false;
    }

    inference.symbol_types.insert(symbol, next);
    true
}

pub(crate) fn merge_expr_type_if_refinable(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    expr: ExprId,
    expected: TypeRef,
) -> bool {
    let current = inference.expr_types.get(hir.expr_result_slot(expr));
    if current.is_some_and(|current| !can_refine_with_expected(current, &expected)) {
        return false;
    }

    merge_expr_type(hir, inference, expr, expected)
}

pub(crate) fn merge_symbol_type_if_refinable(
    inference: &mut FileTypeInference,
    symbol: SymbolId,
    expected: TypeRef,
) -> bool {
    let current = inference.symbol_types.get(&symbol);
    if current.is_some_and(|current| !can_refine_with_expected(current, &expected)) {
        return false;
    }

    merge_symbol_type(inference, symbol, expected)
}

pub(crate) fn propagate_expected_type_to_expr(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    expr: ExprId,
    expected: &TypeRef,
) -> bool {
    let mut changed = false;

    match hir.expr(expr).kind {
        ExprKind::Name => {
            if let Some(symbol) = symbol_for_expr(hir, expr) {
                changed |= merge_symbol_type_if_refinable(inference, symbol, expected.clone());
            }
        }
        ExprKind::Array => {
            if let Some(array) = hir.array_expr(expr)
                && let TypeRef::Array(inner) = expected
            {
                for item in &array.items {
                    changed |= propagate_expected_type_to_expr(hir, inference, *item, inner);
                }
            }
        }
        ExprKind::Object => {
            for field in hir.object_fields.iter().filter(|field| field.owner == expr) {
                let Some(value) = field.value else {
                    continue;
                };
                let Some(field_expected) =
                    expected_object_field_type(expected, field.name.as_str())
                else {
                    continue;
                };
                changed |= propagate_expected_type_to_expr(hir, inference, value, &field_expected);
            }
        }
        ExprKind::Block => {
            if let Some(block) = hir.block_expr(expr)
                && hir.body_may_fall_through(block.body)
                && let Some(tail) = hir.body_tail_value(block.body)
            {
                changed |= propagate_expected_type_to_expr(hir, inference, tail, expected);
            }
        }
        ExprKind::If => {
            if let Some(if_expr) = hir.if_expr(expr) {
                if let Some(then_branch) = if_expr.then_branch {
                    changed |=
                        propagate_expected_type_to_expr(hir, inference, then_branch, expected);
                }
                if let Some(else_branch) = if_expr.else_branch {
                    changed |=
                        propagate_expected_type_to_expr(hir, inference, else_branch, expected);
                }
            }
        }
        ExprKind::Switch => {
            if let Some(switch) = hir.switch_expr(expr) {
                for arm in switch.arms.iter().flatten().copied() {
                    changed |= propagate_expected_type_to_expr(hir, inference, arm, expected);
                }
            }
        }
        ExprKind::Assign => {
            if let Some(assign) = hir.assign_expr(expr)
                && let Some(rhs) = assign.rhs
            {
                changed |= propagate_expected_type_to_expr(hir, inference, rhs, expected);
            }
        }
        ExprKind::Paren => {
            if let Some(inner) = largest_inner_expr(hir, expr) {
                changed |= propagate_expected_type_to_expr(hir, inference, inner, expected);
            }
        }
        ExprKind::Closure => {
            if let TypeRef::Function(signature) = expected {
                changed |=
                    propagate_expected_function_type_to_closure(hir, inference, expr, signature);
            }
        }
        _ => {}
    }

    changed | merge_expr_type_if_refinable(hir, inference, expr, expected.clone())
}

pub(crate) fn expected_object_field_type(expected: &TypeRef, field_name: &str) -> Option<TypeRef> {
    match expected {
        TypeRef::Object(fields) => fields.get(field_name).cloned(),
        TypeRef::Map(key, value)
            if matches!(
                key.as_ref(),
                TypeRef::String | TypeRef::Unknown | TypeRef::Any
            ) =>
        {
            Some((**value).clone())
        }
        TypeRef::Nullable(inner) => expected_object_field_type(inner, field_name),
        TypeRef::Union(items) => items
            .iter()
            .filter_map(|item| expected_object_field_type(item, field_name))
            .reduce(|left, right| join_types(&left, &right)),
        _ => None,
    }
}

pub(crate) fn propagate_expected_type_to_body_results(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    body: rhai_hir::BodyId,
    expected: &TypeRef,
) -> bool {
    let mut changed = false;

    for expr in hir.body_return_values(body) {
        changed |= propagate_expected_type_to_expr(hir, inference, expr, expected);
    }

    if hir.body_may_fall_through(body)
        && let Some(tail) = hir.body_tail_value(body)
    {
        changed |= propagate_expected_type_to_expr(hir, inference, tail, expected);
    }

    changed
}

pub(crate) fn propagate_expected_function_type_to_closure(
    hir: &FileHir,
    inference: &mut FileTypeInference,
    expr: ExprId,
    expected: &FunctionTypeRef,
) -> bool {
    let Some(closure) = hir.closure_expr(expr) else {
        return false;
    };

    let mut changed = false;
    let params = hir
        .scope(hir.body(closure.body).scope)
        .symbols
        .iter()
        .copied()
        .filter(|symbol_id| hir.symbol(*symbol_id).kind == SymbolKind::Parameter)
        .collect::<Vec<_>>();

    for (param, expected_param) in params.into_iter().zip(expected.params.iter()) {
        changed |= merge_symbol_type_if_refinable(inference, param, expected_param.clone());
    }

    changed |= propagate_expected_type_to_body_results(
        hir,
        inference,
        closure.body,
        expected.ret.as_ref(),
    );
    changed |=
        merge_expr_type_if_refinable(hir, inference, expr, TypeRef::Function(expected.clone()));
    changed
}
