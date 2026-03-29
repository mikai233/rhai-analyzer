use std::collections::HashMap;

use rhai_hir::{
    BinaryOperator, BodyId, ControlFlowKind, ExprId, ExprKind, ExternalSignatureIndex, FileHir,
    FunctionTypeRef, LiteralKind, ScopeKind, SymbolId, SymbolKind, SymbolMutationKind, TypeRef,
    UnaryOperator,
};

use crate::{FileTypeInference, HostFunction, HostType, best_matching_signature_index};

pub(crate) fn infer_file_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    seed_symbol_types: &HashMap<SymbolId, TypeRef>,
) -> FileTypeInference {
    let mut inference = FileTypeInference {
        expr_types: hir.new_type_slot_assignments(),
        symbol_types: HashMap::new(),
    };

    for (&symbol, ty) in seed_symbol_types {
        merge_symbol_type(&mut inference, symbol, ty.clone());
    }

    for (index, symbol) in hir.symbols.iter().enumerate() {
        if let Some(annotation) = symbol.annotation.clone() {
            merge_symbol_type(&mut inference, SymbolId(index as u32), annotation);
        }
    }

    let max_iterations =
        hir.exprs.len() + hir.symbols.len() + hir.calls.len() + hir.bodies.len() + 1;
    for _ in 0..max_iterations.max(1) {
        let mut changed = false;

        changed |= infer_expr_types(hir, external, globals, host_types, &mut inference);
        changed |= propagate_call_argument_types(hir, &mut inference);
        changed |= propagate_value_flows(hir, &mut inference);
        changed |= propagate_symbol_mutations(hir, &mut inference);
        changed |= infer_function_signatures(hir, &mut inference);

        if !changed {
            break;
        }
    }

    inference
}

fn infer_expr_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    inference: &mut FileTypeInference,
) -> bool {
    let mut changed = false;

    for (index, expr) in hir.exprs.iter().enumerate() {
        let expr_id = ExprId(index as u32);
        let inferred = match expr.kind {
            ExprKind::Literal => infer_literal_expr_type(hir, expr_id),
            ExprKind::Array => infer_array_expr_type(hir, inference, expr_id),
            ExprKind::Block => infer_block_expr_type(hir, inference, expr_id),
            ExprKind::If => infer_if_expr_type(hir, inference, expr_id),
            ExprKind::Switch => infer_switch_expr_type(hir, inference, expr_id),
            ExprKind::While => infer_while_expr_type(hir, inference, expr_id),
            ExprKind::Loop => infer_loop_expr_type(hir, inference, expr_id),
            ExprKind::For => infer_for_expr_type(hir, inference, expr_id),
            ExprKind::Do => infer_do_expr_type(hir, inference, expr_id),
            ExprKind::Path => infer_path_expr_type(hir, expr_id, external),
            ExprKind::Name => infer_name_expr_type(hir, expr_id, inference, external),
            ExprKind::InterpolatedString => Some(TypeRef::String),
            ExprKind::Unary => infer_unary_expr_type(hir, inference, expr_id),
            ExprKind::Binary => infer_binary_expr_type(hir, inference, expr_id),
            ExprKind::Assign => infer_assign_expr_type(hir, inference, expr_id),
            ExprKind::Paren => infer_paren_expr_type(hir, inference, expr_id),
            ExprKind::Call => {
                infer_call_expr_type(hir, expr_id, inference, external, globals, host_types)
            }
            ExprKind::Index => infer_index_expr_type(hir, inference, expr_id),
            ExprKind::Field => infer_field_expr_type(hir, inference, expr_id, host_types),
            ExprKind::Object => infer_object_expr_type(hir, inference, expr_id),
            ExprKind::Closure => infer_closure_expr_type(hir, inference, expr_id),
            ExprKind::Error => Some(TypeRef::Unknown),
        };

        if let Some(ty) = inferred {
            changed |= merge_expr_type(hir, inference, expr_id, ty);
        }
    }

    changed
}

