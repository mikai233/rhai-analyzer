use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_blob_type() -> HostType {
    HostType {
        name: "blob".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai blob type.".to_owned()),
        methods: vec![
            builtin_method(
                "push",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Appends a byte to the end of the BLOB.".to_owned()),
            ),
            builtin_method(
                "append",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Blob],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Concatenates another BLOB to the end.".to_owned()),
            ),
            builtin_method(
                "insert",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Inserts a byte at a certain position.".to_owned()),
            ),
            builtin_method(
                "pop",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Removes the last byte and returns it.".to_owned()),
            ),
            builtin_method(
                "shift",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Removes the first byte and returns it.".to_owned()),
            ),
            builtin_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Removes a byte at a particular position.".to_owned()),
            ),
            builtin_method(
                "reverse",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Reverses the BLOB byte by byte.".to_owned()),
            ),
            builtin_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the number of bytes in the BLOB.".to_owned()),
            ),
            builtin_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the BLOB is empty.".to_owned()),
            ),
            builtin_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Empties the BLOB.".to_owned()),
            ),
            builtin_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Cuts off the BLOB at the specified length.".to_owned()),
            ),
            builtin_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the BLOB contains a byte value.".to_owned()),
            ),
            builtin_method(
                "to_array",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Int))),
                }],
                Some("Converts the BLOB into an array of integers.".to_owned()),
            ),
        ],
    }
}
