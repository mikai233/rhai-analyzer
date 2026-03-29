use std::collections::{BTreeMap, HashMap};

use rhai_hir::{
    AssignmentOperator, BinaryOperator, BodyId, ControlFlowKind, ExprId, ExprKind,
    ExternalSignatureIndex, FileHir, FunctionTypeRef, LiteralKind, MutationPathSegment, ScopeKind,
    SymbolId, SymbolKind, SymbolMutationKind, TypeRef, UnaryOperator,
};

use crate::{FileTypeInference, HostFunction, HostType, best_matching_signature_index};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedMethodSignature {
    pub name: String,
    pub receiver: TypeRef,
    pub signature: FunctionTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedModuleMember {
    pub module_path: Vec<String>,
    pub name: String,
    pub ty: TypeRef,
}

pub(crate) fn infer_file_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    imported_members: &[ImportedModuleMember],
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

        changed |= infer_expr_types(
            hir,
            external,
            globals,
            host_types,
            imported_methods,
            imported_members,
            &mut inference,
        );
        changed |= propagate_for_binding_types(hir, &mut inference);
        changed |= propagate_expected_types(
            hir,
            external,
            globals,
            host_types,
            imported_methods,
            &mut inference,
        );
        changed |= propagate_call_argument_types(hir, imported_methods, &mut inference);
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
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    if let Some(ty) = imported_module_member_type_for_expr(hir, expr, imported_members) {
        return Some(ty);
    }
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

fn refined_symbol_type_at_offset(
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

fn flow_sensitive_symbol_type_at_offset(
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

fn enclosing_if_exprs(hir: &FileHir, offset: rhai_syntax::TextSize) -> Vec<&rhai_hir::IfExprInfo> {
    let mut items = hir
        .if_exprs
        .iter()
        .filter(|if_expr| hir.expr(if_expr.owner).range.contains(offset))
        .collect::<Vec<_>>();
    items.sort_by_key(|if_expr| hir.expr(if_expr.owner).range.len());
    items
}

fn refine_type_from_if_condition(
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
enum BranchPolarity {
    Then,
    Else,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NullabilityRefinement {
    NonNull,
    NullOnly,
}

fn branch_nullability_refinement(
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

fn strip_nullable(ty: &TypeRef) -> TypeRef {
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

fn nullish_only_type(ty: &TypeRef) -> Option<TypeRef> {
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

fn combine_nullability_refinements(
    left: Option<NullabilityRefinement>,
    right: Option<NullabilityRefinement>,
) -> Option<NullabilityRefinement> {
    match (left, right) {
        (Some(left), Some(right)) if left == right => Some(left),
        (Some(refinement), None) | (None, Some(refinement)) => Some(refinement),
        _ => None,
    }
}

fn equality_nullability_refinement(
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
struct FlowStateCandidate {
    range: rhai_syntax::TextRange,
    ty: TypeRef,
    conditional: bool,
}

#[derive(Clone)]
struct CompleteIfOverwrite {
    range: rhai_syntax::TextRange,
    ty: TypeRef,
}

fn skipped_complete_if_exprs(
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

fn complete_if_overwrite_candidate(
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

fn joined_flow_type_in_expr(
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

fn flow_is_shadowed_by_complete_if(
    flow_range: rhai_syntax::TextRange,
    complete_if_exprs: &[CompleteIfOverwrite],
) -> bool {
    complete_if_exprs
        .iter()
        .any(|candidate| candidate.range.contains_range(flow_range))
}

fn range_is_conditional_for_use(
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

fn infer_index_expr_type(
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

fn infer_field_expr_type(
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

fn propagate_call_argument_types(
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

fn propagate_expected_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    inference: &mut FileTypeInference,
) -> bool {
    let mut changed = false;

    for flow in &hir.value_flows {
        let Some(expected) = inference.symbol_types.get(&flow.symbol).cloned() else {
            continue;
        };
        changed |= propagate_expected_type_to_expr(hir, inference, flow.expr, &expected);
    }

    for (index, symbol) in hir.symbols.iter().enumerate() {
        if symbol.kind != SymbolKind::Function {
            continue;
        }

        let symbol_id = SymbolId(index as u32);
        let Some(TypeRef::Function(signature)) = inference.symbol_types.get(&symbol_id).cloned()
        else {
            continue;
        };
        let Some(body) = hir.body_of(symbol_id) else {
            continue;
        };

        changed |=
            propagate_expected_type_to_body_results(hir, inference, body, signature.ret.as_ref());
    }

    for call in &hir.calls {
        let Some(signature) = expected_call_signature(
            hir,
            inference,
            call,
            external,
            globals,
            host_types,
            imported_methods,
        ) else {
            continue;
        };

        for (arg_expr, expected) in effective_call_argument_exprs(hir, call)
            .iter()
            .copied()
            .zip(signature.params.iter())
        {
            changed |= propagate_expected_type_to_expr(hir, inference, arg_expr, expected);
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

fn propagate_for_binding_types(hir: &FileHir, inference: &mut FileTypeInference) -> bool {
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
            SymbolMutationKind::Path { segments } => {
                nested_mutation_container_type(hir, inference, segments, value_ty)
            }
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

fn merge_expr_type_if_refinable(
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

fn merge_symbol_type_if_refinable(
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

fn propagate_expected_type_to_expr(
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

fn propagate_expected_type_to_body_results(
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

fn propagate_expected_function_type_to_closure(
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

#[derive(Clone)]
struct CallableTarget {
    signature: FunctionTypeRef,
    local_symbol: Option<SymbolId>,
}

fn call_builtin_fn_signature(globals: &[HostFunction]) -> Option<&FunctionTypeRef> {
    globals
        .iter()
        .find(|function| function.name == "Fn")?
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .find(|signature| signature.params.len() == 1)
}

fn is_builtin_fn_call(hir: &FileHir, call: &rhai_hir::CallSite) -> bool {
    call.callee_reference
        .map(|reference_id| hir.reference(reference_id).name.as_str())
        == Some("Fn")
}

fn infer_fn_pointer_call_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
) -> TypeRef {
    let Some(name_expr) = call.arg_exprs.first().copied() else {
        return TypeRef::FnPtr;
    };
    let Some(name) = string_literal_value(hir, name_expr) else {
        return TypeRef::FnPtr;
    };

    let targets = named_callable_targets_at_offset(
        hir,
        inference,
        name,
        call.range.start(),
        external,
        globals,
        None,
        &mut Vec::new(),
    );

    join_callable_target_signatures(&targets, None)
        .map(TypeRef::Function)
        .unwrap_or(TypeRef::FnPtr)
}

#[allow(clippy::too_many_arguments)]
fn callable_targets_for_call(
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
        && let Some(target_expr) = call.arg_exprs.first().copied()
    {
        targets.extend(callable_targets_for_expr(
            hir,
            inference,
            target_expr,
            call.range.start(),
            external,
            globals,
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
            imported_methods,
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
    }

    if let Some(callee_expr) = call.callee_range.and_then(|range| hir.expr_at(range)) {
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
            arg_types.as_deref(),
            &mut Vec::new(),
        ));
    }

    dedup_callable_targets(targets)
}

#[allow(clippy::too_many_arguments)]
fn callable_targets_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    match hir.expr(expr).kind {
        ExprKind::Name => symbol_for_expr(hir, expr)
            .map(|symbol| {
                callable_targets_for_symbol_use(
                    hir,
                    inference,
                    symbol,
                    use_offset,
                    external,
                    globals,
                    imported_methods,
                    arg_types,
                    visited_symbols,
                )
            })
            .unwrap_or_default(),
        ExprKind::Field => local_method_targets_for_expr(
            hir,
            inference,
            expr,
            use_offset,
            external,
            globals,
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
                    imported_methods,
                    arg_types,
                    visited_symbols,
                )
            })
            .unwrap_or_default(),
        ExprKind::Closure => inferred_expr_type(hir, inference, expr)
            .and_then(|ty| signature_from_type(&ty))
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
                    arg_types,
                    visited_symbols,
                ))
            })
            .unwrap_or_default(),
        _ => inferred_expr_type(hir, inference, expr)
            .and_then(|ty| signature_from_type(&ty))
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
fn callable_targets_for_symbol_use(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    imported_methods: &[ImportedMethodSignature],
    arg_types: Option<&[Option<TypeRef>]>,
    visited_symbols: &mut Vec<SymbolId>,
) -> Vec<CallableTarget> {
    if visited_symbols.contains(&symbol) {
        return Vec::new();
    }
    visited_symbols.push(symbol);

    let mut targets = callable_signature_for_symbol(hir, inference, symbol, external)
        .map(|signature| {
            vec![CallableTarget {
                signature,
                local_symbol: (hir.symbol(symbol).kind == SymbolKind::Function).then_some(symbol),
            }]
        })
        .unwrap_or_default();

    for flow in hir
        .value_flows_into(symbol)
        .filter(|flow| flow.range.start() < use_offset)
    {
        targets.extend(callable_targets_for_expr(
            hir,
            inference,
            flow.expr,
            use_offset,
            external,
            globals,
            imported_methods,
            arg_types,
            visited_symbols,
        ));
    }

    visited_symbols.pop();
    dedup_callable_targets(targets)
}

#[allow(clippy::too_many_arguments)]
fn local_method_targets_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
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
        imported_methods,
        arg_types,
        visited_symbols,
    )
}

#[allow(clippy::too_many_arguments)]
fn local_method_targets_for_name(
    hir: &FileHir,
    inference: &FileTypeInference,
    name: &str,
    receiver_ty: &TypeRef,
    use_offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
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

fn imported_method_signature_for_expr(
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

fn imported_method_targets_for_name(
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
        && let Some(index) = best_matching_signature_index(
            matching.iter().map(|method| &method.signature),
            arg_types,
        )
        && let Some(method) = matching.get(index)
    {
        targets.push(CallableTarget {
            signature: method.signature.clone(),
            local_symbol: None,
        });
        return targets;
    }

    targets.extend(matching.into_iter().map(|method| CallableTarget {
        signature: method.signature,
        local_symbol: None,
    }));
    targets
}

fn builtin_universal_method_targets(
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

fn builtin_universal_method_signature(method_name: &str) -> Option<FunctionTypeRef> {
    match method_name {
        "type_of" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::String),
        }),
        _ => None,
    }
}

fn callable_signature_for_symbol(
    hir: &FileHir,
    inference: &FileTypeInference,
    symbol: SymbolId,
    external: &ExternalSignatureIndex,
) -> Option<FunctionTypeRef> {
    inference
        .symbol_types
        .get(&symbol)
        .or_else(|| hir.declared_symbol_type(symbol))
        .or_else(|| external.get(hir.symbol(symbol).name.as_str()))
        .and_then(signature_from_type)
}

#[allow(clippy::too_many_arguments)]
fn named_callable_targets_at_offset(
    hir: &FileHir,
    inference: &FileTypeInference,
    name: &str,
    offset: rhai_syntax::TextSize,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
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
                &[],
                arg_types,
                visited_symbols,
            ));
        }
        return dedup_callable_targets(targets);
    }

    let mut targets = Vec::new();

    if let Some(arg_types) = arg_types {
        if let Some(signature) = global_signature_for_call(globals, name, arg_types) {
            targets.push(CallableTarget {
                signature: signature.clone(),
                local_symbol: None,
            });
        }
    } else if let Some(signature) = global_signature_for_pointer(globals, name) {
        targets.push(CallableTarget {
            signature,
            local_symbol: None,
        });
    }

    if let Some(TypeRef::Function(signature)) = external.get(name) {
        targets.push(CallableTarget {
            signature: signature.clone(),
            local_symbol: None,
        });
    }

    dedup_callable_targets(targets)
}

fn global_signature_for_pointer(globals: &[HostFunction], name: &str) -> Option<FunctionTypeRef> {
    let signatures = globals
        .iter()
        .find(|function| function.name == name)?
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref().cloned())
        .collect::<Vec<_>>();

    join_function_signatures_if_compatible(signatures, None)
}

fn join_callable_target_signatures(
    targets: &[CallableTarget],
    arg_count: Option<usize>,
) -> Option<FunctionTypeRef> {
    join_function_signatures_if_compatible(
        targets
            .iter()
            .map(|target| target.signature.clone())
            .collect(),
        arg_count,
    )
}

fn join_function_signatures_if_compatible(
    signatures: Vec<FunctionTypeRef>,
    arg_count: Option<usize>,
) -> Option<FunctionTypeRef> {
    let mut signatures = signatures
        .into_iter()
        .filter(|signature| arg_count.is_none_or(|count| signature.params.len() == count))
        .collect::<Vec<_>>();
    let first = signatures.pop()?;
    let param_len = first.params.len();
    if signatures
        .iter()
        .any(|signature| signature.params.len() != param_len)
    {
        return None;
    }

    Some(signatures.into_iter().fold(first, join_function_signatures))
}

fn dedup_callable_targets(targets: Vec<CallableTarget>) -> Vec<CallableTarget> {
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

fn expected_call_signature(
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

fn signature_from_type(ty: &TypeRef) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Function(signature) => Some(signature.clone()),
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

fn caller_scope_dispatches_via_first_arg(hir: &FileHir, call: &rhai_hir::CallSite) -> bool {
    call.caller_scope
        && call
            .callee_reference
            .map(|reference| hir.reference(reference).name.as_str())
            == Some("call")
}

fn effective_call_argument_exprs<'a>(hir: &FileHir, call: &'a rhai_hir::CallSite) -> &'a [ExprId] {
    let offset =
        usize::from(caller_scope_dispatches_via_first_arg(hir, call)).min(call.arg_exprs.len());
    &call.arg_exprs[offset..]
}

fn effective_call_argument_types(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
) -> Vec<Option<TypeRef>> {
    let arg_offset =
        usize::from(caller_scope_dispatches_via_first_arg(hir, call)).min(call.arg_exprs.len());
    call_argument_types(hir, inference, &call.arg_exprs[arg_offset..])
}

fn effective_arg_types_for_call(
    hir: &FileHir,
    inference: &FileTypeInference,
    call: &rhai_hir::CallSite,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<Vec<Option<TypeRef>>> {
    arg_types
        .map(|arg_types| {
            let arg_offset = usize::from(caller_scope_dispatches_via_first_arg(hir, call));
            arg_types[arg_offset.min(arg_types.len())..].to_vec()
        })
        .or_else(|| Some(effective_call_argument_types(hir, inference, call)))
}

fn for_binding_types_from_iterable(ty: &TypeRef, binding_count: usize) -> Option<Vec<TypeRef>> {
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
        _ => None,
    }
}

fn loop_binding_types(item_ty: TypeRef, binding_count: usize, counter_ty: TypeRef) -> Vec<TypeRef> {
    let mut binding_types = vec![TypeRef::Unknown; binding_count];
    if let Some(first) = binding_types.first_mut() {
        *first = item_ty;
    }
    if binding_count > 1 {
        binding_types[1] = counter_ty;
    }
    binding_types
}

fn join_binding_type_sets(left: Vec<TypeRef>, right: Vec<TypeRef>) -> Vec<TypeRef> {
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
        TypeRef::Object(fields) => fields.get(field_name).cloned(),
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
                SymbolMutationKind::Path { segments }
                    if matches_top_level_field_segment(segments, field_name) =>
                {
                    Some(mutation.value)
                }
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

fn symbol_for_condition_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Paren => {
            largest_inner_expr(hir, expr).and_then(|inner| symbol_for_condition_expr(hir, inner))
        }
        _ => symbol_for_expr(hir, expr),
    }
}

fn expr_is_unit_like(hir: &FileHir, inference: &FileTypeInference, expr: ExprId) -> bool {
    match hir.expr(expr).kind {
        ExprKind::Paren => largest_inner_expr(hir, expr)
            .is_none_or(|inner| expr_is_unit_like(hir, inference, inner)),
        _ => inferred_expr_type(hir, inference, expr).is_some_and(|ty| matches!(ty, TypeRef::Unit)),
    }
}

fn string_literal_value(hir: &FileHir, expr: ExprId) -> Option<&str> {
    let literal = hir.literal(expr)?;
    (literal.kind == LiteralKind::String)
        .then_some(literal.text.as_deref())
        .flatten()
        .and_then(unquote_string_literal)
}

fn unquote_string_literal(text: &str) -> Option<&str> {
    (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
        .then_some(&text[1..text.len() - 1])
}

fn inferred_object_value_union(fields: &BTreeMap<String, TypeRef>) -> Option<TypeRef> {
    fields
        .values()
        .cloned()
        .reduce(|left, right| join_types(&left, &right))
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

fn imported_module_member_type_for_expr(
    hir: &FileHir,
    expr: ExprId,
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    let parts = qualified_path_parts(hir, expr)?;
    let (member_name, module_path) = parts.split_last()?;
    if module_path.is_empty() {
        return None;
    }
    imported_members
        .iter()
        .find(|member| member.module_path == module_path && member.name == *member_name)
        .map(|member| member.ty.clone())
}

fn qualified_path_parts(hir: &FileHir, expr: ExprId) -> Option<Vec<String>> {
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
            .collect()
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

fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) => items
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
        TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never | TypeRef::Union(_)
    )
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

fn can_refine_with_expected(current: &TypeRef, expected: &TypeRef) -> bool {
    if current == expected {
        return false;
    }

    match (current, expected) {
        (TypeRef::Unknown | TypeRef::Never, _) => true,
        (TypeRef::FnPtr, TypeRef::Function(_)) => true,
        (TypeRef::Array(inner), TypeRef::Array(_))
        | (TypeRef::Nullable(inner), TypeRef::Nullable(_)) => type_has_vague_parts(inner),
        (TypeRef::Object(fields), TypeRef::Object(_)) => fields.values().any(type_has_vague_parts),
        (TypeRef::Map(key, value), TypeRef::Map(_, _)) => {
            type_has_vague_parts(key) || type_has_vague_parts(value)
        }
        (TypeRef::Function(current), TypeRef::Function(expected))
            if current.params.len() == expected.params.len() =>
        {
            current.params.iter().any(type_has_vague_parts)
                || type_has_vague_parts(current.ret.as_ref())
        }
        _ => false,
    }
}

fn type_has_vague_parts(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Unknown | TypeRef::Never | TypeRef::FnPtr => true,
        TypeRef::Array(inner) | TypeRef::Nullable(inner) => type_has_vague_parts(inner),
        TypeRef::Object(fields) => fields.values().any(type_has_vague_parts),
        TypeRef::Map(key, value) => type_has_vague_parts(key) || type_has_vague_parts(value),
        TypeRef::Function(signature) => {
            signature.params.iter().any(type_has_vague_parts)
                || type_has_vague_parts(signature.ret.as_ref())
        }
        _ => false,
    }
}

pub(crate) fn join_types(left: &TypeRef, right: &TypeRef) -> TypeRef {
    if left == right {
        return left.clone();
    }

    match (left, right) {
        (TypeRef::Unknown, other) | (TypeRef::Never, other) => other.clone(),
        (other, TypeRef::Unknown) | (other, TypeRef::Never) => other.clone(),
        (TypeRef::FnPtr, TypeRef::Function(signature))
        | (TypeRef::Function(signature), TypeRef::FnPtr) => TypeRef::Function(signature.clone()),
        (TypeRef::Object(fields), other) | (other, TypeRef::Object(fields))
            if fields.is_empty()
                && matches!(
                    other,
                    TypeRef::Object(_) | TypeRef::Map(_, _) | TypeRef::Array(_)
                ) =>
        {
            other.clone()
        }
        (TypeRef::Array(left), TypeRef::Array(right)) => {
            TypeRef::Array(Box::new(join_types(left, right)))
        }
        (TypeRef::Array(items), TypeRef::Map(key, value))
        | (TypeRef::Map(key, value), TypeRef::Array(items))
            if matches!(
                key.as_ref(),
                TypeRef::Int | TypeRef::Unknown | TypeRef::Never
            ) =>
        {
            TypeRef::Array(Box::new(join_types(items, value)))
        }
        (TypeRef::Object(left_fields), TypeRef::Object(right_fields)) => {
            let mut merged = left_fields.clone();
            for (name, right_ty) in right_fields {
                let next = match merged.get(name.as_str()) {
                    Some(left_ty) => join_types(left_ty, right_ty),
                    None => right_ty.clone(),
                };
                merged.insert(name.clone(), next);
            }
            TypeRef::Object(merged)
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

fn nested_mutation_container_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    segments: &[MutationPathSegment],
    value: TypeRef,
) -> TypeRef {
    let mut current = value;
    for segment in segments.iter().rev() {
        current = match segment {
            MutationPathSegment::Field { name } => object_type_with_field(name, current),
            MutationPathSegment::Index { index } => {
                match inferred_expr_type(hir, inference, *index) {
                    Some(TypeRef::Int) => TypeRef::Array(Box::new(current)),
                    Some(index_ty) => TypeRef::Map(Box::new(index_ty), Box::new(current)),
                    None => TypeRef::Map(Box::new(TypeRef::Unknown), Box::new(current)),
                }
            }
        };
    }
    current
}

fn matches_top_level_field_segment(segments: &[MutationPathSegment], field_name: &str) -> bool {
    matches!(
        segments,
        [MutationPathSegment::Field { name }] if name == field_name
    )
}

fn object_type_with_field(name: &str, value: TypeRef) -> TypeRef {
    TypeRef::Object(BTreeMap::from([(name.to_owned(), value)]))
}
