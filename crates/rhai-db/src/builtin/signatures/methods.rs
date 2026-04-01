use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_method_docs;

pub fn builtin_universal_method_signature(method_name: &str) -> Option<FunctionTypeRef> {
    match method_name {
        "type_of" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::String),
        }),
        "tag" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Int),
        }),
        "is_shared" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Bool),
        }),
        _ => None,
    }
}

pub fn builtin_universal_method_docs(method_name: &str) -> Option<String> {
    match method_name {
        "type_of" => Some(builtin_method_docs(
            "any",
            "type_of",
            &[FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::String),
            }],
            "Return the dynamic Rhai type name of the current value.",
            &[
                "let kind = 42.type_of();",
                "// kind == \"i64\" or the engine's configured integer type name",
                "let user_kind = #{ name: \"Ada\" }.type_of();",
                "// user_kind describes the object's dynamic Rhai type",
            ],
            "https://rhai.rs/book/ref/type-of.html",
        )),
        "tag" => Some(builtin_method_docs(
            "any",
            "tag",
            &[FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Int),
            }],
            "Return the dynamic value tag attached to the current value. Tags default to zero and can be used to carry small amounts of debugging or provenance information alongside a value.",
            &[
                "let value = 42;",
                "let initial = value.tag();",
                "// initial == 0",
                "set_tag(value, 7);",
                "let tagged = value.tag();",
                "// tagged == 7",
            ],
            "https://rhai.rs/book/language/dynamic-tag.html",
        )),
        "is_shared" => Some(builtin_method_docs(
            "any",
            "is_shared",
            &[FunctionTypeRef {
                params: Vec::new(),
                ret: Box::new(TypeRef::Bool),
            }],
            "Return `true` when the current value is stored inside a shared wrapper. Most ordinary script values return `false`; this is mainly useful when interacting with shared values provided from Rust.",
            &[
                "let shared = value.is_shared();",
                "// shared == true or false",
                "if value.is_shared() { debug(\"shared value\"); }",
                "// enters the branch only for Rust-provided shared values",
                "let is_local_shared = 42.is_shared();",
                "// is_local_shared == false for ordinary script values",
            ],
            "https://rhai.rs/book/language/shared-values.html",
        )),
        _ => None,
    }
}

pub(crate) fn builtin_universal_method_names() -> &'static [&'static str] {
    &["type_of", "tag", "is_shared"]
}
