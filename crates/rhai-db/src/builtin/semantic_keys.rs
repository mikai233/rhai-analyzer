use rhai_hir::{AssignmentOperator, BinaryOperator, TypeRef, UnaryOperator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinSemanticKey {
    ArrayIndex,
    ArrayRangeIndex,
    BlobIndex,
    BlobRangeIndex,
    StringIndex,
    StringRangeIndex,
    MapIndex,
    DynamicTagPropertyAccess,
    MapPropertyAccess,
    IntBitIndex,
    IntBitRangeIndex,
    ContainsArray,
    ContainsString,
    ContainsBlob,
    ContainsMap,
    ContainsRange,
    RangeOperator,
    RangeInclusiveOperator,
    NumericAddition,
    NumericArithmetic,
    EqualityString,
    EqualityScalar,
    EqualityContainer,
    ComparisonString,
    ComparisonNumber,
    StringConcatenation,
    ArrayConcatenation,
    BlobConcatenation,
    MapMerge,
    NullCoalesce,
    NumericAssignment,
    StringAppendAssignment,
    ArrayAppendAssignment,
    BlobAppendAssignment,
    BitwiseAssignment,
    NullCoalesceAssignment,
    UnaryPlusNumber,
    UnaryMinusNumber,
    LogicalNotBool,
}

pub fn builtin_unary_semantic_key(
    operator: UnaryOperator,
    operand_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match operator {
        UnaryOperator::Plus if operand_ty.is_some_and(type_may_be_numeric) => {
            Some(BuiltinSemanticKey::UnaryPlusNumber)
        }
        UnaryOperator::Minus if operand_ty.is_some_and(type_may_be_numeric) => {
            Some(BuiltinSemanticKey::UnaryMinusNumber)
        }
        UnaryOperator::Not if operand_ty.is_some_and(type_may_be_bool) => {
            Some(BuiltinSemanticKey::LogicalNotBool)
        }
        _ => None,
    }
}

pub fn builtin_binary_semantic_key(
    operator: BinaryOperator,
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match operator {
        BinaryOperator::In => contains_semantic_key(rhs_ty),
        BinaryOperator::Range => Some(BuiltinSemanticKey::RangeOperator),
        BinaryOperator::RangeInclusive => Some(BuiltinSemanticKey::RangeInclusiveOperator),
        BinaryOperator::Add => additive_semantic_key(lhs_ty, rhs_ty),
        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Power
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Or
        | BinaryOperator::Xor
        | BinaryOperator::And
            if lhs_ty.is_some_and(type_may_be_numeric)
                && rhs_ty.is_some_and(type_may_be_numeric) =>
        {
            Some(BuiltinSemanticKey::NumericArithmetic)
        }
        BinaryOperator::EqEq | BinaryOperator::NotEq => equality_semantic_key(lhs_ty, rhs_ty),
        BinaryOperator::Gt | BinaryOperator::GtEq | BinaryOperator::Lt | BinaryOperator::LtEq => {
            comparison_semantic_key(lhs_ty, rhs_ty)
        }
        BinaryOperator::NullCoalesce => Some(BuiltinSemanticKey::NullCoalesce),
        BinaryOperator::OrOr | BinaryOperator::AndAnd => None,
        _ => None,
    }
}

pub fn builtin_assignment_semantic_key(
    operator: AssignmentOperator,
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match operator {
        AssignmentOperator::Assign => None,
        AssignmentOperator::NullCoalesce => Some(BuiltinSemanticKey::NullCoalesceAssignment),
        AssignmentOperator::Add => additive_assignment_semantic_key(lhs_ty, rhs_ty),
        AssignmentOperator::Subtract
        | AssignmentOperator::Multiply
        | AssignmentOperator::Divide
        | AssignmentOperator::Remainder
        | AssignmentOperator::Power
            if lhs_ty.is_some_and(type_may_be_numeric)
                && rhs_ty.is_some_and(type_may_be_numeric) =>
        {
            Some(BuiltinSemanticKey::NumericAssignment)
        }
        AssignmentOperator::ShiftLeft
        | AssignmentOperator::ShiftRight
        | AssignmentOperator::Or
        | AssignmentOperator::Xor
        | AssignmentOperator::And
            if lhs_ty.is_some_and(type_may_be_numeric)
                && rhs_ty.is_some_and(type_may_be_numeric) =>
        {
            Some(BuiltinSemanticKey::BitwiseAssignment)
        }
        _ => None,
    }
}

pub fn builtin_index_semantic_key(
    receiver_ty: &TypeRef,
    index_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match receiver_ty {
        TypeRef::Array(_) => {
            if is_range_index(index_ty) {
                Some(BuiltinSemanticKey::ArrayRangeIndex)
            } else {
                Some(BuiltinSemanticKey::ArrayIndex)
            }
        }
        TypeRef::Blob => {
            if is_range_index(index_ty) {
                Some(BuiltinSemanticKey::BlobRangeIndex)
            } else {
                Some(BuiltinSemanticKey::BlobIndex)
            }
        }
        TypeRef::String => {
            if is_range_index(index_ty) {
                Some(BuiltinSemanticKey::StringRangeIndex)
            } else {
                Some(BuiltinSemanticKey::StringIndex)
            }
        }
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(BuiltinSemanticKey::MapIndex),
        TypeRef::Int => {
            if is_range_index(index_ty) {
                Some(BuiltinSemanticKey::IntBitRangeIndex)
            } else if is_int_index(index_ty) {
                Some(BuiltinSemanticKey::IntBitIndex)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn builtin_property_access_semantic_key(
    receiver_ty: &TypeRef,
    field_name: &str,
) -> Option<BuiltinSemanticKey> {
    if field_name == "tag" {
        return if !type_may_be_map_like(receiver_ty) {
            Some(BuiltinSemanticKey::DynamicTagPropertyAccess)
        } else {
            Some(BuiltinSemanticKey::MapPropertyAccess)
        };
    }

    match receiver_ty {
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(BuiltinSemanticKey::MapPropertyAccess),
        _ => None,
    }
}

pub fn type_may_be_numeric(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => true,
        TypeRef::Nullable(inner) => type_may_be_numeric(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_numeric)
        }
        _ => false,
    }
}

pub fn type_may_be_bool(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Bool => true,
        TypeRef::Nullable(inner) => type_may_be_bool(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_bool)
        }
        _ => false,
    }
}

fn type_may_be_stringy(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::String | TypeRef::Char => true,
        TypeRef::Nullable(inner) => type_may_be_stringy(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_stringy)
        }
        _ => false,
    }
}

