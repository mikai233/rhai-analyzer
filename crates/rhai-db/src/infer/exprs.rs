use std::collections::BTreeMap;

use crate::infer::calls::{
    builtin_universal_method_signature, callable_targets_for_call, effective_call_argument_types,
    imported_method_signature_for_expr, infer_fn_pointer_call_type, inferred_expr_type,
    is_builtin_fn_call,
};
use crate::infer::helpers::{
    infer_additive_result, infer_numeric_result, join_option_types, join_types,
    nested_mutation_container_type,
};
use crate::infer::loops::{
    block_expr_result_type, function_like_body_result_type, infer_loop_like_expr_type,
    join_expr_types,
};
use crate::infer::objects::{
    expr_is_unit_like, host_method_signature_for_expr, imported_module_member_type_for_expr,
    infer_member_type_from_expr, inferred_object_value_union, largest_inner_expr,
    qualified_path_name, string_literal_value, symbol_for_condition_expr, symbol_for_expr,
};
use crate::infer::propagation::merge_expr_type;
use crate::infer::{ImportedMethodSignature, ImportedModuleMember};
use crate::{FileTypeInference, HostFunction, HostType};
use rhai_hir::{
    AssignmentOperator, BinaryOperator, ExprId, ExprKind, ExternalSignatureIndex, FileHir,
    FunctionTypeRef, LiteralKind, SymbolId, SymbolKind, SymbolMutationKind, TypeRef, UnaryOperator,
};

pub(crate) fn infer_expr_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    imported_members: &[ImportedModuleMember],
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
            ExprKind::Path => infer_path_expr_type(hir, expr_id, external, imported_members),
            ExprKind::Name => infer_name_expr_type(hir, expr_id, inference, external),
            ExprKind::InterpolatedString => Some(TypeRef::String),
            ExprKind::Unary => infer_unary_expr_type(hir, inference, expr_id),
            ExprKind::Binary => infer_binary_expr_type(hir, inference, expr_id),
            ExprKind::Assign => infer_assign_expr_type(hir, inference, expr_id),
            ExprKind::Paren => infer_paren_expr_type(hir, inference, expr_id),
            ExprKind::Call => infer_call_expr_type(
                hir,
                expr_id,
                inference,
                external,
                globals,
                host_types,
                imported_methods,
            ),
            ExprKind::Index => infer_index_expr_type(hir, inference, expr_id),
            ExprKind::Field => {
                infer_field_expr_type(hir, inference, expr_id, host_types, imported_methods)
            }
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

pub(crate) fn infer_literal_expr_type(hir: &FileHir, expr: ExprId) -> Option<TypeRef> {
    let literal = hir.literal(expr)?;
    Some(match literal.kind {
        LiteralKind::Int => TypeRef::Int,
        LiteralKind::Float => TypeRef::Float,
        LiteralKind::String => TypeRef::String,
        LiteralKind::Char => TypeRef::Char,
        LiteralKind::Bool => TypeRef::Bool,
    })
}

pub(crate) fn infer_array_expr_type(
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

pub(crate) fn infer_object_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let mut fields = BTreeMap::new();
    for field in hir.object_fields.iter().filter(|field| field.owner == expr) {
        let value = field
            .value
            .and_then(|value| inferred_expr_type(hir, inference, value))
            .unwrap_or(TypeRef::Unknown);
        let merged = match fields.get(field.name.as_str()) {
            Some(current) => join_types(current, &value),
            None => value,
        };
        fields.insert(field.name.clone(), merged);
    }

    Some(TypeRef::Object(fields))
}

pub(crate) fn infer_block_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let block = hir.block_expr(expr)?;
    block_expr_result_type(hir, inference, block.body)
}

pub(crate) fn infer_if_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let if_expr = hir.if_expr(expr)?;
    join_expr_types(hir, inference, if_expr.then_branch, if_expr.else_branch)
}

pub(crate) fn infer_switch_expr_type(
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

pub(crate) fn infer_while_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

pub(crate) fn infer_loop_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, false)
}

pub(crate) fn infer_for_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

pub(crate) fn infer_do_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    infer_loop_like_expr_type(hir, inference, expr, true)
}

