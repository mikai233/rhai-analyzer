use crate::FileTypeInference;
use crate::infer::calls::inferred_expr_type;
use crate::infer::helpers::{join_option_types, join_types};
use crate::infer::objects::direct_loop_body;
use rhai_hir::{BodyId, ControlFlowKind, ExprId, FileHir, TypeRef};

pub(crate) fn function_like_body_result_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    body: BodyId,
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

pub(crate) fn block_expr_result_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    body: BodyId,
) -> Option<TypeRef> {
    hir.body_may_fall_through(body)
        .then(|| hir.body_tail_value(body))
        .flatten()
        .and_then(|expr| inferred_expr_type(hir, inference, expr))
}

pub(crate) fn infer_loop_like_expr_type(
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

pub(crate) fn join_expr_types(
    hir: &FileHir,
    inference: &FileTypeInference,
    left: Option<ExprId>,
    right: Option<ExprId>,
) -> Option<TypeRef> {
    let left = left.and_then(|expr| inferred_expr_type(hir, inference, expr));
    let right = right.and_then(|expr| inferred_expr_type(hir, inference, expr));
    join_option_types(left.as_ref(), right.as_ref())
}