fn infer_literal_expr_type(hir: &FileHir, expr: ExprId) -> Option<TypeRef> {
    let literal = hir.literal(expr)?;
    Some(match literal.kind {
        LiteralKind::Int => TypeRef::Int,
        LiteralKind::Float => TypeRef::Float,
        LiteralKind::String => TypeRef::String,
        LiteralKind::Char => TypeRef::Char,
        LiteralKind::Bool => TypeRef::Bool,
    })
}

fn infer_array_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let array = hir.array_expr(expr)?;
    let inner = array
        .items
        .iter()
        .filter_map(|item| inferred_expr_type(hir, inference, *item))
        .reduce(|left, right| join_types(&left, &right))
        .unwrap_or(TypeRef::Unknown);
    Some(TypeRef::Array(Box::new(inner)))
}

fn infer_object_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let value = hir
        .object_fields
        .iter()
        .filter(|field| field.owner == expr)
        .filter_map(|field| {
            field
                .value
                .and_then(|value| inferred_expr_type(hir, inference, value))
        })
        .reduce(|left, right| join_types(&left, &right))
        .unwrap_or(TypeRef::Unknown);

    Some(TypeRef::Map(Box::new(TypeRef::String), Box::new(value)))
}

fn infer_block_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let block = hir.block_expr(expr)?;
    block_expr_result_type(hir, inference, block.body)
}

fn infer_if_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let if_expr = hir.if_expr(expr)?;
    join_expr_types(hir, inference, if_expr.then_branch, if_expr.else_branch)
}

fn infer_switch_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let switch = hir.switch_expr(expr)?;
    switch
        .arms
        .iter()
        .flatten()
        .filter_map(|arm| inferred_expr_type(hir, inference, *arm))
        .reduce(|left, right| join_types(&left, &right))
}

fn infer_while_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

fn infer_loop_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, false)
}

fn infer_for_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

fn infer_do_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

fn infer_path_expr_type(
    hir: &FileHir,
    expr: ExprId,
    external: &ExternalSignatureIndex,
) -> Option<TypeRef> {
    let qualified = qualified_path_name(hir, expr)?;
    external.get(qualified.as_str()).cloned()
}

fn infer_name_expr_type(
    hir: &FileHir,
    expr: ExprId,
    inference: &FileTypeInference,
    external: &ExternalSignatureIndex,
) -> Option<TypeRef> {
    let reference = hir.reference_at(hir.expr(expr).range)?;
    if let Some(target) = hir.definition_of(reference) {
        return inference
            .symbol_types
            .get(&target)
            .cloned()
            .or_else(|| hir.declared_symbol_type(target).cloned());
    }

    external
        .get(hir.reference(reference).name.as_str())
        .cloned()
}

fn infer_unary_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let unary = hir.unary_expr(expr)?;
    let operand = unary.operand.and_then(|operand| {
        inference
            .expr_types
            .get(hir.expr_result_slot(operand))
            .cloned()
    })?;

    match unary.operator {
        UnaryOperator::Not => Some(TypeRef::Bool),
        UnaryOperator::Plus | UnaryOperator::Minus => match operand {
            TypeRef::Int | TypeRef::Float | TypeRef::Decimal => Some(operand),
            _ => None,
        },
    }
}

