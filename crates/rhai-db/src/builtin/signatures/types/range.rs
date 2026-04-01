use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
use crate::types::{HostFunction, HostType};

const RANGE_REFERENCE_URL: &str = "https://rhai.rs/book/language/loops.html";

fn range_method(
    receiver_type: &str,
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        receiver_type,
        name,
        signatures,
        summary,
        examples,
        RANGE_REFERENCE_URL,
    )
}

pub(crate) fn builtin_range_types() -> Vec<HostType> {
    vec![builtin_range_type("range"), builtin_range_type("range=")]
}

fn builtin_range_type(name: &str) -> HostType {
    HostType {
        name: name.to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            name,
            "Builtin Rhai range type for integer intervals and loop-friendly numeric spans.",
            &[
                "let span = 0..10;",
                "let inside = span.contains(3);",
                "// inside == true",
            ],
            RANGE_REFERENCE_URL,
        )),
        methods: vec![
            range_method(
                name,
                "start",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the beginning of the range.",
                &["let first = (1..10).start();", "// first == 1"],
            ),
            range_method(
                name,
                "end",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the end of the range.",
                &["let last = (1..10).end();", "// last == 10"],
            ),
            range_method(
                name,
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the range contains a number.",
                &["let inside = (1..10).contains(5);", "// inside == true"],
            ),
            range_method(
                name,
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the range contains no items.",
                &["let empty = (1..1).is_empty();", "// empty == true"],
            ),
            range_method(
                name,
                "is_inclusive",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the range is inclusive.",
                &["let flag = (1..=10).is_inclusive();", "// flag == true"],
            ),
            range_method(
                name,
                "is_exclusive",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the range is exclusive.",
                &["let flag = (1..10).is_exclusive();", "// flag == true"],
            ),
        ],
    }
}
