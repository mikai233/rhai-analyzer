use rhai_hir::FunctionTypeRef;

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