fn infer_binary_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let binary = hir.binary_expr(expr)?;
    let lhs = binary
        .lhs
        .and_then(|lhs| inference.expr_types.get(hir.expr_result_slot(lhs)).cloned());
    let rhs = binary
        .rhs
        .and_then(|rhs| inference.expr_types.get(hir.expr_result_slot(rhs)).cloned());

    match binary.operator {
        BinaryOperator::OrOr
        | BinaryOperator::AndAnd
        | BinaryOperator::EqEq
        | BinaryOperator::NotEq
        | BinaryOperator::In
        | BinaryOperator::Gt
        | BinaryOperator::GtEq
        | BinaryOperator::Lt
        | BinaryOperator::LtEq => Some(TypeRef::Bool),
        BinaryOperator::Range => Some(TypeRef::Range),
        BinaryOperator::RangeInclusive => Some(TypeRef::RangeInclusive),
        BinaryOperator::NullCoalesce => join_option_types(lhs.as_ref(), rhs.as_ref()),
        BinaryOperator::Add => infer_additive_result(lhs.as_ref(), rhs.as_ref()),
        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Power
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Or
        | BinaryOperator::Xor
        | BinaryOperator::And => infer_numeric_result(lhs.as_ref(), rhs.as_ref()),
    }
}

fn infer_assign_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    trailing_inner_expr(hir, expr).and_then(|rhs| inferred_expr_type(hir, inference, rhs))
}

fn infer_paren_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    largest_inner_expr(hir, expr).and_then(|inner| inferred_expr_type(hir, inference, inner))
}

fn infer_call_expr_type(
    hir: &FileHir,
    expr: ExprId,
    inference: &FileTypeInference,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
) -> Option<TypeRef> {
    let call = hir
        .calls
        .iter()
        .find(|call| call.range == hir.expr(expr).range)?;
    let arg_types = call_argument_types(hir, inference, &call.arg_exprs);

    if let Some(callee) = call.resolved_callee
        && let Some(callee_ty) = inference
            .symbol_types
            .get(&callee)
            .cloned()
            .or_else(|| hir.declared_symbol_type(callee).cloned())
        && let Some(ret) = function_return_type(&callee_ty)
    {
        return Some(ret);
    }

    if let Some(callee_expr) = call.callee_range.and_then(|range| hir.expr_at(range)) {
        if let Some(signature) = host_method_signature_for_expr(
            hir,
            inference,
            callee_expr,
            host_types,
            Some(&arg_types),
        ) {
            return Some((*signature.ret).clone());
        }

        if let Some(callee_ty) = inferred_expr_type(hir, inference, callee_expr)
            && let Some(ret) = function_return_type(&callee_ty)
        {
            return Some(ret);
        }
    }

    let callee_name = call
        .callee_reference
        .map(|reference_id| hir.reference(reference_id).name.as_str())?;

    if let Some(signature) = global_signature_for_call(globals, callee_name, &arg_types) {
        return Some((*signature.ret).clone());
    }

    function_return_type(external.get(callee_name)?)
}

fn infer_index_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let index = hir.index_expr(expr)?;
    match inferred_expr_type(hir, inference, index.receiver?)? {
        TypeRef::Array(inner) => Some(*inner),
        TypeRef::Map(_, value) => Some(*value),
        _ => None,
    }
}

fn infer_field_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    host_types: &[HostType],
) -> Option<TypeRef> {
    let access = hir.member_access(expr)?;
    let field_name = hir.reference(access.field_reference).name.as_str();
    infer_member_type_from_expr(hir, inference, access.receiver, field_name).or_else(|| {
        host_method_signature_for_expr(hir, inference, expr, host_types, None)
            .map(TypeRef::Function)
    })
}

fn infer_closure_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let closure = hir.closure_expr(expr)?;
    let params = hir
        .scope(hir.body(closure.body).scope)
        .symbols
        .iter()
        .copied()
        .filter(|symbol_id| hir.symbol(*symbol_id).kind == SymbolKind::Parameter)
        .map(|param| {
            inference
                .symbol_types
                .get(&param)
                .cloned()
                .or_else(|| hir.declared_symbol_type(param).cloned())
                .unwrap_or(TypeRef::Unknown)
        })
        .collect::<Vec<_>>();
    let ret =
        function_like_body_result_type(hir, inference, closure.body).unwrap_or(TypeRef::Unknown);

    Some(TypeRef::Function(FunctionTypeRef {
        params,
        ret: Box::new(ret),
    }))
}