pub(crate) fn infer_path_expr_type(
    hir: &FileHir,
    expr: ExprId,
    external: &ExternalSignatureIndex,
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    if let Some(ty) = imported_module_member_type_for_expr(hir, expr, imported_members) {
        return Some(ty);
    }
    let qualified = qualified_path_name(hir, expr)?;
    external.get(qualified.as_str()).cloned()
}

pub(crate) fn infer_name_expr_type(
    hir: &FileHir,
    expr: ExprId,
    inference: &FileTypeInference,
    external: &ExternalSignatureIndex,
) -> Option<TypeRef> {
    let reference = hir.reference_at(hir.expr(expr).range)?;
    if hir.reference(reference).kind == rhai_hir::ReferenceKind::This {
        return hir.this_type_at(hir.expr(expr).range.start());
    }

    if let Some(target) = hir.definition_of(reference) {
        return refined_symbol_type_at_offset(hir, inference, target, hir.expr(expr).range.start())
            .or_else(|| inference.symbol_types.get(&target).cloned())
            .or_else(|| hir.declared_symbol_type(target).cloned());
    }

    external
        .get(hir.reference(reference).name.as_str())
        .cloned()
}

pub(crate) fn refined_symbol_type_at_offset(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    offset: rhai_syntax::TextSize,
) -> Option<TypeRef> {
    let mut ty = flow_sensitive_symbol_type_at_offset(hir, inference, symbol, offset)?;

    for if_expr in enclosing_if_exprs(hir, offset) {
        ty = refine_type_from_if_condition(hir, inference, &ty, symbol, if_expr, offset);
    }

    Some(ty)
}

pub(crate) fn flow_sensitive_symbol_type_at_offset(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    offset: rhai_syntax::TextSize,
) -> Option<TypeRef> {
    let skipped_if_exprs = skipped_complete_if_exprs(hir, inference, symbol, offset);
    let mut candidates = hir
        .value_flows_into(symbol)
        .filter(|flow| flow.range.start() < offset)
        .filter(|flow| !flow_is_shadowed_by_complete_if(flow.range, &skipped_if_exprs))
        .filter_map(|flow| {
            let ty = inferred_expr_type(hir, inference, flow.expr)?;
            Some(FlowStateCandidate {
                range: flow.range,
                ty,
                conditional: range_is_conditional_for_use(hir, flow.range, offset),
            })
        })
        .collect::<Vec<_>>();

    candidates.extend(
        hir.symbol_mutations_into(symbol)
            .filter(|mutation| mutation.range.start() < offset)
            .filter_map(|mutation| {
                let value_ty = inferred_expr_type(hir, inference, mutation.value)?;
                let ty = match &mutation.kind {
                    SymbolMutationKind::Path { segments } => {
                        nested_mutation_container_type(hir, inference, segments, value_ty)
                    }
                };
                Some(FlowStateCandidate {
                    range: mutation.range,
                    ty,
                    conditional: range_is_conditional_for_use(hir, mutation.range, offset),
                })
            }),
    );

    candidates.extend(
        skipped_if_exprs
            .iter()
            .cloned()
            .map(|if_candidate| FlowStateCandidate {
                range: if_candidate.range,
                ty: if_candidate.ty,
                conditional: false,
            }),
    );

    if let Some(annotation) = hir.declared_symbol_type(symbol).cloned() {
        candidates.push(FlowStateCandidate {
            range: hir.symbol(symbol).range,
            ty: annotation,
            conditional: false,
        });
    }

    candidates.sort_by_key(|candidate| candidate.range.start());
    if candidates.is_empty() {
        return None;
    }

    let baseline_index = candidates
        .iter()
        .rposition(|candidate| !candidate.conditional);
    let mut result = baseline_index
        .and_then(|index| candidates.get(index).map(|candidate| candidate.ty.clone()));

    let start_index = baseline_index.map(|index| index + 1).unwrap_or(0);
    for candidate in candidates.into_iter().skip(start_index) {
        result = Some(match result {
            Some(current) => join_types(&current, &candidate.ty),
            None => candidate.ty,
        });
    }

    result
}

