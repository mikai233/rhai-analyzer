use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_int_type() -> HostType {
    HostType {
        name: "int".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai integer type.".to_owned()),
        methods: vec![
            builtin_method(
                "is_odd",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is an odd number.".to_owned()),
            ),
            builtin_method(
                "is_even",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is an even number.".to_owned()),
            ),
            builtin_method(
                "abs",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the absolute value.".to_owned()),
            ),
            builtin_method(
                "sign",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns -1 for negative, +1 for positive, and 0 for zero.".to_owned()),
            ),
            builtin_method(
                "is_zero",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is zero.".to_owned()),
            ),
            builtin_method(
                "to_float",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Converts the integer into a floating-point number.".to_owned()),
            ),
            builtin_method(
                "min",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the smaller of two integers.".to_owned()),
            ),
            builtin_method(
                "max",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the larger of two integers.".to_owned()),
            ),
        ],
    }
}