fn propagate_call_argument_types(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
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
    }

    changed
}

fn propagate_value_flows(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
    let mut changed = false;

    for flow in &hir.value_flows {
        let Some(expr_ty) = inference
            .expr_types
            .get(hir.expr_result_slot(flow.expr))
            .cloned()
        else {
            continue;
        };
        changed |= merge_symbol_type(inference, flow.symbol, expr_ty);
    }

    changed
}

fn propagate_symbol_mutations(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
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
            SymbolMutationKind::Field { .. } => {
                TypeRef::Map(Box::new(TypeRef::String), Box::new(value_ty))
            }
            SymbolMutationKind::Index { index } => match inferred_expr_type(hir, inference, *index)
            {
                Some(TypeRef::Int) => TypeRef::Array(Box::new(value_ty)),
                Some(index_ty) => TypeRef::Map(Box::new(index_ty), Box::new(value_ty)),
                None => TypeRef::Map(Box::new(TypeRef::Unknown), Box::new(value_ty)),
            },
        };

        changed |= merge_symbol_type(inference, mutation.symbol, container_ty);
    }

    changed
}

fn infer_function_signatures(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
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

fn merge_expr_type(
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

fn merge_symbol_type(inference: &mut FileTypeInference, symbol: SymbolId, next: TypeRef) -> bool {
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

fn global_signature_for_call<'a>(
    globals: &'a [HostFunction],
    name: &str,
    arg_types: &[Option<TypeRef>],
) -> Option<&'a FunctionTypeRef> {
    let function = globals.iter().find(|function| function.name == name)?;
    let matching = function
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }

    if has_informative_arg_types(arg_types)
        && let Some(index) = best_matching_signature_index(matching.iter().copied(), arg_types)
    {
        return matching.get(index).copied();
    }

    matching
        .into_iter()
        .find(|signature| signature.params.len() == arg_types.len())
}

fn function_return_type(ty: &TypeRef) -> Option<TypeRef> {
    match ty {
        TypeRef::Function(signature) => Some((*signature.ret).clone()),
        _ => None,
    }
}

fn inferred_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    inference
        .expr_types
        .get(hir.expr_result_slot(expr))
        .cloned()
}

fn call_argument_types(
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

fn has_informative_arg_types(arg_types: &[Option<TypeRef>]) -> bool {
    arg_types.iter().flatten().any(|ty| {
        !matches!(
            ty,
            TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never
        )
    })
}

fn function_like_body_result_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    body: rhai_hir::BodyId,
) -> Option<TypeRef> {
    let explicit = hir
        .body_return_values(body)
        .filter_map(|expr| inferred_expr_type(hir, inference, expr))
        .reduce(|left, right| join_types(&left, &right));

    let tail = if hir.body_may_fall_through(body) {
        hir.body_tail_value(body)
            .and_then(|expr| inferred_expr_type(hir, inference, expr))
    } else {
        None
    };

    join_option_types(explicit.as_ref(), tail.as_ref())
}

fn block_expr_result_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    body: rhai_hir::BodyId,
) -> Option<TypeRef> {
    hir.body_may_fall_through(body)
        .then(|| hir.body_tail_value(body))
        .flatten()
        .and_then(|expr| inferred_expr_type(hir, inference, expr))
}

fn infer_loop_like_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    may_fall_through: bool,
) -> Option<TypeRef> {
    let loop_body = direct_loop_body(hir, expr)?;
    let loop_scope = hir.body(loop_body).scope;
    let mut result = None::<TypeRef>;

    for event in hir.body_control_flow(loop_body) {
        if event.kind != ControlFlowKind::Break || event.target_loop != Some(loop_scope) {
            continue;
        }

        let next = match event.value_range.and_then(|range| hir.expr_at(range)) {
            Some(break_expr) => {
                inferred_expr_type(hir, inference, break_expr).unwrap_or(TypeRef::Unknown)
            }
            None => TypeRef::Unit,
        };
        result = Some(match result {
            Some(current) => join_types(&current, &next),
            None => next,
        });
    }

    if may_fall_through {
        let unit = TypeRef::Unit;
        return Some(match result {
            Some(current) => join_types(&current, &unit),
            None => unit,
        });
    }

    result.or(Some(TypeRef::Never))
}

