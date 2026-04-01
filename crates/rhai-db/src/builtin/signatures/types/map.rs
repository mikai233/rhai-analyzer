use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::builtin::signatures::docs::builtin_type_docs;
use crate::builtin::signatures::helpers::builtin_documented_method;
use crate::types::{HostFunction, HostType};

const MAP_REFERENCE_URL: &str = "https://rhai.rs/book/ref/object-maps.html";

fn map_method(
    name: &str,
    signatures: Vec<FunctionTypeRef>,
    summary: &str,
    examples: &[&str],
) -> HostFunction {
    builtin_documented_method(
        "map",
        name,
        signatures,
        summary,
        examples,
        MAP_REFERENCE_URL,
    )
}

pub(crate) fn builtin_map_type() -> HostType {
    HostType {
        name: "map".to_owned(),
        generic_params: Vec::new(),
        docs: Some(builtin_type_docs(
            "map",
            "Builtin Rhai object map type for property-based records and ad-hoc objects.",
            &[
                "let user = #{ name: \"Ada\", active: true };",
                "let keys = user.keys();",
                "// keys == [\"name\", \"active\"]",
            ],
            MAP_REFERENCE_URL,
        )),
        methods: vec![
            map_method(
                "get",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Get a copy of the value stored under a property name.",
                &[
                    "let user = #{ name: \"Ada\", active: true };",
                    "let name = user.get(\"name\");",
                    "// name == \"Ada\"",
                ],
            ),
            map_method(
                "set",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String, TypeRef::Any],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Set a property to a new value.",
                &[
                    "let user = #{};",
                    "user.set(\"name\", \"Ada\");",
                    "// user == #{ name: \"Ada\" }",
                ],
            ),
            map_method(
                "len",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Int),
                }],
                "Return the number of properties in the map.",
                &[
                    "let count = #{ name: \"Ada\", active: true }.len();",
                    "// count == 2",
                ],
            ),
            map_method(
                "is_empty",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Bool),
                }],
                "Return `true` if the object map is empty.",
                &["let empty = #{}.is_empty();", "// empty == true"],
            ),
            map_method(
                "clear",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Unit),
                }],
                "Clear all properties from the map.",
                &[
                    "let user = #{ name: \"Ada\" };",
                    "user.clear();",
                    "// user == #{}",
                ],
            ),
            map_method(
                "remove",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Union(vec![TypeRef::Any, TypeRef::Unit])),
                }],
                "Remove a property and return its previous value.",
                &[
                    "let user = #{ name: \"Ada\", active: true };",
                    "let old_name = user.remove(\"name\");",
                    "// old_name == \"Ada\"",
                    "// user == #{ active: true }",
                ],
            ),
            map_method(
                "contains",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }],
                "Check whether the map contains a property.",
                &[
                    "let found = #{ name: \"Ada\" }.contains(\"name\");",
                    "// found == true",
                ],
            ),
            map_method(
                "keys",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::String))),
                }],
                "Return all property names in the map.",
                &[
                    "let fields = #{ name: \"Ada\", active: true }.keys();",
                    "// fields == [\"name\", \"active\"]",
                ],
            ),
            map_method(
                "values",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Array(Box::new(TypeRef::Any))),
                }],
                "Return all property values in the map.",
                &[
                    "let values = #{ name: \"Ada\", active: true }.values();",
                    "// values contains \"Ada\" and true",
                ],
            ),
            map_method(
                "mixin",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Map(
                        Box::new(TypeRef::String),
                        Box::new(TypeRef::Any),
                    )],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Merge properties from another object map into this one.",
                &[
                    "let user = #{ name: \"Ada\" };",
                    "user.mixin(#{ active: true });",
                    "// user == #{ name: \"Ada\", active: true }",
                ],
            ),
            map_method(
                "fill_with",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::Map(
                        Box::new(TypeRef::String),
                        Box::new(TypeRef::Any),
                    )],
                    ret: Box::new(TypeRef::Unit),
                }],
                "Fill missing properties from another object map without overwriting existing ones.",
                &[
                    "let user = #{ name: \"Ada\" };",
                    "user.fill_with(#{ name: \"Grace\", active: true });",
                    "// user == #{ name: \"Ada\", active: true }",
                ],
            ),
            map_method(
                "drain",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Map(
                        Box::new(TypeRef::String),
                        Box::new(TypeRef::Any),
                    )),
                }],
                "Remove all properties accepted by a predicate callback and return them as a new object map.",
                &[
                    "fn keep_name(key, value) { key == \"name\" }",
                    "let user = #{ name: \"Ada\", active: true };",
                    "let removed = user.drain(Fn(\"keep_name\"));",
                    "// removed == #{ name: \"Ada\" }",
                    "// user == #{ active: true }",
                ],
            ),
            map_method(
                "retain",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Map(
                        Box::new(TypeRef::String),
                        Box::new(TypeRef::Any),
                    )),
                }],
                "Retain properties accepted by a predicate callback and return the removed properties as a new object map.",
                &[
                    "fn keep_name(key, value) { key == \"name\" }",
                    "let user = #{ name: \"Ada\", active: true };",
                    "let removed = user.retain(Fn(\"keep_name\"));",
                    "// removed == #{ active: true }",
                    "// user == #{ name: \"Ada\" }",
                ],
            ),
            map_method(
                "filter",
                vec![FunctionTypeRef {
                    params: vec![TypeRef::FnPtr],
                    ret: Box::new(TypeRef::Map(
                        Box::new(TypeRef::String),
                        Box::new(TypeRef::Any),
                    )),
                }],
                "Construct a new object map containing only the properties accepted by a predicate callback.",
                &[
                    "fn keep_name(key, value) { key == \"name\" }",
                    "let user = #{ name: \"Ada\", active: true };",
                    "let filtered = user.filter(Fn(\"keep_name\"));",
                    "// filtered == #{ name: \"Ada\" }",
                    "// user is unchanged",
                ],
            ),
            map_method(
                "to_json",
                vec![FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::String),
                }],
                "Serialize the object map into a JSON string.",
                &[
                    "let json = #{ name: \"Ada\", active: true }.to_json();",
                    "// json == '{\"name\":\"Ada\",\"active\":true}' or equivalent JSON text",
                ],
            ),
        ],
    }
}
