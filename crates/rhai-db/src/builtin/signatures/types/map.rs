use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::helpers::builtin_method;
use crate::types::HostType;

pub(crate) fn builtin_map_type() -> HostType {
    HostType {
        name: "map".to_owned(),
        generic_params: Vec::new(),
        docs: Some("Builtin Rhai object map type.".to_owned()),
        methods: vec![
            builtin_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Gets a copy of the value of a certain property.".to_owned()),
            ),
            builtin_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Sets a certain property to a new value.".to_owned()),
            ),
            builtin_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                Some("Returns the number of properties.".to_owned()),
            ),
            builtin_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Returns true if the object map is empty.".to_owned()),
            ),
            builtin_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                Some("Empties the object map.".to_owned()),
            ),
            builtin_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                Some("Removes a certain property and returns it.".to_owned()),
            ),
            builtin_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                Some("Checks whether the object map contains a property.".to_owned()),
            ),
            builtin_method(
                "keys",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                }],
                Some("Returns an array of all the property names.".to_owned()),
            ),
            builtin_method(
                "values",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                Some("Returns an array of all the property values.".to_owned()),
            ),
        ],
    }
}