fn join_expr_types(
    hir: &FileHir,
    inference: &FileTypeInference,
    left: Option<ExprId>,
    right: Option<ExprId>,
) -> Option<TypeRef> {
    let left = left.and_then(|expr| inferred_expr_type(hir, inference, expr));
    let right = right.and_then(|expr| inferred_expr_type(hir, inference, expr));
    join_option_types(left.as_ref(), right.as_ref())
}

fn infer_member_type_from_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    field_name: &str,
) -> Option<TypeRef> {
    let resolved = field_value_exprs_from_expr(hir, expr, field_name)
        .into_iter()
        .filter_map(|value_expr| inferred_expr_type(hir, inference, value_expr))
        .reduce(|left, right| join_types(&left, &right));

    let documented = symbol_for_expr(hir, expr).and_then(|symbol| {
        hir.documented_fields(symbol)
            .into_iter()
            .find(|field| field.name == field_name)
            .map(|field| field.annotation)
    });

    let fallback_map_value = inferred_expr_type(hir, inference, expr).and_then(|ty| match ty {
        TypeRef::Map(_, value) => Some(*value),
        _ => None,
    });

    let symbolic = join_option_types(documented.as_ref(), fallback_map_value.as_ref());
    if resolved.is_some() {
        return resolved;
    }
    if documented.is_some() {
        return documented;
    }
    symbolic
}

fn field_value_exprs_from_expr(hir: &FileHir, expr: ExprId, field_name: &str) -> Vec<ExprId> {
    match hir.expr(expr).kind {
        ExprKind::Object => field_value_exprs_from_object_expr(hir, expr, field_name),
        ExprKind::Name => symbol_for_expr(hir, expr)
            .into_iter()
            .flat_map(|symbol| field_value_exprs_from_symbol(hir, symbol, field_name))
            .collect(),
        ExprKind::Field => {
            let Some(access) = hir.member_access(expr) else {
                return Vec::new();
            };
            let intermediate = hir.reference(access.field_reference).name.as_str();
            field_value_exprs_from_expr(hir, access.receiver, intermediate)
                .into_iter()
                .flat_map(|value_expr| field_value_exprs_from_expr(hir, value_expr, field_name))
                .collect()
        }
        ExprKind::Block => hir
            .block_expr(expr)
            .and_then(|block| hir.body_tail_value(block.body))
            .into_iter()
            .flat_map(|tail| field_value_exprs_from_expr(hir, tail, field_name))
            .collect(),
        ExprKind::If => hir
            .if_expr(expr)
            .into_iter()
            .flat_map(|if_expr| [if_expr.then_branch, if_expr.else_branch])
            .flatten()
            .flat_map(|branch| field_value_exprs_from_expr(hir, branch, field_name))
            .collect(),
        ExprKind::Switch => hir
            .switch_expr(expr)
            .into_iter()
            .flat_map(|switch| switch.arms.iter().flatten().copied())
            .flat_map(|arm| field_value_exprs_from_expr(hir, arm, field_name))
            .collect(),
        _ => Vec::new(),
    }
}

