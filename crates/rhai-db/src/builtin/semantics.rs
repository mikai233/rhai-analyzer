use rhai_hir::{
    BinaryOperator, ExprId, ExprKind, ExternalSignatureIndex, FileHir, TypeRef, UnaryOperator,
};

use crate::infer::{
    ReadTargetKey, join_callable_target_signatures, largest_inner_expr,
    named_callable_targets_at_offset, read_target_key_for_expr, string_literal_value,
};
use crate::{FileTypeInference, HostFunction};

pub(crate) fn is_builtin_fn_call(hir: &FileHir, call: &rhai_hir::CallSite) -> bool {
    call.callee_reference
        .map(|reference_id| hir.reference(reference_id).name.as_str())
        == Some("Fn")
}

pub(crate) fn infer_fn_pointer_call_type(
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
        &[],
        None,
        &mut Vec::new(),
    );

    join_callable_target_signatures(&targets, None)
        .map(TypeRef::Function)
        .unwrap_or(TypeRef::FnPtr)
}

pub(crate) fn refine_type_from_type_of_condition_for_target(
    hir: &FileHir,
    current: &TypeRef,
    condition: ExprId,
    target: &ReadTargetKey,
    then_branch: bool,
) -> TypeRef {
    match hir.expr(condition).kind {
        ExprKind::Paren => largest_inner_expr(hir, condition)
            .map(|inner| {
                refine_type_from_type_of_condition_for_target(
                    hir,
                    current,
                    inner,
                    target,
                    then_branch,
                )
            })
            .unwrap_or_else(|| current.clone()),
        ExprKind::Unary => hir
            .unary_expr(condition)
            .filter(|unary| unary.operator == UnaryOperator::Not)
            .and_then(|unary| unary.operand)
            .map(|operand| {
                refine_type_from_type_of_condition_for_target(
                    hir,
                    current,
                    operand,
                    target,
                    !then_branch,
                )
            })
            .unwrap_or_else(|| current.clone()),
        ExprKind::Binary => {
            let Some(binary) = hir.binary_expr(condition) else {
                return current.clone();
            };

            match binary.operator {
                BinaryOperator::EqEq | BinaryOperator::NotEq => {
                    type_of_equality_refinement(hir, current, binary, target, then_branch)
                        .unwrap_or_else(|| current.clone())
                }
                BinaryOperator::AndAnd if then_branch => {
                    let current = binary
                        .lhs
                        .map(|lhs| {
                            refine_type_from_type_of_condition_for_target(
                                hir, current, lhs, target, true,
                            )
                        })
                        .unwrap_or_else(|| current.clone());

                    binary
                        .rhs
                        .map(|rhs| {
                            refine_type_from_type_of_condition_for_target(
                                hir, &current, rhs, target, true,
                            )
                        })
                        .unwrap_or(current)
                }
                BinaryOperator::OrOr if !then_branch => {
                    let current = binary
                        .lhs
                        .map(|lhs| {
                            refine_type_from_type_of_condition_for_target(
                                hir, current, lhs, target, false,
                            )
                        })
                        .unwrap_or_else(|| current.clone());

                    binary
                        .rhs
                        .map(|rhs| {
                            refine_type_from_type_of_condition_for_target(
                                hir, &current, rhs, target, false,
                            )
                        })
                        .unwrap_or(current)
                }
                _ => current.clone(),
            }
        }
        _ => current.clone(),
    }
}

