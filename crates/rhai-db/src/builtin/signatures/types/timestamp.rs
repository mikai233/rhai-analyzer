use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_timestamp_type() -> HostType {
    HostType {
        name: "timestamp".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai timestamp type.".to_owned()),
        methods: vec![builtin_method(
            "elapsed",
            vec![FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Float),
            }],
            Some("Returns the number of seconds since the timestamp.".to_owned()),
        )],
    }
}