fn type_may_be_array(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Array(_) => true,
        TypeRef::Nullable(inner) => type_may_be_array(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_array)
        }
        _ => false,
    }
}

fn type_may_be_blob(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Blob => true,
        TypeRef::Nullable(inner) => type_may_be_blob(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_blob)
        }
        _ => false,
    }
}

fn type_may_be_map_like(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Map(_, _) | TypeRef::Object(_) => true,
        TypeRef::Nullable(inner) => type_may_be_map_like(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_map_like)
        }
        _ => false,
    }
}

fn additive_semantic_key(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match (lhs_ty?, rhs_ty?) {
        (lhs, rhs) if type_may_be_numeric(lhs) && type_may_be_numeric(rhs) => {
            Some(BuiltinSemanticKey::NumericAddition)
        }
        (lhs, rhs) if type_may_be_stringy(lhs) && type_may_be_stringy(rhs) => {
            Some(BuiltinSemanticKey::StringConcatenation)
        }
        (lhs, rhs) if type_may_be_array(lhs) && type_may_be_array(rhs) => {
            Some(BuiltinSemanticKey::ArrayConcatenation)
        }
        (lhs, rhs) if type_may_be_blob(lhs) && type_may_be_blob(rhs) => {
            Some(BuiltinSemanticKey::BlobConcatenation)
        }
        (lhs, rhs) if type_may_be_map_like(lhs) && type_may_be_map_like(rhs) => {
            Some(BuiltinSemanticKey::MapMerge)
        }
        _ => None,
    }
}

