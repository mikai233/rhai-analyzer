use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_string_type() -> HostType {
    HostType {
        name: "string".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai string type.".to_owned()),
        methods: vec![
            builtin_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the number of characters in the string.".to_owned()),
            ),
            builtin_method(
                "bytes",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the number of UTF-8 bytes in the string.".to_owned()),
            ),
            builtin_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the string is empty.".to_owned()),
            ),
            builtin_method(
                "to_blob",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Blob),
                }],
                Some("Converts the string into a UTF-8 encoded BLOB.".to_owned()),
            ),
            builtin_method(
                "to_chars",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                }],
                Some("Splits the string into individual characters.".to_owned()),
            ),
            builtin_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Char, TypeRef::Unit])),
                }],
                Some("Gets the character at the specified position.".to_owned()),
            ),
            builtin_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Char],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Sets the character at the specified position.".to_owned()),
            ),
            builtin_method(
                "pad",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                Some("Pads the string to the target length.".to_owned()),
            ),
            builtin_method(
                "append",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Appends an item to the string.".to_owned()),
            ),
            builtin_method(
                "remove",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                Some("Removes a character or substring from the string.".to_owned()),
            ),
            builtin_method(
                "pop",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Union(vec![TypeRef::Char, TypeRef::Unit])),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                ],
                Some("Removes characters from the end of the string.".to_owned()),
            ),
            builtin_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Empties the string.".to_owned()),
            ),
            builtin_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Cuts the string to the specified length.".to_owned()),
            ),
            builtin_method(
                "to_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                Some("Returns an upper-case copy of the string.".to_owned()),
            ),
            builtin_method(
                "to_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                Some("Returns a lower-case copy of the string.".to_owned()),
            ),
            builtin_method(
                "make_upper",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Converts the string to upper-case in place.".to_owned()),
            ),
            builtin_method(
                "make_lower",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Converts the string to lower-case in place.".to_owned()),
            ),
            builtin_method(
                "trim",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Trims leading and trailing whitespace.".to_owned()),
            ),
            builtin_method(
                "contains",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Bool),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Bool),
                    },
                ],
                Some("Checks whether the string contains a character or substring.".to_owned()),
            ),
            builtin_method(
                "starts_with",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the string starts with a prefix.".to_owned()),
            ),
            builtin_method(
                "ends_with",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the string ends with a suffix.".to_owned()),
            ),
            builtin_method(
                "index_of",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                ],
                Some("Finds the position of a character or substring.".to_owned()),
            ),
            builtin_method(
                "sub_string",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::String),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::String),
                    },
                ],
                Some("Extracts a substring.".to_owned()),
            ),
            builtin_method(
                "split",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                ],
                Some("Splits the string into segments.".to_owned()),
            ),
            builtin_method(
                "split_rev",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                    },
                ],
                Some("Splits the string in reverse order.".to_owned()),
            ),
            builtin_method(
                "crop",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Range],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::RangeInclusive],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                Some("Retains only a portion of the string.".to_owned()),
            ),
            builtin_method(
                "replace",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Char, TypeRef::Char],
                        ret: Box::new(TypeRef::Unit),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::String],
                        ret: Box::new(TypeRef::Unit),
                    },
                ],
                Some("Replaces a character or substring.".to_owned()),
            ),
            builtin_method(
                "chars",
                vec![
                    FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(TypeRef::Array(Box::new(TypeRef::Char))),
                    },
                ],
                Some("Iterates over the characters of the string.".to_owned()),
            ),
        ],
    }
}
