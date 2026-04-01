use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
use crate::types::{HostFunction, HostType};

const TIMESTAMP_REFERENCE_URL: &str = "https://rhai.rs/book/ref/timestamps.html";

fn timestamp_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "timestamp",
        name,
        signatures,
        summary,
        examples,
        TIMESTAMP_REFERENCE_URL,
    )
}

pub(crate) fn builtin_timestamp_type() -> HostType {
    HostType {
        name: "timestamp".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "timestamp",
            "Builtin Rhai timestamp type for time points and elapsed-time measurements.",
            &[
                "let started = timestamp();",
                "let seconds = started.elapsed();",
                "// seconds is the elapsed duration in seconds",
            ],
            TIMESTAMP_REFERENCE_URL,
        )),
        methods: vec![timestamp_method(
            "elapsed",
            vec![FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Float),
            }],
            "Return the number of seconds that have elapsed since the timestamp.",
            &[
                "let started = timestamp();",
                "let seconds = started.elapsed();",
                "// seconds is a floating-point duration in seconds",
            ],
        )],
    }
}
