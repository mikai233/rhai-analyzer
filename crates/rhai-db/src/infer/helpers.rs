use std::collections::BTreeMap;

use crate::FileTypeInference;
use crate::infer::calls::inferred_expr_type;
use rhai_hir::{FileHir, FunctionTypeRef, MutationPathSegment, TypeRef};

pub(crate) fn infer_additive_result(
    lhs: Option<&TypeRef>,
    rhs: Option<&TypeRef>,
) -> Option<TypeRef> {
    match (lhs?, rhs?) {
        (TypeRef::String, TypeRef::String) => Some(TypeRef::String),
        (left, right) => infer_numeric_result(Some(left), Some(right)),
    }
}

pub(crate) fn infer_numeric_result(
    lhs: Option<&TypeRef>,
    rhs: Option<&TypeRef>,
) -> Option<TypeRef> {
    let lhs = lhs?;
    let rhs = rhs?;
    if lhs == rhs && matches!(lhs, TypeRef::Int | TypeRef::Float | TypeRef::Decimal) {
        return Some(lhs.clone());
    }

    match (lhs, rhs) {
        (TypeRef::Decimal, TypeRef::Int | TypeRef::Float)
        | (TypeRef::Int | TypeRef::Float, TypeRef::Decimal) => Some(TypeRef::Decimal),
        (TypeRef::Float, TypeRef::Int) | (TypeRef::Int, TypeRef::Float) => Some(TypeRef::Float),
        _ => None,
    }
}

pub(crate) fn join_option_types(lhs: Option<&TypeRef>, rhs: Option<&TypeRef>) -> Option<TypeRef> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => Some(join_types(lhs, rhs)),
        (Some(lhs), None) => Some(lhs.clone()),
        (None, Some(rhs)) => Some(rhs.clone()),
        (None, None) => None,
    }
}

pub(crate) fn can_refine_with_expected(current: &TypeRef, expected: &TypeRef) -> bool {
    if current == expected {
        return false;
    }

    match (current, expected) {
        (TypeRef::Unknown | TypeRef::Never, _) => true,
        (TypeRef::FnPtr, TypeRef::Function(_)) => true,
        (TypeRef::Array(inner), TypeRef::Array(_))
        | (TypeRef::Nullable(inner), TypeRef::Nullable(_)) => type_has_vague_parts(inner),
        (TypeRef::Object(fields), TypeRef::Object(_)) => fields.values().any(type_has_vague_parts),
        (TypeRef::Map(key, value), TypeRef::Map(_, _)) => {
            type_has_vague_parts(key) || type_has_vague_parts(value)
        }
        (TypeRef::Function(current), TypeRef::Function(expected))
            if current.params.len() == expected.params.len() =>
        {
            current.params.iter().any(type_has_vague_parts)
                || type_has_vague_parts(current.ret.as_ref())
        }
        _ => false,
    }
}

pub(crate) fn type_has_vague_parts(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Unknown | TypeRef::Never | TypeRef::FnPtr => true,
        TypeRef::Array(inner) | TypeRef::Nullable(inner) => type_has_vague_parts(inner),
        TypeRef::Object(fields) => fields.values().any(type_has_vague_parts),
        TypeRef::Map(key, value) => type_has_vague_parts(key) || type_has_vague_parts(value),
        TypeRef::Function(signature) => {
            signature.params.iter().any(type_has_vague_parts)
                || type_has_vague_parts(signature.ret.as_ref())
        }
        _ => false,
    }
}

