use rhai_hir::{FunctionTypeRef, TypeRef};

pub(crate) fn builtin_universal_method_signature(method_name: &str) -> Option<FunctionTypeRef> {
    match method_name {
        "type_of" => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::String),
        }),
        _ => None,
    }
}
