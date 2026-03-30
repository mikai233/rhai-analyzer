use crate::builtin::signatures::host_type_name_for_type;
use std::collections::BTreeMap;

use crate::infer::ImportedModuleMember;
use crate::infer::calls::{
    has_informative_arg_types, inferred_expr_type, merge_function_candidate_signatures,
};
use crate::infer::exprs::{flow_sensitive_symbol_type_at_offset, refined_target_type_at_offset};
use crate::infer::generics::specialize_signature_with_receiver_and_arg_types;
use crate::infer::helpers::{
    join_option_types, join_types, matches_top_level_field_segment, read_target_key_for_expr,
    read_type_from_segments,
};
use crate::{FileTypeInference, HostType, best_matching_signature_indexes};
use rhai_hir::{
    BodyId, ExprId, ExprKind, FileHir, FunctionTypeRef, LiteralKind, ScopeKind, SymbolId,
    SymbolMutationKind, SymbolReadKind, TypeRef,
};

pub(crate) fn infer_member_type_from_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    field_name: &str,
) -> Option<TypeRef> {
    if let Some(ty) = infer_symbol_read_type(hir, inference, expr) {
        return Some(ty);
    }

    let resolved = field_value_exprs_from_expr(hir, expr, field_name)
        .into_iter()
        .filter_map(|value_expr| inferred_expr_type(hir, inference, value_expr))
        .reduce(|left, right| join_types(&left, &right));

    let documented = symbol_for_expr(hir, expr).and_then(|symbol| {
        hir.documented_fields(symbol)
            .into_iter()
            .find(|field| field.name == field_name)
            .map(|field| field.annotation)
    });

    let fallback_map_value = inferred_expr_type(hir, inference, expr).and_then(|ty| match ty {
        TypeRef::Object(fields) => fields.get(field_name).cloned(),
        TypeRef::Map(_, value) => Some(*value),
        _ => None,
    });

    let symbolic = join_option_types(documented.as_ref(), fallback_map_value.as_ref());
    if resolved.is_some() {
        return resolved;
    }
    if documented.is_some() {
        return documented;
    }
    symbolic
}

pub(crate) fn infer_symbol_read_type(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> Option<TypeRef> {
    let read = hir.symbol_read(expr)?;
    let SymbolReadKind::Path { segments } = &read.kind;
    let root_ty =
        flow_sensitive_symbol_type_at_offset(hir, inference, read.symbol, read.range.start())?;
    let current = read_type_from_segments(hir, &root_ty, segments)?;
    let target = read_target_key_for_expr(hir, expr)?;
    let refined =
        refined_target_type_at_offset(hir, inference, &current, &target, read.range.start());
    Some(refined)
}

pub(crate) fn field_value_exprs_from_expr(
    hir: &FileHir,
    expr: ExprId,
    field_name: &str,
) -> Vec<ExprId> {
    match hir.expr(expr).kind {
        ExprKind::Object => field_value_exprs_from_object_expr(hir, expr, field_name),
        ExprKind::Name => symbol_for_expr(hir, expr)
            .into_iter()
            .flat_map(|symbol| field_value_exprs_from_symbol(hir, symbol, field_name))
            .collect(),
        ExprKind::Field => {
            let Some(access) = hir.member_access(expr) else {
                return Vec::new();
            };
            let intermediate = hir.reference(access.field_reference).name.as_str();
            field_value_exprs_from_expr(hir, access.receiver, intermediate)
                .into_iter()
                .flat_map(|value_expr| field_value_exprs_from_expr(hir, value_expr, field_name))
                .collect()
        }
        ExprKind::Block => hir
            .block_expr(expr)
            .and_then(|block| hir.body_tail_value(block.body))
            .into_iter()
            .flat_map(|tail| field_value_exprs_from_expr(hir, tail, field_name))
            .collect(),
        ExprKind::If => hir
            .if_expr(expr)
            .into_iter()
            .flat_map(|if_expr| [if_expr.then_branch, if_expr.else_branch])
            .flatten()
            .flat_map(|branch| field_value_exprs_from_expr(hir, branch, field_name))
            .collect(),
        ExprKind::Switch => hir
            .switch_expr(expr)
            .into_iter()
            .flat_map(|switch| switch.arms.iter().flatten().copied())
            .flat_map(|arm| field_value_exprs_from_expr(hir, arm, field_name))
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn field_value_exprs_from_symbol(
    hir: &FileHir,
    symbol: SymbolId,
    field_name: &str,
) -> Vec<ExprId> {
    let mut exprs = hir
        .value_flows_into(symbol)
        .flat_map(|flow| field_value_exprs_from_expr(hir, flow.expr, field_name))
        .collect::<Vec<_>>();

    exprs.extend(
        hir.symbol_mutations_into(symbol)
            .filter_map(|mutation| match &mutation.kind {
                SymbolMutationKind::Path { segments }
                    if matches_top_level_field_segment(segments, field_name) =>
                {
                    Some(mutation.value)
                }
                _ => None,
            }),
    );

    exprs.sort_by_key(|expr| expr.0);
    exprs.dedup_by_key(|expr| expr.0);
    exprs
}

pub(crate) fn field_value_exprs_from_object_expr(
    hir: &FileHir,
    expr: ExprId,
    field_name: &str,
) -> Vec<ExprId> {
    hir.object_fields
        .iter()
        .filter(|field| field.owner == expr && field.name == field_name)
        .filter_map(|field| field.value)
        .collect()
}

pub(crate) fn symbol_for_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Name => hir
            .reference_at(hir.expr(expr).range)
            .and_then(|reference| hir.definition_of(reference)),
        _ => None,
    }
}

pub(crate) fn expr_is_unit_like(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
) -> bool {
    match hir.expr(expr).kind {
        ExprKind::Paren => largest_inner_expr(hir, expr)
            .is_none_or(|inner| expr_is_unit_like(hir, inference, inner)),
        _ => inferred_expr_type(hir, inference, expr).is_some_and(|ty| matches!(ty, TypeRef::Unit)),
    }
}

pub(crate) fn string_literal_value(hir: &FileHir, expr: ExprId) -> Option<&str> {
    let literal = hir.literal(expr)?;
    (literal.kind == LiteralKind::String)
        .then_some(literal.text.as_deref())
        .flatten()
        .and_then(unquote_string_literal)
}

pub(crate) fn unquote_string_literal(text: &str) -> Option<&str> {
    (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
        .then_some(&text[1..text.len() - 1])
}

pub(crate) fn inferred_object_value_union(fields: &BTreeMap<String, TypeRef>) -> Option<TypeRef> {
    fields
        .values()
        .cloned()
        .reduce(|left, right| join_types(&left, &right))
}

pub(crate) fn direct_loop_body(hir: &FileHir, expr: ExprId) -> Option<BodyId> {
    let range = hir.expr(expr).range;
    hir.bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| hir.scope(body.scope).kind == ScopeKind::Loop)
        .filter(|(_, body)| {
            body.range.start() >= range.start()
                && body.range.end() <= range.end()
                && body.range != range
        })
        .max_by_key(|(_, body)| body.range.len())
        .map(|(index, _)| BodyId(index as u32))
}

pub(crate) fn largest_inner_expr(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let candidate = ExprId(*index as u32);
            candidate != expr
                && node.range.start() >= range.start()
                && node.range.end() <= range.end()
                && node.range != range
        })
        .max_by_key(|(_, node)| node.range.len())
        .map(|(index, _)| ExprId(index as u32))
}