pub(crate) fn refine_type_from_type_of_switch_for_target(
    hir: &FileHir,
    current: &TypeRef,
    switch_expr: &rhai_hir::SwitchExprInfo,
    target: &ReadTargetKey,
    offset: rhai_syntax::TextSize,
) -> TypeRef {
    let Some(scrutinee) = switch_expr.scrutinee else {
        return current.clone();
    };
    if type_of_guard_subject_target(hir, scrutinee).as_ref() != Some(target) {
        return current.clone();
    }

    let arms = hir.switch_arms(switch_expr.owner).collect::<Vec<_>>();
    let Some(active_index) = arms.iter().position(|arm| {
        arm.value
            .is_some_and(|value| hir.expr(value).range.contains(offset))
    }) else {
        return current.clone();
    };

    let mut excluded = Vec::new();
    for arm in &arms[..active_index] {
        if arm.wildcard {
            continue;
        }
        for guard in switch_arm_type_of_guards(hir, arm) {
            push_type_of_guard(&mut excluded, guard);
        }
    }

    let active_arm = arms[active_index];
    if active_arm.wildcard {
        return exclude_many_type_of_guards(current, &excluded).unwrap_or_else(|| current.clone());
    }

    let active_guards = switch_arm_type_of_guards(hir, active_arm);
    if active_guards.is_empty() {
        return current.clone();
    }

    retain_many_type_of_guards(current, &active_guards).unwrap_or_else(|| current.clone())
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum TypeOfGuard {
    Unit,
    Bool,
    Int,
    Float,
    Decimal,
    String,
    Char,
    Blob,
    Timestamp,
    Fn,
    Range,
    RangeInclusive,
    Array,
    Map,
    Named(String),
}

fn type_of_equality_refinement(
    hir: &FileHir,
    current: &TypeRef,
    binary: &rhai_hir::BinaryExprInfo,
    target: &ReadTargetKey,
    then_branch: bool,
) -> Option<TypeRef> {
    let lhs = binary.lhs?;
    let rhs = binary.rhs?;
    let guard = if type_of_guard_subject_target(hir, lhs).as_ref() == Some(target) {
        string_literal_value(hir, rhs).and_then(type_of_guard_from_name)
    } else if type_of_guard_subject_target(hir, rhs).as_ref() == Some(target) {
        string_literal_value(hir, lhs).and_then(type_of_guard_from_name)
    } else {
        None
    }?;

    let refined = match (binary.operator, then_branch) {
        (BinaryOperator::EqEq, true) | (BinaryOperator::NotEq, false) => {
            retain_type_of_guard(current, &guard)
        }
        (BinaryOperator::EqEq, false) | (BinaryOperator::NotEq, true) => {
            exclude_type_of_guard(current, &guard)
        }
        _ => None,
    };

    refined.filter(|refined| refined != current)
}

fn type_of_guard_subject_target(hir: &FileHir, expr: ExprId) -> Option<ReadTargetKey> {
    match hir.expr(expr).kind {
        ExprKind::Paren => {
            largest_inner_expr(hir, expr).and_then(|inner| type_of_guard_subject_target(hir, inner))
        }
        ExprKind::Call => {
            let call = hir
                .calls
                .iter()
                .find(|call| call.range == hir.expr(expr).range)?;

            if call.callee_reference.is_some_and(|reference_id| {
                hir.reference(reference_id).name == "type_of" && call.resolved_callee.is_none()
            }) {
                return call
                    .arg_exprs
                    .first()
                    .copied()
                    .and_then(|argument| read_target_key_for_expr(hir, argument));
            }

            if !call.arg_exprs.is_empty() {
                return None;
            }

            let callee_expr = call.callee_range.and_then(|range| hir.expr_at(range))?;
            let access = hir.member_access(callee_expr)?;
            (hir.reference(access.field_reference).name == "type_of")
                .then_some(access.receiver)
                .and_then(|receiver| read_target_key_for_expr(hir, receiver))
        }
        _ => None,
    }
}

fn type_of_guard_from_name(name: &str) -> Option<TypeOfGuard> {
    match name {
        "()" => Some(TypeOfGuard::Unit),
        "bool" => Some(TypeOfGuard::Bool),
        "i64" | "int" => Some(TypeOfGuard::Int),
        "f64" | "float" => Some(TypeOfGuard::Float),
        "decimal" => Some(TypeOfGuard::Decimal),
        "string" => Some(TypeOfGuard::String),
        "char" => Some(TypeOfGuard::Char),
        "blob" => Some(TypeOfGuard::Blob),
        "timestamp" => Some(TypeOfGuard::Timestamp),
        "Fn" | "FnPtr" => Some(TypeOfGuard::Fn),
        "range" => Some(TypeOfGuard::Range),
        "range=" => Some(TypeOfGuard::RangeInclusive),
        "array" => Some(TypeOfGuard::Array),
        "map" | "object" => Some(TypeOfGuard::Map),
        custom if !custom.is_empty() => Some(TypeOfGuard::Named(custom.to_owned())),
        _ => None,
    }
}

fn retain_type_of_guard(current: &TypeRef, guard: &TypeOfGuard) -> Option<TypeRef> {
    rebuild_refined_type(
        flattened_refinement_members(current)
            .into_iter()
            .filter(|member| member_matches_type_of_guard_for_retain(member, guard))
            .collect(),
    )
}

fn exclude_type_of_guard(current: &TypeRef, guard: &TypeOfGuard) -> Option<TypeRef> {
    rebuild_refined_type(
        flattened_refinement_members(current)
            .into_iter()
            .filter(|member| member_matches_type_of_guard_for_exclude(member, guard))
            .collect(),
    )
}

fn retain_many_type_of_guards(current: &TypeRef, guards: &[TypeOfGuard]) -> Option<TypeRef> {
    rebuild_refined_type(
        flattened_refinement_members(current)
            .into_iter()
            .filter(|member| {
                matches!(member, TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic)
                    || guards
                        .iter()
                        .any(|guard| member_matches_type_of_guard(member, guard))
            })
            .collect(),
    )
}

fn exclude_many_type_of_guards(current: &TypeRef, guards: &[TypeOfGuard]) -> Option<TypeRef> {
    rebuild_refined_type(
        flattened_refinement_members(current)
            .into_iter()
            .filter(|member| {
                matches!(member, TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic)
                    || guards
                        .iter()
                        .all(|guard| !member_matches_type_of_guard(member, guard))
            })
            .collect(),
    )
}

fn switch_arm_type_of_guards(hir: &FileHir, arm: &rhai_hir::SwitchArmInfo) -> Vec<TypeOfGuard> {
    let mut guards = Vec::new();
    for pattern in &arm.patterns {
        let Some(name) = string_literal_value(hir, *pattern) else {
            continue;
        };
        let Some(guard) = type_of_guard_from_name(name) else {
            continue;
        };
        push_type_of_guard(&mut guards, guard);
    }
    guards
}

fn push_type_of_guard(guards: &mut Vec<TypeOfGuard>, guard: TypeOfGuard) {
    if !guards.iter().any(|existing| existing == &guard) {
        guards.push(guard);
    }
}

fn flattened_refinement_members(ty: &TypeRef) -> Vec<TypeRef> {
    let mut members = Vec::new();
    collect_refinement_members(ty, &mut members);
    members
}

fn collect_refinement_members(ty: &TypeRef, members: &mut Vec<TypeRef>) {
    match ty {
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                collect_refinement_members(item, members);
            }
        }
        TypeRef::Nullable(inner) => {
            push_refinement_member(members, TypeRef::Unit);
            collect_refinement_members(inner, members);
        }
        _ => push_refinement_member(members, ty.clone()),
    }
}

