use crate::FileTypeInference;
use crate::best_matching_signature_indexes;
use crate::infer::calls::CallableTarget;
use rhai_hir::{ExprId, FileHir, TypeRef};

pub(crate) fn select_best_callable_targets(
    targets: Vec<CallableTarget>,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Vec<CallableTarget> {
    let Some(arg_types) = arg_types else {
        return targets;
    };

    let arity_matched = targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.signature.params.len() == arg_types.len())
        .collect::<Vec<_>>();
    if arity_matched.is_empty() {
        return Vec::new();
    }

    if has_informative_arg_types(arg_types) {
        let indexes = best_matching_signature_indexes(
            arity_matched.iter().map(|(_, target)| &target.signature),
            arg_types,
        );
        if !indexes.is_empty() {
            return indexes
                .into_iter()
                .filter_map(|index| {
                    arity_matched
                        .get(index)
                        .map(|(target_index, _)| *target_index)
                })
                .filter_map(|target_index| targets.get(target_index).cloned())
                .collect();
        }
    }

    arity_matched
        .into_iter()
        .filter_map(|(index, _)| targets.get(index).cloned())
        .collect()
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
pub(crate) fn has_informative_arg_types(arg_types: &[Option<TypeRef>]) -> bool {
    arg_types.iter().flatten().any(|ty| {
        !matches!(
            ty,
            TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never
        )
    })
}