fn field_value_exprs_from_symbol(hir: &FileHir, symbol: SymbolId, field_name: &str) -> Vec<ExprId> {
    let mut exprs = hir
        .value_flows_into(symbol)
        .flat_map(|flow| field_value_exprs_from_expr(hir, flow.expr, field_name))
        .collect::<Vec<_>>();

    exprs.extend(
        hir.symbol_mutations_into(symbol)
            .filter_map(|mutation| match &mutation.kind {
                SymbolMutationKind::Field { name } if name == field_name => Some(mutation.value),
                _ => None,
            }),
    );

    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

fn field_value_exprs_from_object_expr(
    hir: &FileHir,
    expr: ExprId,
    field_name: &str,
) -> Vec<ExprId> {
    hir.object_fields
        .iter()
        .filter(|field| field.owner == expr && field.name == field_name)
        .filter_map(|field| field.value)
        .collect()
}

fn symbol_for_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Name => hir
            .reference_at(hir.expr(expr).range)
            .and_then(|reference| hir.definition_of(reference)),
        _ => None,
    }
}

fn direct_loop_body(hir: &FileHir, expr: ExprId) -> Option<BodyId> {
    let range = hir.expr(expr).range;
    hir.bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| hir.scope(body.scope).kind == ScopeKind::Loop)
        .filter(|(_, body)| {
            body.range.start() >= range.start()
                && body.range.end() <= range.end()
                && body.range != range
        })
        .max_by_key(|(_, body)| body.range.len())
        .map(|(index, _)| BodyId(index as u32))
}

fn largest_inner_expr(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let candidate = ExprId(*index as u32);
            candidate != expr
                && node.range.start() >= range.start()
                && node.range.end() <= range.end()
                && node.range != range
        })
        .max_by_key(|(_, node)| node.range.len())
        .map(|(index, _)| ExprId(index as u32))
}

fn trailing_inner_expr(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let candidate = ExprId(*index as u32);
            candidate != expr
                && node.range.end() == range.end()
                && node.range.start() >= range.start()
                && node.range != range
        })
        .max_by_key(|(_, node)| node.range.len())
        .map(|(index, _)| ExprId(index as u32))
}

fn qualified_path_name(hir: &FileHir, expr: ExprId) -> Option<String> {
    let range = hir.expr(expr).range;
    let mut parts = hir
        .references
        .iter()
        .filter(|reference| {
            matches!(
                reference.kind,
                rhai_hir::ReferenceKind::Name | rhai_hir::ReferenceKind::PathSegment
            ) && reference.range.start() >= range.start()
                && reference.range.end() <= range.end()
        })
        .collect::<Vec<_>>();

    parts.sort_by_key(|reference| reference.range.start());
    (!parts.is_empty()).then(|| {
        parts
            .into_iter()
            .map(|reference| reference.name.clone())
            .collect::<Vec<_>>()
            .join("::")
    })
}

fn host_method_signature_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let access = hir.member_access(expr)?;
    let method_name = hir.reference(access.field_reference).name.as_str();
    let receiver_ty = inferred_expr_type(hir, inference, access.receiver)?;
    host_method_signature_for_type(&receiver_ty, method_name, host_types, arg_types)
}

fn host_method_signature_for_type(
    ty: &TypeRef,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Union(items) => items
            .iter()
            .filter_map(|item| {
                host_method_signature_for_type(item, method_name, host_types, arg_types)
            })
            .reduce(join_function_signatures),
        TypeRef::Named(name) => {
            host_method_signature_for_name(name, method_name, host_types, arg_types)
        }
        TypeRef::Applied { name, .. } => {
            host_method_signature_for_name(name, method_name, host_types, arg_types)
        }
        _ => None,
    }
}

fn host_method_signature_for_name(
    type_name: &str,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let method = host_types
        .iter()
        .find(|ty| ty.name == type_name)?
        .methods
        .iter()
        .find(|method| method.name == method_name)?;

    let matching = method
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref().cloned())
        .filter(|signature| {
            arg_types.is_none_or(|arg_types| signature.params.len() == arg_types.len())
        })
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return None;
    }

    if let Some(arg_types) = arg_types
        && has_informative_arg_types(arg_types)
        && let Some(index) = best_matching_signature_index(matching.iter(), arg_types)
    {
        return matching.get(index).cloned();
    }

    matching.into_iter().reduce(join_function_signatures)
}

