use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_char_type() -> HostType {
    HostType {
        name: "char".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai character type.".to_owned()),
        methods: vec![
            builtin_method(
                "to_int",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Converts the character into its Unicode code point.".to_owned()),
            ),
            builtin_method(
                "to_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Char),
                }],
                Some("Returns an upper-case copy of the character.".to_owned()),
            ),
            builtin_method(
                "to_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Char),
                }],
                Some("Returns a lower-case copy of the character.".to_owned()),
            ),
            builtin_method(
                "make_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Converts the character to upper-case.".to_owned()),
            ),
            builtin_method(
                "make_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Converts the character to lower-case.".to_owned()),
            ),
        ],
    }
}
