use crate::builtin::semantics::is_builtin_fn_call;
use crate::infer::ImportedMethodSignature;
use crate::infer::calls::{
    CallableTarget, callable_targets_for_call, effective_call_argument_types,
    has_informative_arg_types,
};
use crate::infer::generics::specialize_signature_with_arg_types;
use crate::infer::helpers::make_ambiguous_type;
use crate::{FileTypeInference, HostFunction, HostType, best_matching_signature_indexes};
use rhai_hir::{ExternalSignatureIndex, FileHir, FunctionTypeRef, TypeRef};

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
pub(crate) fn call_builtin_fn_signature(globals: &[HostFunction]) -> Option<&FunctionTypeRef> {
    globals
        .iter()
        .find(|function| function.name == "Fn")?
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .find(|signature| signature.params.len() == 1)
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