fn push_refinement_member(members: &mut Vec<TypeRef>, member: TypeRef) {
    if !members.iter().any(|existing| existing == &member) {
        members.push(member);
    }
}

fn rebuild_refined_type(mut members: Vec<TypeRef>) -> Option<TypeRef> {
    match members.len() {
        0 => None,
        1 => members.pop(),
        _ => Some(TypeRef::Union(members)),
    }
}

fn member_matches_type_of_guard_for_retain(member: &TypeRef, guard: &TypeOfGuard) -> bool {
    matches!(member, TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic)
        || member_matches_type_of_guard(member, guard)
}

fn member_matches_type_of_guard_for_exclude(member: &TypeRef, guard: &TypeOfGuard) -> bool {
    matches!(member, TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic)
        || !member_matches_type_of_guard(member, guard)
}

fn member_matches_type_of_guard(member: &TypeRef, guard: &TypeOfGuard) -> bool {
    match guard {
        TypeOfGuard::Unit => matches!(member, TypeRef::Unit),
        TypeOfGuard::Bool => matches!(member, TypeRef::Bool),
        TypeOfGuard::Int => matches!(member, TypeRef::Int),
        TypeOfGuard::Float => matches!(member, TypeRef::Float),
        TypeOfGuard::Decimal => matches!(member, TypeRef::Decimal),
        TypeOfGuard::String => matches!(member, TypeRef::String),
        TypeOfGuard::Char => matches!(member, TypeRef::Char),
        TypeOfGuard::Blob => matches!(member, TypeRef::Blob),
        TypeOfGuard::Timestamp => matches!(member, TypeRef::Timestamp),
        TypeOfGuard::Fn => matches!(member, TypeRef::FnPtr | TypeRef::Function(_)),
        TypeOfGuard::Range => matches!(member, TypeRef::Range),
        TypeOfGuard::RangeInclusive => matches!(member, TypeRef::RangeInclusive),
        TypeOfGuard::Array => matches!(member, TypeRef::Array(_)),
        TypeOfGuard::Map => matches!(member, TypeRef::Map(_, _) | TypeRef::Object(_)),
        TypeOfGuard::Named(name) => match member {
            TypeRef::Named(member_name) => member_name == name,
            TypeRef::Applied {
                name: member_name, ..
            } => member_name == name,
            _ => false,
        },
    }
}