pub(crate) fn qualified_path_name(hir: &FileHir, expr: ExprId) -> Option<String> {
    hir.qualified_path_name(expr)
}

pub(crate) fn imported_module_member_type_for_expr(
    hir: &FileHir,
    expr: ExprId,
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    let parts = hir.imported_module_path(expr)?.parts;
    let (member_name, module_path) = parts.split_last()?;
    if module_path.is_empty() {
        return None;
    }
    imported_members
        .iter()
        .find(|member| member.module_path == module_path && member.name == *member_name)
        .map(|member| member.ty.clone())
}

pub(crate) fn host_method_signature_for_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let access = hir.member_access(expr)?;
    let method_name = hir.reference(access.field_reference).name.as_str();
    let receiver_ty = inferred_expr_type(hir, inference, access.receiver)?;
    host_method_signature_for_type(&receiver_ty, method_name, host_types, arg_types)
}

pub(crate) fn receiver_supports_field_method_ambiguity(
    hir: &FileHir,
    inference: &FileTypeInference,
    receiver: ExprId,
) -> bool {
    inferred_expr_type(hir, inference, receiver)
        .is_some_and(|ty| type_supports_field_method_ambiguity(&ty))
}

fn type_supports_field_method_ambiguity(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Map(_, _) | TypeRef::Object(_) => true,
        TypeRef::Nullable(inner) => type_supports_field_method_ambiguity(inner),
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            items.iter().any(type_supports_field_method_ambiguity)
        }
        _ => false,
    }
}

pub(crate) fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) | (TypeRef::Ambiguous(items), expected) => items
            .iter()
            .any(|item| receiver_matches_method_type(item, expected)),
        (TypeRef::Nullable(inner), expected) => receiver_matches_method_type(inner, expected),
        (TypeRef::Applied { name, .. }, TypeRef::Named(expected_name))
        | (
            TypeRef::Named(name),
            TypeRef::Applied {
                name: expected_name,
                ..
            },
        ) => name == expected_name,
        (
            TypeRef::Applied { name, args },
            TypeRef::Applied {
                name: expected_name,
                args: expected_args,
            },
        ) => {
            name == expected_name
                && args.len() == expected_args.len()
                && args
                    .iter()
                    .zip(expected_args.iter())
                    .all(|(arg, expected)| receiver_matches_method_type(arg, expected))
        }
        _ => false,
    }
}

