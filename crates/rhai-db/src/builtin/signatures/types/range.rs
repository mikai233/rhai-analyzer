use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_range_types() -> Vec<HostType> {
    vec![builtin_range_type("range"), builtin_range_type("range=")]
}

fn builtin_range_type(name: &str) -> HostType {
    HostType {
        name: name.to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai range type.".to_owned()),
        methods: vec![
            builtin_method(
                "start",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the beginning of the range.".to_owned()),
            ),
            builtin_method(
                "end",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the end of the range.".to_owned()),
            ),
            builtin_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the range contains a number.".to_owned()),
            ),
            builtin_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the range contains no items.".to_owned()),
            ),
            builtin_method(
                "is_inclusive",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the range is inclusive.".to_owned()),
            ),
            builtin_method(
                "is_exclusive",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the range is exclusive.".to_owned()),
            ),
        ],
    }
}
