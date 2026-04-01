use rhai_hir::FunctionTypeRef;

use crate::builtin::signatures::docs::{builtin_global_docs, builtin_method_docs};
use crate::types::{HostFunction, HostFunctionOverload};

pub(crate) fn builtin_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    docs: Option<String>,
) -> HostFunction {
    HostFunction {
        name: name.to_owned(),
        overloads: signatures
            .into_iter()
            .map(|signature| HostFunctionOverload {
                signature: Some(signature),
                docs: docs.clone(),
            })
            .collect(),
    }
}

pub(crate) fn builtin_documented_method(
    receiver_type: &str,
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
    reference_url: &str,
) -> HostFunction {
    let docs = builtin_method_docs(
        receiver_type,
        name,
        signatures.as_slice(),
        summary,
        examples,
        reference_url,
    );
    builtin_method(name, signatures, Some(docs))
}

pub(crate) fn builtin_global_function(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
    reference_url: &str,
) -> HostFunction {
    let docs = builtin_global_docs(
        name,
        signatures.as_slice(),
        summary,
        examples,
        reference_url,
    );
    HostFunction {
        name: name.to_owned(),
        overloads: signatures
            .into_iter()
            .map(|signature| HostFunctionOverload {
                signature: Some(signature),
                docs: Some(docs.clone()),
            })
            .collect(),
    }
}
