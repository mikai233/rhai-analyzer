use crate::builtin::semantics::{
    infer_fn_pointer_call_type, is_builtin_fn_call, refine_type_from_type_of_condition_for_target,
    refine_type_from_type_of_switch_for_target,
};
use crate::builtin::signatures::builtin_universal_method_signature;
use crate::{
    BuiltinSemanticKey, builtin_assignment_semantic_key, builtin_binary_semantic_key,
    builtin_index_semantic_key, builtin_unary_semantic_key,
};
use std::collections::BTreeMap;

use crate::infer::calls::{
    callable_targets_for_call, effective_call_argument_types, imported_method_signature_for_expr,
    inferred_expr_type,
};
use crate::infer::helpers::{
    ReadTargetKey, array_item_type, infer_map_merge_result, infer_numeric_result,
    join_option_types, join_types, make_ambiguous_type, nested_mutation_container_type,
    symbol_target_key,
};
use crate::infer::loops::{
    block_expr_result_type, function_like_body_result_type, infer_loop_like_expr_type,
    join_expr_types,
};
use crate::infer::objects::{
    expr_is_unit_like, host_method_signature_for_expr, imported_module_member_type_for_expr,
    infer_member_type_from_expr, infer_symbol_read_type, inferred_object_value_union,
    largest_inner_expr, qualified_path_name, receiver_supports_field_method_ambiguity,
    string_literal_value,
};
use crate::infer::propagation::{merge_expr_type, set_expr_type};
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
            ExprKind::Path => {
                infer_path_expr_type(hir, expr_id, inference, external, imported_members)
            }
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
            changed |= match expr.kind {
                ExprKind::Block
                | ExprKind::If
                | ExprKind::Switch
                | ExprKind::Call
                | ExprKind::Name
                | ExprKind::Field
                | ExprKind::Index
                | ExprKind::Paren => set_expr_type(hir, inference, expr_id, ty),
                _ => merge_expr_type(hir, inference, expr_id, ty),
            };
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
    inference: &FileTypeInference,
    external: &ExternalSignatureIndex,
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    if let Some(ty) = infer_rooted_global_constant_type(hir, inference, expr) {
        return Some(ty);
    }
    if let Some(ty) = imported_module_member_type_for_expr(hir, expr, imported_members) {
        return Some(ty);
    }
    let qualified = qualified_path_name(hir, expr)?;
    external.get(qualified.as_str()).cloned()
}

fn infer_rooted_global_constant_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let path = hir.path_expr(expr)?;
    if !path.rooted_global {
        return None;
    }

    let parts = hir.qualified_path_parts(expr)?;
    let [name] = parts.as_slice() else {
        return None;
    };

    hir.symbols.iter().enumerate().find_map(|(index, symbol)| {
        (symbol.kind == SymbolKind::Constant
            && hir.scope(symbol.scope).kind == rhai_hir::ScopeKind::File
            && symbol.name == *name)
            .then(|| {
                let symbol_id = SymbolId(index as u32);
                inference
                    .symbol_types
                    .get(&symbol_id)
                    .cloned()
                    .or_else(|| hir.declared_symbol_type(symbol_id).cloned())
            })
            .flatten()
    })
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
        if hir.symbol(target).kind == SymbolKind::Function {
            let overloads = hir
                .visible_function_overloads_for_reference(reference)
                .into_iter()
                .filter_map(|symbol| {
                    inference
                        .symbol_types
                        .get(&symbol)
                        .cloned()
                        .or_else(|| hir.declared_symbol_type(symbol).cloned())
                })
                .collect::<Vec<_>>();
            if !overloads.is_empty() {
                return Some(make_ambiguous_type(overloads));
            }
        }

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
    let ty = flow_sensitive_symbol_type_at_offset(hir, inference, symbol, offset)?;
    Some(refined_target_type_at_offset(
        hir,
        inference,
        &ty,
        &symbol_target_key(symbol),
        offset,
    ))
}

