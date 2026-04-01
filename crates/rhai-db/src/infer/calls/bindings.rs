use crate::infer::helpers::join_types;
use rhai_hir::TypeRef;

pub(crate) fn for_binding_types_from_iterable(
    ty: &TypeRef,
    binding_count: usize,
) -> Option<Vec<TypeRef>> {
    if binding_count == 0 {
        return Some(Vec::new());
    }

    match ty {
        TypeRef::Array(inner) => Some(loop_binding_types(
            inner.as_ref().clone(),
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::String => Some(loop_binding_types(
            TypeRef::Char,
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::Range | TypeRef::RangeInclusive => Some(loop_binding_types(
            TypeRef::Int,
            binding_count,
            TypeRef::Int,
        )),
        TypeRef::Union(items) => {
            let mut merged = None;
            for item in items {
                let Some(next) = for_binding_types_from_iterable(item, binding_count) else {
                    continue;
                };
                merged = Some(match merged {
                    Some(current) => join_binding_type_sets(current, next),
                    None => next,
                });
            }
            merged
        }
        TypeRef::Ambiguous(items) => {
            let mut merged = None;
            for item in items {
                let Some(next) = for_binding_types_from_iterable(item, binding_count) else {
                    continue;
                };
                merged = Some(match merged {
                    Some(current) => join_binding_type_sets(current, next),
                    None => next,
                });
            }
            merged
        }
        _ => None,
    }
}
pub(crate) fn loop_binding_types(
    item_ty: TypeRef,
    binding_count: usize,
    counter_ty: TypeRef,
) -> Vec<TypeRef> {
    let mut binding_types = vec![TypeRef::Unknown; binding_count];
    if let Some(first) = binding_types.first_mut() {
        *first = item_ty;
    }
    if binding_count > 1 {
        binding_types[1] = counter_ty;
    }
    binding_types
}
pub(crate) fn join_binding_type_sets(left: Vec<TypeRef>, right: Vec<TypeRef>) -> Vec<TypeRef> {
    let len = left.len().max(right.len());
    (0..len)
        .map(|index| match (left.get(index), right.get(index)) {
            (Some(left), Some(right)) => join_types(left, right),
            (Some(left), None) => left.clone(),
            (None, Some(right)) => right.clone(),
            (None, None) => TypeRef::Unknown,
        })
        .collect()
}