pub(crate) fn join_types(left: &TypeRef, right: &TypeRef) -> TypeRef {
    if left == right {
        return left.clone();
    }

    match (left, right) {
        (TypeRef::Unknown, other) | (TypeRef::Never, other) => other.clone(),
        (other, TypeRef::Unknown) | (other, TypeRef::Never) => other.clone(),
        (TypeRef::FnPtr, TypeRef::Function(signature))
        | (TypeRef::Function(signature), TypeRef::FnPtr) => TypeRef::Function(signature.clone()),
        (TypeRef::Object(fields), other) | (other, TypeRef::Object(fields))
            if fields.is_empty()
                && matches!(
                    other,
                    TypeRef::Object(_) | TypeRef::Map(_, _) | TypeRef::Array(_)
                ) =>
        {
            other.clone()
        }
        (TypeRef::Array(left), TypeRef::Array(right)) => {
            TypeRef::Array(Box::new(join_types(left, right)))
        }
        (TypeRef::Array(items), TypeRef::Map(key, value))
        | (TypeRef::Map(key, value), TypeRef::Array(items))
            if matches!(
                key.as_ref(),
                TypeRef::Int | TypeRef::Unknown | TypeRef::Never
            ) =>
        {
            TypeRef::Array(Box::new(join_types(items, value)))
        }
        (TypeRef::Object(left_fields), TypeRef::Object(right_fields)) => {
            let mut merged = left_fields.clone();
            for (name, right_ty) in right_fields {
                let next = match merged.get(name.as_str()) {
                    Some(left_ty) => join_types(left_ty, right_ty),
                    None => right_ty.clone(),
                };
                merged.insert(name.clone(), next);
            }
            TypeRef::Object(merged)
        }
        (TypeRef::Map(left_key, left_value), TypeRef::Map(right_key, right_value)) => TypeRef::Map(
            Box::new(join_types(left_key, right_key)),
            Box::new(join_types(left_value, right_value)),
        ),
        (
            TypeRef::Function(FunctionTypeRef {
                params: left_params,
                ret: left_ret,
            }),
            TypeRef::Function(FunctionTypeRef {
                params: right_params,
                ret: right_ret,
            }),
        ) if left_params.len() == right_params.len() => TypeRef::Function(FunctionTypeRef {
            params: left_params
                .iter()
                .zip(right_params.iter())
                .map(|(left, right)| join_types(left, right))
                .collect(),
            ret: Box::new(join_types(left_ret, right_ret)),
        }),
        _ => make_union(left, right),
    }
}

pub(crate) fn make_union(left: &TypeRef, right: &TypeRef) -> TypeRef {
    let mut members = Vec::new();
    push_union_member(&mut members, left);
    push_union_member(&mut members, right);
    if members.len() == 1 {
        members.pop().expect("expected a single union member")
    } else {
        TypeRef::Union(members)
    }
}

pub(crate) fn push_union_member(members: &mut Vec<TypeRef>, ty: &TypeRef) {
    match ty {
        TypeRef::Union(items) => {
            for item in items {
                push_union_member(members, item);
            }
        }
        other if !members.iter().any(|existing| existing == other) => members.push(other.clone()),
        _ => {}
    }
}

pub(crate) fn nested_mutation_container_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    segments: &[MutationPathSegment],
    value: TypeRef,
) -> TypeRef {
    let mut current = value;
    for segment in segments.iter().rev() {
        current = match segment {
            MutationPathSegment::Field { name } => object_type_with_field(name, current),
            MutationPathSegment::Index { index } => {
                match inferred_expr_type(hir, inference, *index) {
                    Some(TypeRef::Int) => TypeRef::Array(Box::new(current)),
                    Some(index_ty) => TypeRef::Map(Box::new(index_ty), Box::new(current)),
                    None => TypeRef::Map(Box::new(TypeRef::Unknown), Box::new(current)),
                }
            }
        };
    }
    current
}

pub(crate) fn matches_top_level_field_segment(
    segments: &[MutationPathSegment],
    field_name: &str,
) -> bool {
    matches!(
        segments,
        [MutationPathSegment::Field { name }] if name == field_name
    )
}

pub(crate) fn object_type_with_field(name: &str, value: TypeRef) -> TypeRef {
    TypeRef::Object(BTreeMap::from([(name.to_owned(), value)]))
}