pub(crate) fn enclosing_if_exprs(
    hir: &FileHir,
    offset: rhai_syntax::TextSize,
) -> Vec<&rhai_hir::IfExprInfo> {
    let mut items = hir
        .if_exprs
        .iter()
        .filter(|if_expr| hir.expr(if_expr.owner).range.contains(offset))
        .collect::<Vec<_>>();
    items.sort_by_key(|if_expr| hir.expr(if_expr.owner).range.len());
    items
}

pub(crate) fn refine_type_from_if_condition(
    hir: &FileHir,
    inference: &FileTypeInference,
    current: &TypeRef,
    symbol: SymbolId,
    if_expr: &rhai_hir::IfExprInfo,
    offset: rhai_syntax::TextSize,
) -> TypeRef {
    let Some(condition) = if_expr.condition else {
        return current.clone();
    };
    let branch = if if_expr
        .then_branch
        .is_some_and(|then_branch| hir.expr(then_branch).range.contains(offset))
    {
        BranchPolarity::Then
    } else if if_expr
        .else_branch
        .is_some_and(|else_branch| hir.expr(else_branch).range.contains(offset))
    {
        BranchPolarity::Else
    } else {
        return current.clone();
    };

    match branch_nullability_refinement(hir, inference, condition, symbol, branch) {
        Some(NullabilityRefinement::NonNull) => strip_nullable(current),
        Some(NullabilityRefinement::NullOnly) => {
            nullish_only_type(current).unwrap_or_else(|| current.clone())
        }
        None => current.clone(),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum BranchPolarity {
    Then,
    Else,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NullabilityRefinement {
    NonNull,
    NullOnly,
}

pub(crate) fn branch_nullability_refinement(
    hir: &FileHir,
    inference: &FileTypeInference,
    condition: ExprId,
    symbol: SymbolId,
    branch: BranchPolarity,
) -> Option<NullabilityRefinement> {
    match hir.expr(condition).kind {
        ExprKind::Name => (symbol_for_expr(hir, condition) == Some(symbol)
            && matches!(branch, BranchPolarity::Then))
        .then_some(NullabilityRefinement::NonNull),
        ExprKind::Paren => largest_inner_expr(hir, condition)
            .and_then(|inner| branch_nullability_refinement(hir, inference, inner, symbol, branch)),
        ExprKind::Unary => hir
            .unary_expr(condition)
            .filter(|unary| unary.operator == UnaryOperator::Not)
            .and_then(|unary| unary.operand)
            .and_then(|operand| {
                branch_nullability_refinement(
                    hir,
                    inference,
                    operand,
                    symbol,
                    match branch {
                        BranchPolarity::Then => BranchPolarity::Else,
                        BranchPolarity::Else => BranchPolarity::Then,
                    },
                )
            }),
        ExprKind::Binary => {
            let binary = hir.binary_expr(condition)?;
            match binary.operator {
                BinaryOperator::EqEq | BinaryOperator::NotEq => {
                    equality_nullability_refinement(hir, inference, binary, symbol, branch)
                }
                BinaryOperator::AndAnd if matches!(branch, BranchPolarity::Then) => {
                    combine_nullability_refinements(
                        binary.lhs.and_then(|lhs| {
                            branch_nullability_refinement(hir, inference, lhs, symbol, branch)
                        }),
                        binary.rhs.and_then(|rhs| {
                            branch_nullability_refinement(hir, inference, rhs, symbol, branch)
                        }),
                    )
                }
                BinaryOperator::OrOr if matches!(branch, BranchPolarity::Else) => {
                    combine_nullability_refinements(
                        binary.lhs.and_then(|lhs| {
                            branch_nullability_refinement(hir, inference, lhs, symbol, branch)
                        }),
                        binary.rhs.and_then(|rhs| {
                            branch_nullability_refinement(hir, inference, rhs, symbol, branch)
                        }),
                    )
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn strip_nullable(ty: &TypeRef) -> TypeRef {
    match ty {
        TypeRef::Nullable(inner) => strip_nullable(inner),
        TypeRef::Union(items) => {
            let mut refined = Vec::new();
            for item in items {
                if matches!(item, TypeRef::Unit) {
                    continue;
                }
                let next = strip_nullable(item);
                if !refined.iter().any(|existing| existing == &next) {
                    refined.push(next);
                }
            }
            match refined.len() {
                0 => TypeRef::Never,
                1 => refined.pop().expect("expected one refined member"),
                _ => TypeRef::Union(refined),
            }
        }
        _ => ty.clone(),
    }
}

pub(crate) fn nullish_only_type(ty: &TypeRef) -> Option<TypeRef> {
    match ty {
        TypeRef::Nullable(_) | TypeRef::Unit => Some(TypeRef::Unit),
        TypeRef::Union(items) => {
            let mut refined = Vec::new();
            for item in items {
                let Some(next) = nullish_only_type(item) else {
                    continue;
                };
                if !refined.iter().any(|existing| existing == &next) {
                    refined.push(next);
                }
            }

            match refined.len() {
                0 => None,
                1 => refined.pop(),
                _ => Some(TypeRef::Union(refined)),
            }
        }
        _ => None,
    }
}

pub(crate) fn combine_nullability_refinements(
    left: Option<NullabilityRefinement>,
    right: Option<NullabilityRefinement>,
) -> Option<NullabilityRefinement> {
    match (left, right) {
        (Some(left), Some(right)) if left == right => Some(left),
        (Some(refinement), None) | (None, Some(refinement)) => Some(refinement),
        _ => None,
    }
}

pub(crate) fn equality_nullability_refinement(
    hir: &FileHir,
    inference: &FileTypeInference,
    binary: &rhai_hir::BinaryExprInfo,
    symbol: SymbolId,
    branch: BranchPolarity,
) -> Option<NullabilityRefinement> {
    let lhs = binary.lhs?;
    let rhs = binary.rhs?;
    let compares_symbol_to_unit = (symbol_for_condition_expr(hir, lhs) == Some(symbol)
        && expr_is_unit_like(hir, inference, rhs))
        || (symbol_for_condition_expr(hir, rhs) == Some(symbol)
            && expr_is_unit_like(hir, inference, lhs));

    if !compares_symbol_to_unit {
        return None;
    }

    match (binary.operator, branch) {
        (BinaryOperator::EqEq, BranchPolarity::Then)
        | (BinaryOperator::NotEq, BranchPolarity::Else) => Some(NullabilityRefinement::NullOnly),
        (BinaryOperator::EqEq, BranchPolarity::Else)
        | (BinaryOperator::NotEq, BranchPolarity::Then) => Some(NullabilityRefinement::NonNull),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct FlowStateCandidate {
    range: rhai_syntax::TextRange,
    ty: TypeRef,
    conditional: bool,
}

#[derive(Clone)]
pub(crate) struct CompleteIfOverwrite {
    range: rhai_syntax::TextRange,
    ty: TypeRef,
}

pub(crate) fn skipped_complete_if_exprs(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    offset: rhai_syntax::TextSize,
) -> Vec<CompleteIfOverwrite> {
    hir.if_exprs
        .iter()
        .filter_map(|if_expr| {
            complete_if_overwrite_candidate(hir, inference, symbol, if_expr, offset)
        })
        .collect()
}

pub(crate) fn complete_if_overwrite_candidate(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    if_expr: &rhai_hir::IfExprInfo,
    offset: rhai_syntax::TextSize,
) -> Option<CompleteIfOverwrite> {
    let range = hir.expr(if_expr.owner).range;
    if range.end() > offset || range.contains(offset) {
        return None;
    }

    let then_branch = if_expr.then_branch?;
    let else_branch = if_expr.else_branch?;
    let then_ty = joined_flow_type_in_expr(hir, inference, symbol, then_branch)?;
    let else_ty = joined_flow_type_in_expr(hir, inference, symbol, else_branch)?;

    Some(CompleteIfOverwrite {
        range,
        ty: join_types(&then_ty, &else_ty),
    })
}

pub(crate) fn joined_flow_type_in_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    expr: ExprId,
) -> Option<TypeRef> {
    let range = hir.expr(expr).range;
    hir.value_flows_into(symbol)
        .filter(|flow| flow.range.start() >= range.start() && flow.range.end() <= range.end())
        .filter_map(|flow| inferred_expr_type(hir, inference, flow.expr))
        .reduce(|left, right| join_types(&left, &right))
}

pub(crate) fn flow_is_shadowed_by_complete_if(
    flow_range: rhai_syntax::TextRange,
    complete_if_exprs: &[CompleteIfOverwrite],
) -> bool {
    complete_if_exprs
        .iter()
        .any(|candidate| candidate.range.contains_range(flow_range))
}

pub(crate) fn range_is_conditional_for_use(
    hir: &FileHir,
    range: rhai_syntax::TextRange,
    offset: rhai_syntax::TextSize,
) -> bool {
    hir.exprs.iter().any(|expr| {
        matches!(
            expr.kind,
            ExprKind::If
                | ExprKind::Switch
                | ExprKind::While
                | ExprKind::Loop
                | ExprKind::For
                | ExprKind::Do
        ) && expr.range.contains_range(range)
            && !expr.range.contains(offset)
    })
}

pub(crate) fn infer_unary_expr_type(
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

pub(crate) fn infer_binary_expr_type(
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

pub(crate) fn infer_assign_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let assign = hir.assign_expr(expr)?;
    let lhs = assign
        .lhs
        .and_then(|lhs| inferred_expr_type(hir, inference, lhs));
    let rhs = assign
        .rhs
        .and_then(|rhs| inferred_expr_type(hir, inference, rhs));

    match assign.operator {
        AssignmentOperator::Assign => rhs,
        AssignmentOperator::NullCoalesce => join_option_types(lhs.as_ref(), rhs.as_ref()),
        AssignmentOperator::Add => infer_additive_result(lhs.as_ref(), rhs.as_ref()),
        AssignmentOperator::Subtract
        | AssignmentOperator::Multiply
        | AssignmentOperator::Divide
        | AssignmentOperator::Remainder
        | AssignmentOperator::Power
        | AssignmentOperator::ShiftLeft
        | AssignmentOperator::ShiftRight
        | AssignmentOperator::Or
        | AssignmentOperator::Xor
        | AssignmentOperator::And => infer_numeric_result(lhs.as_ref(), rhs.as_ref()),
    }
}

pub(crate) fn infer_paren_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    largest_inner_expr(hir, expr).and_then(|inner| inferred_expr_type(hir, inference, inner))
}

pub(crate) fn infer_call_expr_type(
    hir: &FileHir,
    expr: ExprId,
    inference: &FileTypeInference,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
) -> Option<TypeRef> {
    let call = hir
        .calls
        .iter()
        .find(|call| call.range == hir.expr(expr).range)?;
    let arg_types = effective_call_argument_types(hir, inference, call);

    if is_builtin_fn_call(hir, call) {
        return Some(infer_fn_pointer_call_type(
            hir, inference, call, external, globals,
        ));
    }

    if let Some(ret) = callable_targets_for_call(
        hir,
        inference,
        call,
        external,
        globals,
        host_types,
        imported_methods,
        Some(&arg_types),
    )
    .into_iter()
    .map(|target| (*target.signature.ret).clone())
    .reduce(|left, right| join_types(&left, &right))
    {
        return Some(ret);
    }
    None
}

pub(crate) fn infer_index_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let index = hir.index_expr(expr)?;
    match inferred_expr_type(hir, inference, index.receiver?)? {
        TypeRef::Array(inner) => Some(*inner),
        TypeRef::Map(_, value) => Some(*value),
        TypeRef::Object(fields) => {
            let key = index
                .index
                .and_then(|index_expr| string_literal_value(hir, index_expr));
            key.and_then(|key| fields.get(key).cloned())
                .or_else(|| inferred_object_value_union(&fields))
        }
        _ => None,
    }
}

pub(crate) fn infer_field_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
) -> Option<TypeRef> {
    let access = hir.member_access(expr)?;
    let field_name = hir.reference(access.field_reference).name.as_str();
    infer_member_type_from_expr(hir, inference, access.receiver, field_name).or_else(|| {
        host_method_signature_for_expr(hir, inference, expr, host_types, None)
            .map(TypeRef::Function)
            .or_else(|| {
                imported_method_signature_for_expr(hir, inference, expr, imported_methods, None)
                    .map(TypeRef::Function)
            })
            .or_else(|| builtin_universal_method_signature(field_name).map(TypeRef::Function))
    })
}

pub(crate) fn infer_closure_expr_type(
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
