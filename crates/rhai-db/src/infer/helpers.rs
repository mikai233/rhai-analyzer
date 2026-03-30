use std::collections::BTreeMap;

use crate::FileTypeInference;
use crate::infer::calls::inferred_expr_type;
use rhai_hir::{
    ExprId, FileHir, FunctionTypeRef, LiteralKind, MutationPathSegment, SymbolId, TypeRef,
};

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
        TypeRef::Unknown | TypeRef::Never | TypeRef::FnPtr | TypeRef::Ambiguous(_) => true,
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
        (TypeRef::Ambiguous(items), other) | (other, TypeRef::Ambiguous(items)) => {
            make_ambiguous_type(
                items
                    .iter()
                    .cloned()
                    .chain(std::iter::once(other.clone()))
                    .collect(),
            )
        }
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
        (TypeRef::Array(items), other)
            if matches!(items.as_ref(), TypeRef::Unknown | TypeRef::Never)
                && matches!(other, TypeRef::Array(_)) =>
        {
            other.clone()
        }
        (other, TypeRef::Array(items))
            if matches!(items.as_ref(), TypeRef::Unknown | TypeRef::Never)
                && matches!(other, TypeRef::Array(_)) =>
        {
            other.clone()
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

pub(crate) fn make_ambiguous_type(types: Vec<TypeRef>) -> TypeRef {
    let mut members = Vec::new();
    for ty in types {
        push_ambiguous_member(&mut members, ty);
    }

    if members.len() == 1 {
        members.pop().expect("expected a single ambiguous member")
    } else {
        TypeRef::Ambiguous(members)
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
        other => {
            if let Some(index) = members
                .iter()
                .position(|existing| union_member_subsumes(existing, other))
            {
                members[index] = other.clone();
                return;
            }

            if members
                .iter()
                .any(|existing| existing == other || union_member_subsumes(other, existing))
            {
                return;
            }

            members.push(other.clone());
        }
    }
}

pub(crate) fn push_ambiguous_member(members: &mut Vec<TypeRef>, ty: TypeRef) {
    match ty {
        TypeRef::Ambiguous(items) => {
            for item in items {
                push_ambiguous_member(members, item);
            }
        }
        other => {
            if let Some(index) = members
                .iter()
                .position(|existing| union_member_subsumes(existing, &other))
            {
                members[index] = other;
                return;
            }

            if members
                .iter()
                .any(|existing| existing == &other || union_member_subsumes(&other, existing))
            {
                return;
            }

            members.push(other);
        }
    }
}

fn union_member_subsumes(existing: &TypeRef, next: &TypeRef) -> bool {
    match (existing, next) {
        (TypeRef::Array(existing), TypeRef::Array(next)) => {
            matches!(existing.as_ref(), TypeRef::Unknown | TypeRef::Never)
                && !matches!(next.as_ref(), TypeRef::Unknown | TypeRef::Never)
        }
        _ => false,
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

pub(crate) fn path_segment_read_type(
    hir: &FileHir,
    current: &TypeRef,
    segment: &MutationPathSegment,
) -> Option<TypeRef> {
    match segment {
        MutationPathSegment::Field { name } => match current {
            TypeRef::Object(fields) => fields
                .get(name)
                .cloned()
                .or_else(|| inferred_object_value_union(fields)),
            TypeRef::Map(_, value) => Some((**value).clone()),
            _ => None,
        },
        MutationPathSegment::Index { index } => match current {
            TypeRef::Array(inner) => Some((**inner).clone()),
            TypeRef::Map(_, value) => Some((**value).clone()),
            TypeRef::Object(fields) => string_index_field_name(hir, *index)
                .and_then(|name| fields.get(name).cloned())
                .or_else(|| inferred_object_value_union(fields)),
            TypeRef::Nullable(inner) => path_segment_read_type(hir, inner, segment),
            TypeRef::Union(items) => items
                .iter()
                .filter_map(|item| path_segment_read_type(hir, item, segment))
                .reduce(|left, right| join_types(&left, &right)),
            _ => None,
        },
    }
}

pub(crate) fn read_type_from_segments(
    hir: &FileHir,
    root: &TypeRef,
    segments: &[MutationPathSegment],
) -> Option<TypeRef> {
    let mut current = root.clone();
    for segment in segments {
        current = path_segment_read_type(hir, &current, segment)?;
    }
    Some(current)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadTargetKey {
    pub(crate) symbol: SymbolId,
    pub(crate) segments: Vec<ReadTargetSegmentKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadTargetSegmentKey {
    Field(String),
    Index(ReadTargetIndexKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadTargetIndexKey {
    Symbol(SymbolId),
    String(String),
    Int(String),
}

pub(crate) fn symbol_target_key(symbol: SymbolId) -> ReadTargetKey {
    ReadTargetKey {
        symbol,
        segments: Vec::new(),
    }
}

pub(crate) fn read_target_key_for_expr(hir: &FileHir, expr: ExprId) -> Option<ReadTargetKey> {
    match hir.expr(expr).kind {
        rhai_hir::ExprKind::Paren => hir
            .expr_at_offset(hir.expr(expr).range.start())
            .filter(|inner| *inner != expr)
            .and_then(|inner| read_target_key_for_expr(hir, inner)),
        rhai_hir::ExprKind::Name => {
            let reference = hir.reference_at(hir.expr(expr).range)?;
            let symbol = hir.definition_of(reference)?;
            Some(symbol_target_key(symbol))
        }
        rhai_hir::ExprKind::Field | rhai_hir::ExprKind::Index => {
            let read = hir.symbol_read(expr)?;
            read_target_key_from_symbol_read(hir, read.symbol, &read.kind)
        }
        _ => None,
    }
}

pub(crate) fn read_target_key_from_symbol_read(
    hir: &FileHir,
    symbol: SymbolId,
    kind: &rhai_hir::SymbolReadKind,
) -> Option<ReadTargetKey> {
    let rhai_hir::SymbolReadKind::Path { segments } = kind;
    let mut converted = Vec::with_capacity(segments.len());
    for segment in segments {
        match segment {
            MutationPathSegment::Field { name } => {
                converted.push(ReadTargetSegmentKey::Field(name.clone()));
            }
            MutationPathSegment::Index { index } => {
                converted.push(ReadTargetSegmentKey::Index(index_key_for_expr(
                    hir, *index,
                )?));
            }
        }
    }

    Some(ReadTargetKey {
        symbol,
        segments: converted,
    })
}

fn index_key_for_expr(hir: &FileHir, expr: ExprId) -> Option<ReadTargetIndexKey> {
    match hir.expr(expr).kind {
        rhai_hir::ExprKind::Name => {
            let reference = hir.reference_at(hir.expr(expr).range)?;
            let symbol = hir.definition_of(reference)?;
            Some(ReadTargetIndexKey::Symbol(symbol))
        }
        rhai_hir::ExprKind::Literal => {
            let literal = hir.literal(expr)?;
            match literal.kind {
                LiteralKind::String => Some(ReadTargetIndexKey::String(
                    literal
                        .text
                        .as_deref()
                        .and_then(unquote_string_index_literal)?
                        .to_owned(),
                )),
                LiteralKind::Int => Some(ReadTargetIndexKey::Int(literal.text.clone()?)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn unquote_string_index_literal(text: &str) -> Option<&str> {
    (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
        .then_some(&text[1..text.len() - 1])
}

fn string_index_field_name(hir: &FileHir, expr: rhai_hir::ExprId) -> Option<&str> {
    let literal = hir.literal(expr)?;
    (literal.kind == rhai_hir::LiteralKind::String)
        .then_some(literal.text.as_deref())
        .flatten()
        .and_then(|text| {
            (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
                .then_some(&text[1..text.len() - 1])
        })
}

fn inferred_object_value_union(fields: &BTreeMap<String, TypeRef>) -> Option<TypeRef> {
    fields
        .values()
        .cloned()
        .reduce(|left, right| join_types(&left, &right))
}