pub(crate) fn refined_target_type_at_offset(
    hir: &FileHir,
    inference: &FileTypeInference,
    current: &TypeRef,
    target: &ReadTargetKey,
    offset: rhai_syntax::TextSize,
) -> TypeRef {
    let mut ty = current.clone();

    for if_expr in enclosing_if_exprs(hir, offset) {
        ty = refine_type_from_if_condition_for_target(hir, inference, &ty, target, if_expr, offset);
    }

    for switch_expr in enclosing_switch_exprs(hir, offset) {
        ty = refine_type_from_type_of_switch_for_target(hir, &ty, switch_expr, target, offset);
    }

    ty
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

pub(crate) fn enclosing_switch_exprs(
    hir: &FileHir,
    offset: rhai_syntax::TextSize,
) -> Vec<&rhai_hir::SwitchExprInfo> {
    let mut items = hir
        .switch_exprs
        .iter()
        .filter(|switch_expr| hir.expr(switch_expr.owner).range.contains(offset))
        .collect::<Vec<_>>();
    items.sort_by_key(|switch_expr| hir.expr(switch_expr.owner).range.len());
    items
}

pub(crate) fn refine_type_from_if_condition_for_target(
    hir: &FileHir,
    inference: &FileTypeInference,
    current: &TypeRef,
    target: &ReadTargetKey,
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

    let current =
        match branch_nullability_refinement_for_target(hir, inference, condition, target, branch) {
            Some(NullabilityRefinement::NonNull) => strip_nullable(current),
            Some(NullabilityRefinement::NullOnly) => {
                nullish_only_type(current).unwrap_or_else(|| current.clone())
            }
            None => current.clone(),
        };

    refine_type_from_type_of_condition_for_target(
        hir,
        &current,
        condition,
        target,
        matches!(branch, BranchPolarity::Then),
    )
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

pub(crate) fn branch_nullability_refinement_for_target(
    hir: &FileHir,
    inference: &FileTypeInference,
    condition: ExprId,
    target: &ReadTargetKey,
    branch: BranchPolarity,
) -> Option<NullabilityRefinement> {
    match hir.expr(condition).kind {
        ExprKind::Name | ExprKind::Field | ExprKind::Index => {
            (crate::infer::helpers::read_target_key_for_expr(hir, condition).as_ref()
                == Some(target)
                && matches!(branch, BranchPolarity::Then))
            .then_some(NullabilityRefinement::NonNull)
        }
        ExprKind::Paren => largest_inner_expr(hir, condition).and_then(|inner| {
            branch_nullability_refinement_for_target(hir, inference, inner, target, branch)
        }),
        ExprKind::Unary => hir
            .unary_expr(condition)
            .filter(|unary| unary.operator == UnaryOperator::Not)
            .and_then(|unary| unary.operand)
            .and_then(|operand| {
                branch_nullability_refinement_for_target(
                    hir,
                    inference,
                    operand,
                    target,
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
                    equality_nullability_refinement_for_target(
                        hir, inference, binary, target, branch,
                    )
                }
                BinaryOperator::AndAnd if matches!(branch, BranchPolarity::Then) => {
                    combine_nullability_refinements(
                        binary.lhs.and_then(|lhs| {
                            branch_nullability_refinement_for_target(
                                hir, inference, lhs, target, branch,
                            )
                        }),
                        binary.rhs.and_then(|rhs| {
                            branch_nullability_refinement_for_target(
                                hir, inference, rhs, target, branch,
                            )
                        }),
                    )
                }
                BinaryOperator::OrOr if matches!(branch, BranchPolarity::Else) => {
                    combine_nullability_refinements(
                        binary.lhs.and_then(|lhs| {
                            branch_nullability_refinement_for_target(
                                hir, inference, lhs, target, branch,
                            )
                        }),
                        binary.rhs.and_then(|rhs| {
                            branch_nullability_refinement_for_target(
                                hir, inference, rhs, target, branch,
                            )
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

pub(crate) fn equality_nullability_refinement_for_target(
    hir: &FileHir,
    inference: &FileTypeInference,
    binary: &rhai_hir::BinaryExprInfo,
    target: &ReadTargetKey,
    branch: BranchPolarity,
) -> Option<NullabilityRefinement> {
    let lhs = binary.lhs?;
    let rhs = binary.rhs?;
    let compares_symbol_to_unit =
        (crate::infer::helpers::read_target_key_for_expr(hir, lhs).as_ref() == Some(target)
            && expr_is_unit_like(hir, inference, rhs))
            || (crate::infer::helpers::read_target_key_for_expr(hir, rhs).as_ref() == Some(target)
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

    match builtin_unary_semantic_key(unary.operator, Some(&operand))? {
        BuiltinSemanticKey::LogicalNotBool => Some(TypeRef::Bool),
        BuiltinSemanticKey::UnaryPlusNumber | BuiltinSemanticKey::UnaryMinusNumber => Some(operand),
        _ => None,
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

    match builtin_binary_semantic_key(binary.operator, lhs.as_ref(), rhs.as_ref())? {
        BuiltinSemanticKey::ContainsArray
        | BuiltinSemanticKey::ContainsString
        | BuiltinSemanticKey::ContainsBlob
        | BuiltinSemanticKey::ContainsMap
        | BuiltinSemanticKey::ContainsRange
        | BuiltinSemanticKey::EqualityString
        | BuiltinSemanticKey::EqualityScalar
        | BuiltinSemanticKey::EqualityContainer
        | BuiltinSemanticKey::ComparisonString
        | BuiltinSemanticKey::ComparisonNumber => Some(TypeRef::Bool),
        BuiltinSemanticKey::RangeOperator => Some(TypeRef::Range),
        BuiltinSemanticKey::RangeInclusiveOperator => Some(TypeRef::RangeInclusive),
        BuiltinSemanticKey::NullCoalesce => join_option_types(lhs.as_ref(), rhs.as_ref()),
        BuiltinSemanticKey::NumericAddition | BuiltinSemanticKey::NumericArithmetic => {
            infer_numeric_result(lhs.as_ref(), rhs.as_ref())
        }
        BuiltinSemanticKey::StringConcatenation => Some(TypeRef::String),
        BuiltinSemanticKey::ArrayConcatenation => {
            let lhs_item = lhs.as_ref().and_then(array_item_type);
            let rhs_item = rhs.as_ref().and_then(array_item_type);
            Some(TypeRef::Array(Box::new(join_option_types(
                lhs_item.as_ref(),
                rhs_item.as_ref(),
            )?)))
        }
        BuiltinSemanticKey::BlobConcatenation => Some(TypeRef::Blob),
        BuiltinSemanticKey::MapMerge => infer_map_merge_result(lhs.as_ref(), rhs.as_ref()),
        _ => None,
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

    if assign.operator == AssignmentOperator::Assign {
        return rhs;
    }

    match builtin_assignment_semantic_key(assign.operator, lhs.as_ref(), rhs.as_ref())? {
        BuiltinSemanticKey::NullCoalesceAssignment => join_option_types(lhs.as_ref(), rhs.as_ref()),
        BuiltinSemanticKey::NumericAssignment | BuiltinSemanticKey::BitwiseAssignment => {
            infer_numeric_result(lhs.as_ref(), rhs.as_ref())
        }
        BuiltinSemanticKey::StringAppendAssignment => Some(TypeRef::String),
        BuiltinSemanticKey::ArrayAppendAssignment => {
            let lhs_item = lhs.as_ref().and_then(array_item_type);
            let rhs_item = rhs.as_ref().and_then(array_item_type);
            Some(TypeRef::Array(Box::new(join_option_types(
                lhs_item.as_ref(),
                rhs_item.as_ref(),
            )?)))
        }
        BuiltinSemanticKey::BlobAppendAssignment => Some(TypeRef::Blob),
        _ => None,
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

    let returns = callable_targets_for_call(
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
    .collect::<Vec<_>>();
    if !returns.is_empty() {
        return Some(make_ambiguous_type(returns));
    }
    None
}

pub(crate) fn infer_index_expr_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    if let Some(ty) = infer_symbol_read_type(hir, inference, expr) {
        return Some(ty);
    }

    let index = hir.index_expr(expr)?;
    let receiver_ty = inferred_expr_type(hir, inference, index.receiver?)?;
    let index_ty = index
        .index
        .and_then(|index_expr| inferred_expr_type(hir, inference, index_expr));

    match builtin_index_semantic_key(&receiver_ty, index_ty.as_ref())? {
        BuiltinSemanticKey::ArrayIndex => array_item_type(&receiver_ty),
        BuiltinSemanticKey::ArrayRangeIndex => {
            Some(TypeRef::Array(Box::new(array_item_type(&receiver_ty)?)))
        }
        BuiltinSemanticKey::BlobIndex => Some(TypeRef::Int),
        BuiltinSemanticKey::BlobRangeIndex => Some(TypeRef::Blob),
        BuiltinSemanticKey::StringIndex => Some(TypeRef::Char),
        BuiltinSemanticKey::StringRangeIndex => Some(TypeRef::String),
        BuiltinSemanticKey::MapIndex => match receiver_ty {
            TypeRef::Map(_, value) => Some(*value),
            TypeRef::Object(fields) => {
                let key = index
                    .index
                    .and_then(|index_expr| string_literal_value(hir, index_expr));
                key.and_then(|key| fields.get(key).cloned())
                    .or_else(|| inferred_object_value_union(&fields))
            }
            _ => None,
        },
        BuiltinSemanticKey::IntBitIndex => Some(TypeRef::Bool),
        BuiltinSemanticKey::IntBitRangeIndex => Some(TypeRef::Int),
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
    let tag_property_is_builtin = field_name == "tag"
        && !receiver_supports_field_method_ambiguity(hir, inference, access.receiver);
    let read_ty = if tag_property_is_builtin {
        let builtin_property_ty = Some(TypeRef::Int);
        let base_read_ty = infer_symbol_read_type(hir, inference, expr)
            .or_else(|| infer_member_type_from_expr(hir, inference, access.receiver, field_name));
        join_option_types(base_read_ty.as_ref(), builtin_property_ty.as_ref())
    } else {
        infer_symbol_read_type(hir, inference, expr)
            .or_else(|| infer_member_type_from_expr(hir, inference, access.receiver, field_name))
    };
    let method_ty = host_method_signature_for_expr(hir, inference, expr, host_types, None)
        .map(TypeRef::Function)
        .or_else(|| {
            imported_method_signature_for_expr(hir, inference, expr, imported_methods, None)
                .map(TypeRef::Function)
        })
        .or_else(|| builtin_universal_method_signature(field_name).map(TypeRef::Function));

    match (read_ty, method_ty) {
        (Some(read_ty), Some(method_ty))
            if receiver_supports_field_method_ambiguity(hir, inference, access.receiver) =>
        {
            Some(join_types(&read_ty, &method_ty))
        }
        (Some(read_ty), _) => Some(read_ty),
        (None, Some(method_ty)) => Some(method_ty),
        (None, None) => None,
    }
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
