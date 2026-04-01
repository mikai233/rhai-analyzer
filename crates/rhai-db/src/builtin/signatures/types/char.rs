use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
use crate::types::{HostFunction, HostType};

const CHAR_REFERENCE_URL: &str = "https://rhai.rs/book/ref/strings-chars.html";

fn char_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "char",
        name,
        signatures,
        summary,
        examples,
        CHAR_REFERENCE_URL,
    )
}

pub(crate) fn builtin_char_type() -> HostType {
    HostType {
        name: "char".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "char",
            "Builtin Rhai character type for Unicode scalar values and character transforms.",
            &[
                "let ch = 'x';",
                "let upper = ch.to_upper();",
                "// upper == 'X'",
            ],
            CHAR_REFERENCE_URL,
        )),
        methods: vec![
            char_method(
                "to_int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Convert the character into its Unicode code point.",
                &["let code = 'A'.to_int();", "// code == 65"],
            ),
            char_method(
                "to_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Char),
                }],
                "Return an upper-case copy of the character.",
                &["let upper = 'a'.to_upper();", "// upper == 'A'"],
            ),
            char_method(
                "to_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Char),
                }],
                "Return a lower-case copy of the character.",
                &["let lower = 'A'.to_lower();", "// lower == 'a'"],
            ),
            char_method(
                "make_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Convert the character to upper-case in place.",
                &["let ch = 'a';", "ch.make_upper();", "// ch == 'A'"],
            ),
            char_method(
                "make_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Convert the character to lower-case in place.",
                &["let ch = 'A';", "ch.make_lower();", "// ch == 'a'"],
            ),
        ],
    }
}