fn join_function_signatures(left: FunctionTypeRef, right: FunctionTypeRef) -> FunctionTypeRef {
    if left.params.len() != right.params.len() {
        return left;
    }

    FunctionTypeRef {
        params: left
            .params
            .iter()
            .zip(right.params.iter())
            .map(|(left, right)| join_types(left, right))
            .collect(),
        ret: Box::new(join_types(left.ret.as_ref(), right.ret.as_ref())),
    }
}

fn infer_additive_result(lhs: Option<&TypeRef>, rhs: Option<&TypeRef>) -> Option<TypeRef> {
    match (lhs?, rhs?) {
        (TypeRef::String, TypeRef::String) => Some(TypeRef::String),
        (left, right) => infer_numeric_result(Some(left), Some(right)),
    }
}

fn infer_numeric_result(lhs: Option<&TypeRef>, rhs: Option<&TypeRef>) -> Option<TypeRef> {
    let lhs = lhs?;
    let rhs = rhs?;
    if lhs == rhs && matches!(lhs, TypeRef::Int | TypeRef::Float | TypeRef::Decimal) {
        return Some(lhs.clone());
    }

    match (lhs, rhs) {
        (TypeRef::Decimal, TypeRef::Int | TypeRef::Float)
        | (TypeRef::Int | TypeRef::Float, TypeRef::Decimal) => Some(TypeRef::Decimal),
        (TypeRef::Float, TypeRef::Int) | (TypeRef::Int, TypeRef::Float) => Some(TypeRef::Float),
        _ => None,
    }
}

fn join_option_types(lhs: Option<&TypeRef>, rhs: Option<&TypeRef>) -> Option<TypeRef> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => Some(join_types(lhs, rhs)),
        (Some(lhs), None) => Some(lhs.clone()),
        (None, Some(rhs)) => Some(rhs.clone()),
        (None, None) => None,
    }
}

pub(crate) fn join_types(left: &TypeRef, right: &TypeRef) -> TypeRef {
    if left == right {
        return left.clone();
    }

    match (left, right) {
        (TypeRef::Unknown, other) | (TypeRef::Never, other) => other.clone(),
        (other, TypeRef::Unknown) | (other, TypeRef::Never) => other.clone(),
        (TypeRef::Array(left), TypeRef::Array(right)) => {
            TypeRef::Array(Box::new(join_types(left, right)))
        }
        (TypeRef::Map(left_key, left_value), TypeRef::Map(right_key, right_value)) => TypeRef::Map(
            Box::new(join_types(left_key, right_key)),
            Box::new(join_types(left_value, right_value)),
        ),
        (
            TypeRef::Function(FunctionTypeRef {
                params: left_params,
                ret: left_ret,
            }),
            TypeRef::Function(FunctionTypeRef {
                params: right_params,
                ret: right_ret,
            }),
        ) if left_params.len() == right_params.len() => TypeRef::Function(FunctionTypeRef {
            params: left_params
                .iter()
                .zip(right_params.iter())
                .map(|(left, right)| join_types(left, right))
                .collect(),
            ret: Box::new(join_types(left_ret, right_ret)),
        }),
        _ => make_union(left, right),
    }
}

fn make_union(left: &TypeRef, right: &TypeRef) -> TypeRef {
    let mut members = Vec::new();
    push_union_member(&mut members, left);
    push_union_member(&mut members, right);
    if members.len() == 1 {
        members.pop().expect("expected a single union member")
    } else {
        TypeRef::Union(members)
    }
}

fn push_union_member(members: &mut Vec<TypeRef>, ty: &TypeRef) {
    match ty {
        TypeRef::Union(items) => {
            for item in items {
                push_union_member(members, item);
            }
        }
        other if !members.iter().any(|existing| existing == other) => members.push(other.clone()),
        _ => {}
    }
}
