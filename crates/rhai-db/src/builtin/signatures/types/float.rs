use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::{
    BuiltinCallableOverloadDoc, BuiltinParamDoc, builtin_type_docs,
};
use crate::builtin::signatures::helpers::{
    builtin_documented_method, builtin_documented_overloaded_method,
};
use crate::types::{HostFunction, HostType};

const NUMBER_REFERENCE_URL: &str = "https://rhai.rs/book/language/num-fn.html";

fn float_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "float",
        name,
        signatures,
        summary,
        examples,
        NUMBER_REFERENCE_URL,
    )
}

fn float_overloaded_method(
    name: &str,
    summary: &str,
    overloads: Vec<BuiltinCallableOverloadDoc<'_>>,
) -> HostFunction {
    builtin_documented_overloaded_method("float", name, summary, overloads, NUMBER_REFERENCE_URL)
}

pub(crate) fn builtin_float_type() -> HostType {
    HostType {
        name: "float".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "float",
            "Builtin Rhai floating-point type for fractional arithmetic and math helpers.",
            &[
                "let angle = 3.14;",
                "let sine = angle.sin();",
                "// sine is approximately 0.00159265",
            ],
            NUMBER_REFERENCE_URL,
        )),
        methods: vec![
            float_method(
                "abs",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the absolute value.",
                &["let value = (-3.5).abs();", "// value == 3.5"],
            ),
            float_method(
                "sign",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return `-1` for negative, `1` for positive, and `0` for zero.",
                &["let state = (-3.5).sign();", "// state == -1"],
            ),
            float_method(
                "is_zero",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is zero.",
                &["let zero = 0.0.is_zero();", "// zero == true"],
            ),
            float_method(
                "sin",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the sine of the value in radians.",
                &["let value = 0.0.sin();", "// value == 0.0"],
            ),
            float_method(
                "cos",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the cosine of the value in radians.",
                &["let value = 0.0.cos();", "// value == 1.0"],
            ),
            float_method(
                "tan",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the tangent of the value in radians.",
                &["let value = 0.0.tan();", "// value == 0.0"],
            ),
            float_method(
                "sinh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the hyperbolic sine of the value in radians.",
                &["let value = 0.0.sinh();", "// value == 0.0"],
            ),
            float_method(
                "cosh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the hyperbolic cosine of the value in radians.",
                &["let value = 0.0.cosh();", "// value == 1.0"],
            ),
            float_method(
                "tanh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the hyperbolic tangent of the value in radians.",
                &["let value = 0.0.tanh();", "// value == 0.0"],
            ),
            float_method(
                "hypot",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the length of the hypotenuse for this value and another side length.",
                &["let length = 3.0.hypot(4.0);", "// length == 5.0"],
            ),
            float_method(
                "asin",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the arc-sine in radians.",
                &["let value = 0.0.asin();", "// value == 0.0"],
            ),
            float_method(
                "acos",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the arc-cosine in radians.",
                &["let value = 1.0.acos();", "// value == 0.0"],
            ),
            float_overloaded_method(
                "atan",
                "Return the arc-tangent in radians, optionally using another coordinate as the second axis.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Return the arc-tangent of the current value, interpreted as a slope ratio.",
                        params: &[],
                        examples: &[
                            "let angle = 1.0.atan();",
                            "// angle is approximately 0.7853981634",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Return the polar angle using the current value and a second coordinate, similar to `atan2(y, x)`.",
                        params: &[BuiltinParamDoc {
                            name: "x",
                            description: "Second coordinate paired with the receiver value to compute the full polar angle.",
                        }],
                        examples: &[
                            "let angle = 1.0.atan(1.0);",
                            "// angle is approximately 0.7853981634",
                        ],
                    },
                ],
            ),
            float_method(
                "asinh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the inverse hyperbolic sine in radians.",
                &["let value = 0.0.asinh();", "// value == 0.0"],
            ),
            float_method(
                "acosh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the inverse hyperbolic cosine in radians.",
                &["let value = 1.0.acosh();", "// value == 0.0"],
            ),
            float_method(
                "atanh",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the inverse hyperbolic tangent in radians.",
                &["let value = 0.0.atanh();", "// value == 0.0"],
            ),
            float_method(
                "sqrt",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the square root.",
                &["let root = 9.0.sqrt();", "// root == 3.0"],
            ),
            float_method(
                "exp",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return `e` raised to the current value.",
                &[
                    "let value = 1.0.exp();",
                    "// value is approximately 2.7182818",
                ],
            ),
            float_method(
                "ln",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the natural logarithm.",
                &["let value = 1.0.ln();", "// value == 0.0"],
            ),
            float_overloaded_method(
                "log",
                "Return the logarithm, using base 10 by default or a custom base when provided.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: Vec::new(),
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Return the base-10 logarithm of the current value.",
                        params: &[],
                        examples: &["let value = 100.0.log();", "// value == 2.0"],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                            ret: Box::new(TypeRef::Float),
                        },
                        summary: "Return the logarithm of the current value using a custom base.",
                        params: &[BuiltinParamDoc {
                            name: "base",
                            description: "Base used to evaluate the logarithm.",
                        }],
                        examples: &["let binary = 8.0.log(2.0);", "// binary == 3.0"],
                    },
                ],
            ),
            float_method(
                "floor",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Round down to the nearest integer value.",
                &["let value = 3.8.floor();", "// value == 3.0"],
            ),
            float_method(
                "ceiling",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Round up to the nearest integer value.",
                &["let value = 3.2.ceiling();", "// value == 4.0"],
            ),
            float_method(
                "round",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Round to the nearest integer value.",
                &["let value = 3.5.round();", "// value == 4.0"],
            ),
            float_method(
                "round_half_up",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Float),
                }],
                "Round to a fixed number of decimal places, with halves rounded away from zero.",
                &["let value = 3.145.round_half_up(2);", "// value == 3.15"],
            ),
            float_method(
                "round_half_down",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Float),
                }],
                "Round to a fixed number of decimal places, with halves rounded toward zero.",
                &["let value = 3.145.round_half_down(2);", "// value == 3.14"],
            ),
            float_method(
                "int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the integral part.",
                &["let integral = 3.5.int();", "// integral == 3.0"],
            ),
            float_method(
                "fraction",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the fractional part.",
                &["let frac = 3.5.fraction();", "// frac == 0.5"],
            ),
            float_method(
                "to_int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Convert the floating-point number into an integer.",
                &["let count = 3.5.to_int();", "// count == 3"],
            ),
            float_method(
                "to_float",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the floating-point number unchanged.",
                &["let same = 3.5.to_float();", "// same == 3.5"],
            ),
            float_method(
                "to_decimal",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Decimal),
                }],
                "Convert the floating-point number into a fixed-precision decimal number.",
                &[
                    "let amount = 3.5.to_decimal();",
                    "// amount == 3.5 as a decimal value",
                ],
            ),
            float_method(
                "to_degrees",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Convert an angle from radians to degrees.",
                &[
                    "let degrees = 3.1415926535.to_degrees();",
                    "// degrees is approximately 180.0",
                ],
            ),
            float_method(
                "to_radians",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Convert an angle from degrees to radians.",
                &[
                    "let radians = 180.0.to_radians();",
                    "// radians is approximately 3.1415926535",
                ],
            ),
            float_method(
                "is_nan",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is NaN.",
                &["let invalid = (0.0 / 0.0).is_nan();", "// invalid == true"],
            ),
            float_method(
                "is_finite",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is finite.",
                &["let finite = 1.5.is_finite();", "// finite == true"],
            ),
            float_method(
                "is_infinite",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is infinite.",
                &[
                    "let infinite = (1.0 / 0.0).is_infinite();",
                    "// infinite == true",
                ],
            ),
            float_method(
                "min",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the smaller of two numbers.",
                &["let lower = 3.5.min(5);", "// lower == 3.5"],
            ),
            float_method(
                "max",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Union(vec![TypeRef::Int, TypeRef::Float])],
                    ret: Box::new(TypeRef::Float),
                }],
                "Return the larger of two numbers.",
                &["let upper = 3.5.max(5);", "// upper == 5.0"],
            ),
        ],
    }
}