fn additive_assignment_semantic_key(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    match (lhs_ty?, rhs_ty?) {
        (lhs, rhs) if type_may_be_numeric(lhs) && type_may_be_numeric(rhs) => {
            Some(BuiltinSemanticKey::NumericAssignment)
        }
        (lhs, rhs) if type_may_be_stringy(lhs) && type_may_be_stringy(rhs) => {
            Some(BuiltinSemanticKey::StringAppendAssignment)
        }
        (lhs, rhs) if type_may_be_array(lhs) && type_may_be_array(rhs) => {
            Some(BuiltinSemanticKey::ArrayAppendAssignment)
        }
        (lhs, rhs) if type_may_be_blob(lhs) && type_may_be_blob(rhs) => {
            Some(BuiltinSemanticKey::BlobAppendAssignment)
        }
        _ => None,
    }
}

fn contains_semantic_key(rhs_ty: Option<&TypeRef>) -> Option<BuiltinSemanticKey> {
    let rhs = rhs_ty?;
    match rhs {
        TypeRef::Array(_) => Some(BuiltinSemanticKey::ContainsArray),
        TypeRef::String => Some(BuiltinSemanticKey::ContainsString),
        TypeRef::Blob => Some(BuiltinSemanticKey::ContainsBlob),
        TypeRef::Map(_, _) | TypeRef::Object(_) => Some(BuiltinSemanticKey::ContainsMap),
        TypeRef::Range | TypeRef::RangeInclusive => Some(BuiltinSemanticKey::ContainsRange),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => members
            .iter()
            .find_map(|member| contains_semantic_key(Some(member))),
        _ => None,
    }
}

fn equality_semantic_key(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;
    match ty {
        TypeRef::String | TypeRef::Char => Some(BuiltinSemanticKey::EqualityString),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal | TypeRef::Bool => {
            Some(BuiltinSemanticKey::EqualityScalar)
        }
        TypeRef::Array(_) | TypeRef::Blob | TypeRef::Map(_, _) | TypeRef::Object(_) => {
            Some(BuiltinSemanticKey::EqualityContainer)
        }
        _ => None,
    }
}

fn comparison_semantic_key(
    lhs_ty: Option<&TypeRef>,
    rhs_ty: Option<&TypeRef>,
) -> Option<BuiltinSemanticKey> {
    let ty = dominant_operator_type(lhs_ty, rhs_ty)?;
    match ty {
        TypeRef::String | TypeRef::Char => Some(BuiltinSemanticKey::ComparisonString),
        TypeRef::Int | TypeRef::Float | TypeRef::Decimal => {
            Some(BuiltinSemanticKey::ComparisonNumber)
        }
        _ => None,
    }
}

fn dominant_operator_type<'a>(
    lhs_ty: Option<&'a TypeRef>,
    rhs_ty: Option<&'a TypeRef>,
) -> Option<&'a TypeRef> {
    lhs_ty
        .filter(|ty| is_operator_topic_type(ty))
        .or_else(|| rhs_ty.filter(|ty| is_operator_topic_type(ty)))
}

fn is_operator_topic_type(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int
        | TypeRef::Float
        | TypeRef::Decimal
        | TypeRef::String
        | TypeRef::Char
        | TypeRef::Blob
        | TypeRef::Map(_, _)
        | TypeRef::Object(_)
        | TypeRef::Array(_) => true,
        TypeRef::Nullable(inner) => is_operator_topic_type(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(is_operator_topic_type)
        }
        _ => false,
    }
}

fn is_int_index(index_ty: Option<&TypeRef>) -> bool {
    index_ty.is_none_or(type_may_be_int)
}

fn is_range_index(index_ty: Option<&TypeRef>) -> bool {
    index_ty.is_some_and(type_may_be_range)
}

fn type_may_be_int(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Int => true,
        TypeRef::Nullable(inner) => type_may_be_int(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_int)
        }
        _ => false,
    }
}

fn type_may_be_range(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Range | TypeRef::RangeInclusive => true,
        TypeRef::Nullable(inner) => type_may_be_range(inner),
        TypeRef::Union(members) | TypeRef::Ambiguous(members) => {
            members.iter().any(type_may_be_range)
        }
        _ => false,
    }
}
