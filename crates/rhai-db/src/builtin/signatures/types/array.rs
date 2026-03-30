use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_array_type() -> HostType {
    HostType {
        name: "array".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai array type.".to_owned()),
        methods: vec![
            builtin_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Gets a copy of the element at a certain position.".to_owned()),
            ),
            builtin_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Sets a certain position to a new value.".to_owned()),
            ),
            builtin_method(
                "push",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Appends an element to the end of the array.".to_owned()),
            ),
            builtin_method(
                "append",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Array(Box::new(TypeRef::Any))],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Concatenates another array to the end.".to_owned()),
            ),
            builtin_method(
                "insert",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Inserts an element at a certain position.".to_owned()),
            ),
            builtin_method(
                "pop",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Removes the last element and returns it.".to_owned()),
            ),
            builtin_method(
                "shift",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Removes the first element and returns it.".to_owned()),
            ),
            builtin_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Removes an element at a particular position.".to_owned()),
            ),
            builtin_method(
                "reverse",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Reverses the array.".to_owned()),
            ),
            builtin_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the number of elements.".to_owned()),
            ),
            builtin_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the array is empty.".to_owned()),
            ),
            builtin_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Empties the array.".to_owned()),
            ),
            builtin_method(
                "truncate",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Int],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Cuts off the array at the specified length.".to_owned()),
            ),
            builtin_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the array contains a value.".to_owned()),
            ),
            builtin_method(
                "index_of",
                vec![
                    FunctionTypeRef {
                        params: vec![TypeRef::Any],
                        ret: Box::new(TypeRef::Int),
                    },
                    FunctionTypeRef {
                        params: vec![TypeRef::Any, TypeRef::Int],
                        ret: Box::new(TypeRef::Int),
                    },
                ],
                Some("Finds the position of a value in the array.".to_owned()),
            ),
        ],
    }
}