pub(crate) fn receiver_dispatch_is_precise(receiver: &TypeRef) -> bool {
    !matches!(
        receiver,
        TypeRef::Unknown
            | TypeRef::Any
            | TypeRef::Dynamic
            | TypeRef::Never
            | TypeRef::Union(_)
            | TypeRef::Ambiguous(_)
    )
}

pub(crate) fn host_method_signature_for_type(
    ty: &TypeRef,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            let signatures = items
                .iter()
                .filter_map(|item| {
                    host_method_signature_for_type(item, method_name, host_types, arg_types)
                })
                .collect::<Vec<_>>();
            merge_function_candidate_signatures(signatures, arg_types.map(|items| items.len()))
        }
        TypeRef::Nullable(inner) => {
            host_method_signature_for_type(inner, method_name, host_types, arg_types)
        }
        _ if specialized_builtin_method_signature(ty, method_name, arg_types).is_some() => {
            specialized_builtin_method_signature(ty, method_name, arg_types)
        }
        _ if host_type_name_for_type(ty).is_some() => host_method_signature_for_name(
            ty,
            host_type_name_for_type(ty).expect("checked builtin host type name"),
            method_name,
            host_types,
            arg_types,
        ),
        TypeRef::Named(name) => {
            host_method_signature_for_name(ty, name, method_name, host_types, arg_types)
        }
        TypeRef::Applied { name, .. } => {
            host_method_signature_for_name(ty, name, method_name, host_types, arg_types)
        }
        _ => None,
    }
}

fn specialized_builtin_method_signature(
    ty: &TypeRef,
    method_name: &str,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Array(inner) => {
            specialized_array_method_signature(inner.as_ref().clone(), method_name, arg_types)
        }
        TypeRef::Map(_, value) => {
            specialized_map_like_method_signature(value.as_ref().clone(), method_name, arg_types)
        }
        TypeRef::Object(fields) => specialized_map_like_method_signature(
            inferred_object_value_union(fields).unwrap_or(TypeRef::Unknown),
            method_name,
            arg_types,
        ),
        _ => None,
    }
}

fn specialized_array_method_signature(
    inner: TypeRef,
    method_name: &str,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match method_name {
        "get" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Union(vec![inner, TypeRef::Unit])),
        }),
        "pop" | "shift" if arg_count_matches(arg_types, 0) => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Union(vec![inner, TypeRef::Unit])),
        }),
        "remove" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Union(vec![inner, TypeRef::Unit])),
        }),
        "push" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![inner],
            ret: Box::new(TypeRef::Unit),
        }),
        "append" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![TypeRef::Array(Box::new(inner))],
            ret: Box::new(TypeRef::Unit),
        }),
        "insert" if arg_count_matches(arg_types, 2) => Some(FunctionTypeRef {
            params: vec![TypeRef::Int, inner],
            ret: Box::new(TypeRef::Unit),
        }),
        "contains" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![inner],
            ret: Box::new(TypeRef::Bool),
        }),
        "index_of" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![inner],
            ret: Box::new(TypeRef::Int),
        }),
        "index_of" if arg_count_matches(arg_types, 2) => Some(FunctionTypeRef {
            params: vec![inner, TypeRef::Int],
            ret: Box::new(TypeRef::Int),
        }),
        _ => None,
    }
}

fn specialized_map_like_method_signature(
    value: TypeRef,
    method_name: &str,
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match method_name {
        "get" | "remove" if arg_count_matches(arg_types, 1) => Some(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Union(vec![value, TypeRef::Unit])),
        }),
        "set" if arg_count_matches(arg_types, 2) => Some(FunctionTypeRef {
            params: vec![TypeRef::String, value],
            ret: Box::new(TypeRef::Unit),
        }),
        "values" if arg_count_matches(arg_types, 0) => Some(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Array(Box::new(value))),
        }),
        _ => None,
    }
}

fn arg_count_matches(arg_types: Option<&[Option<TypeRef>]>, expected: usize) -> bool {
    arg_types.is_none_or(|arg_types| arg_types.len() == expected)
}

pub(crate) fn host_method_signature_for_name(
    receiver_ty: &TypeRef,
    type_name: &str,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let host_type = host_types.iter().find(|ty| ty.name == type_name)?;
    let method = host_type
        .methods
        .iter()
        .find(|method| method.name == method_name)?;

    let matching = method
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .map(|signature| {
            specialize_signature_with_receiver_and_arg_types(
                signature,
                Some(receiver_ty),
                host_type.generic_params.as_slice(),
                arg_types,
                host_types,
            )
        })
        .filter(|signature| {
            arg_types.is_none_or(|arg_types| signature.params.len() == arg_types.len())
        })
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return None;
    }

    if let Some(arg_types) = arg_types
        && has_informative_arg_types(arg_types)
    {
        let indexes = best_matching_signature_indexes(matching.iter(), arg_types);
        if !indexes.is_empty() {
            return merge_function_candidate_signatures(
                indexes
                    .into_iter()
                    .filter_map(|index| matching.get(index).cloned())
                    .collect(),
                Some(arg_types.len()),
            );
        }
    }

    merge_function_candidate_signatures(matching, arg_types.map(|items| items.len()))
}
