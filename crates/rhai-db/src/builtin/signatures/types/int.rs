use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::{
    BuiltinCallableOverloadDoc, BuiltinParamDoc, builtin_type_docs,
};
use crate::builtin::signatures::helpers::{
    builtin_documented_method, builtin_documented_overloaded_method,
};
use crate::types::{HostFunction, HostType};

const NUMBER_REFERENCE_URL: &str = "https://rhai.rs/book/language/num-fn.html";

fn int_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "int",
        name,
        signatures,
        summary,
        examples,
        NUMBER_REFERENCE_URL,
    )
}

fn int_overloaded_method(
    name: &str,
    summary: &str,
    overloads: Vec<BuiltinCallableOverloadDoc<'_>>,
) -> HostFunction {
    builtin_documented_overloaded_method("int", name, summary, overloads, NUMBER_REFERENCE_URL)
}

pub(crate) fn builtin_int_type() -> HostType {
    HostType {
        name: "int".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "int",
            "Builtin Rhai integer type for whole-number arithmetic and numeric helpers.",
            &[
                "let value = 42;",
                "let as_float = value.to_float();",
                "// as_float == 42.0",
            ],
            NUMBER_REFERENCE_URL,
        )),
        methods: vec![
            int_method(
                "is_odd",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is an odd number.",
                &["let odd = 5.is_odd();", "// odd == true"],
            ),
            int_method(
                "is_even",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is an even number.",
                &["let even = 6.is_even();", "// even == true"],
            ),
            int_method(
                "abs",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the absolute value.",
                &["let value = (-42).abs();", "// value == 42"],
            ),
            int_method(
                "sign",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return `-1` for negative, `1` for positive, and `0` for zero.",
                &["let state = (-10).sign();", "// state == -1"],
            ),
            int_method(
                "is_zero",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the value is zero.",
                &["let done = 0.is_zero();", "// done == true"],
            ),
            int_method(
                "to_float",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Float),
                }],
                "Convert the integer into a floating-point number.",
                &["let ratio = 42.to_float();", "// ratio == 42.0"],
            ),
            int_method(
                "to_decimal",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Decimal),
                }],
                "Convert the integer into a fixed-precision decimal number.",
                &[
                    "let amount = 42.to_decimal();",
                    "// amount == 42 as a decimal value",
                ],
            ),
            int_method(
                "to_binary",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Convert the integer into a binary string representation.",
                &["let bits = 10.to_binary();", "// bits == \"1010\""],
            ),
            int_method(
                "to_octal",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Convert the integer into an octal string representation.",
                &["let text = 10.to_octal();", "// text == \"12\""],
            ),
            int_method(
                "to_hex",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Convert the integer into a hexadecimal string representation.",
                &[
                    "let text = 255.to_hex();",
                    "// text == \"ff\" or \"FF\" depending on engine formatting",
                ],
            ),
            int_method(
                "get_bit",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the bit at the specified zero-based position is set.",
                &[
                    "let is_set = 0b1010.get_bit(1);",
                    "// is_set == true",
                    "let missing = 0b1010.get_bit(0);",
                    "// missing == false",
                ],
            ),
            int_method(
                "set_bit",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Bool],
                    ret: Box::new(TypeRef::Int),
                }],
                "Return a new integer with the specified bit turned on or off.",
                &[
                    "let enabled = 0b1000.set_bit(1, true);",
                    "// enabled == 0b1010",
                    "let cleared = enabled.set_bit(3, false);",
                    "// cleared == 0b0010",
                ],
            ),
            int_overloaded_method(
                "get_bits",
                "Extract a range of bits and return them right-aligned as a new integer.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Extract a fixed number of bits starting at a zero-based bit offset.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Zero-based index of the first bit to read, starting from the least-significant bit.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bits to extract.",
                            },
                        ],
                        examples: &[
                            "let middle = 0b110110.get_bits(1, 3);",
                            "// middle == 0b011",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Extract bits selected by an exclusive bit range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Exclusive bit range to extract, starting from the least-significant bit.",
                        }],
                        examples: &[
                            "let middle = 0b110110.get_bits(1..4);",
                            "// middle == 0b011",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Extract bits selected by an inclusive bit range.",
                        params: &[BuiltinParamDoc {
                            name: "range",
                            description: "Inclusive bit range to extract, starting from the least-significant bit.",
                        }],
                        examples: &[
                            "let middle = 0b110110.get_bits(1..=3);",
                            "// middle == 0b011",
                        ],
                    },
                ],
            ),
            int_overloaded_method(
                "set_bits",
                "Return a new integer with a range of bits replaced by the supplied value.",
                vec![
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Int, TypeRef::Int, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Replace a fixed number of bits starting at a zero-based bit offset and return the updated integer.",
                        params: &[
                            BuiltinParamDoc {
                                name: "start",
                                description: "Zero-based index of the first bit to replace, starting from the least-significant bit.",
                            },
                            BuiltinParamDoc {
                                name: "len",
                                description: "Number of bits to replace.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "New bit pattern written into the selected range.",
                            },
                        ],
                        examples: &[
                            "let value = 0b110110.set_bits(1, 3, 0b000);",
                            "// value == 0b110000",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::Range, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Replace bits selected by an exclusive range and return the updated integer.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Exclusive bit range to replace, starting from the least-significant bit.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "New bit pattern written into the selected range.",
                            },
                        ],
                        examples: &[
                            "let value = 0b110110.set_bits(1..4, 0b000);",
                            "// value == 0b110000",
                        ],
                    },
                    BuiltinCallableOverloadDoc {
                        signature: FunctionTypeRef {
                            params: vec![TypeRef::RangeInclusive, TypeRef::Int],
                            ret: Box::new(TypeRef::Int),
                        },
                        summary: "Replace bits selected by an inclusive range and return the updated integer.",
                        params: &[
                            BuiltinParamDoc {
                                name: "range",
                                description: "Inclusive bit range to replace, starting from the least-significant bit.",
                            },
                            BuiltinParamDoc {
                                name: "value",
                                description: "New bit pattern written into the selected range.",
                            },
                        ],
                        examples: &[
                            "let value = 0b110110.set_bits(1..=3, 0b000);",
                            "// value == 0b110000",
                        ],
                    },
                ],
            ),
            int_method(
                "bits",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Bool))),
                }],
                "Return the integer as an array of booleans, from least-significant bit to most-significant bit.",
                &[
                    "let flags = 0b1010.bits();",
                    "// flags starts with [false, true, false, true]",
                ],
            ),
            int_method(
                "min",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the smaller of two integers.",
                &["let lower = 10.min(20);", "// lower == 10"],
            ),
            int_method(
                "max",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the larger of two integers.",
                &["let upper = 10.max(20);", "// upper == 20"],
            ),
        ],
    }
}
