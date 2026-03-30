use rhai_hir::{FunctionTypeRef, TypeRef};

pub fn builtin_universal_method_signature(method_name: &str) -> Option<FunctionTypeRef> {
    match method_name {
        "type_of" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::String),
        }),
        "is_shared" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Bool),
        }),
        _ => None,
    }
}

pub(crate) fn builtin_universal_method_names() -> &'static [&'static str] {
    &["type_of", "is_shared"]
}
