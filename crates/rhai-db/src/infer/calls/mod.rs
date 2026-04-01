use rhai_hir::{FunctionTypeRef, SymbolId};

pub(crate) mod bindings;
pub(crate) mod selection;
pub(crate) mod signatures;
pub(crate) mod targets;

pub(crate) use crate::infer::calls::bindings::for_binding_types_from_iterable;
pub(crate) use crate::infer::calls::selection::{
    effective_call_argument_types, has_informative_arg_types, inferred_expr_type,
};
pub(crate) use crate::infer::calls::signatures::{
    expected_call_signature, join_callable_target_signatures, merge_function_candidate_signatures,
};
pub(crate) use crate::infer::calls::targets::{
    callable_targets_for_call, imported_method_signature_for_expr, named_callable_targets_at_offset,
};

#[derive(Clone)]
pub(crate) struct CallableTarget {
    pub(crate) signature: FunctionTypeRef,
    pub(crate) local_symbol: Option<SymbolId>,
}
