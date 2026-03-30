use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_float_type() -> HostType {
    HostType {
        name: "float".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai floating-point type.".to_owned()),
        methods: vec![
            builtin_method(
                "abs",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
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
                "sin",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the sine in radians.".to_owned()),
            ),
            builtin_method(
                "cos",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the cosine in radians.".to_owned()),
            ),
            builtin_method(
                "tan",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the tangent in radians.".to_owned()),
            ),
            builtin_method(
                "sqrt",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the square root.".to_owned()),
            ),
            builtin_method(
                "exp",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns e raised to the number.".to_owned()),
            ),
            builtin_method(
                "ln",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the natural logarithm.".to_owned()),
            ),
            builtin_method(
                "log",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the base-10 logarithm.".to_owned()),
            ),
            builtin_method(
                "floor",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Rounds down to the nearest integer value.".to_owned()),
            ),
            builtin_method(
                "ceiling",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Rounds up to the nearest integer value.".to_owned()),
            ),
            builtin_method(
                "round",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Rounds to the nearest integer value.".to_owned()),
            ),
            builtin_method(
                "int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the integral part.".to_owned()),
            ),
            builtin_method(
                "fraction",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the fractional part.".to_owned()),
            ),
            builtin_method(
                "to_int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Converts the floating-point number into an integer.".to_owned()),
            ),
            builtin_method(
                "to_float",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the floating-point number itself.".to_owned()),
            ),
            builtin_method(
                "to_degrees",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Converts the angle from radians to degrees.".to_owned()),
            ),
            builtin_method(
                "to_radians",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Converts the angle from degrees to radians.".to_owned()),
            ),
            builtin_method(
                "is_nan",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is NaN.".to_owned()),
            ),
            builtin_method(
                "is_finite",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is finite.".to_owned()),
            ),
            builtin_method(
                "is_infinite",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the value is infinite.".to_owned()),
            ),
            builtin_method(
                "min",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the smaller of two numbers.".to_owned()),
            ),
            builtin_method(
                "max",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                    ret: Box::new(TypeRef::Float),
                }],
                Some("Returns the larger of two numbers.".to_owned()),
            ),
        ],
    }
}
