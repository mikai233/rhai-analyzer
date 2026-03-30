mod array;
mod blob;
mod char;
mod float;
mod int;
mod map;
mod range;
mod string;
mod timestamp;

use rhai_hir::TypeRef;

use crate::types::HostType;

pub(crate) use crate::builtin::signatures::types::array::builtin_array_type;
pub(crate) use crate::builtin::signatures::types::blob::builtin_blob_type;
pub(crate) use crate::builtin::signatures::types::char::builtin_char_type;
pub(crate) use crate::builtin::signatures::types::float::builtin_float_type;
pub(crate) use crate::builtin::signatures::types::int::builtin_int_type;
pub(crate) use crate::builtin::signatures::types::map::builtin_map_type;
pub(crate) use crate::builtin::signatures::types::range::builtin_range_types;
pub(crate) use crate::builtin::signatures::types::string::builtin_string_type;
pub(crate) use crate::builtin::signatures::types::timestamp::builtin_timestamp_type;

pub(crate) fn builtin_host_types() -> Vec<HostType> {
    let mut types = vec![
        builtin_int_type(),
        builtin_float_type(),
        builtin_char_type(),
        builtin_string_type(),
        builtin_array_type(),
        builtin_map_type(),
        builtin_blob_type(),
        builtin_timestamp_type(),
    ];
    types.extend(builtin_range_types());
    types
}

pub(crate) fn host_type_name_for_type(ty: &TypeRef) -> Option<&'static str> {
    match ty {
        TypeRef::Int => Some("int"),
        TypeRef::Float => Some("float"),
        TypeRef::Char => Some("char"),
        TypeRef::String => Some("string"),
        TypeRef::Array(_) => Some("array"),
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some("map"),
        TypeRef::Blob => Some("blob"),
        TypeRef::Timestamp => Some("timestamp"),
        TypeRef::Range => Some("range"),
        TypeRef::RangeInclusive => Some("range="),
        _ => None,
    }
}
