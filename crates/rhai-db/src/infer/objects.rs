use std::collections::BTreeMap;

use crate::infer::ImportedModuleMember;
use crate::infer::calls::{has_informative_arg_types, inferred_expr_type};
use crate::infer::helpers::{join_option_types, join_types, matches_top_level_field_segment};
use crate::{FileTypeInference, HostType, best_matching_signature_index};
use rhai_hir::{
    BodyId, ExprId, ExprKind, FileHir, FunctionTypeRef, LiteralKind, ScopeKind, SymbolId,
    SymbolMutationKind, TypeRef,
};

pub(crate) fn infer_member_type_from_expr(
    hir: &FileHir,
    inference: &FileTypeInference,
    expr: ExprId,
    field_name: &str,
) -> Option<TypeRef> {
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

pub(crate) fn symbol_for_condition_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Paren => {
            largest_inner_expr(hir, expr).and_then(|inner| symbol_for_condition_expr(hir, inner))
        }
        _ => symbol_for_expr(hir, expr),
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
    let range = hir.expr(expr).range;
    let mut parts = hir
        .references
        .iter()
        .filter(|reference| {
            matches!(
                reference.kind,
                rhai_hir::ReferenceKind::Name | rhai_hir::ReferenceKind::PathSegment
            ) && reference.range.start() >= range.start()
                && reference.range.end() <= range.end()
        })
        .collect::<Vec<_>>();

    parts.sort_by_key(|reference| reference.range.start());
    (!parts.is_empty()).then(|| {
        parts
            .into_iter()
            .map(|reference| reference.name.clone())
            .collect::<Vec<_>>()
            .join("::")
    })
}

pub(crate) fn imported_module_member_type_for_expr(
    hir: &FileHir,
    expr: ExprId,
    imported_members: &[ImportedModuleMember],
) -> Option<TypeRef> {
    let parts = qualified_path_parts(hir, expr)?;
    let (member_name, module_path) = parts.split_last()?;
    if module_path.is_empty() {
        return None;
    }
    imported_members
        .iter()
        .find(|member| member.module_path == module_path && member.name == *member_name)
        .map(|member| member.ty.clone())
}

pub(crate) fn qualified_path_parts(hir: &FileHir, expr: ExprId) -> Option<Vec<String>> {
    let range = hir.expr(expr).range;
    let mut parts = hir
        .references
        .iter()
        .filter(|reference| {
            matches!(
                reference.kind,
                rhai_hir::ReferenceKind::Name | rhai_hir::ReferenceKind::PathSegment
            ) && reference.range.start() >= range.start()
                && reference.range.end() <= range.end()
        })
        .collect::<Vec<_>>();

    parts.sort_by_key(|reference| reference.range.start());
    (!parts.is_empty()).then(|| {
        parts
            .into_iter()
            .map(|reference| reference.name.clone())
            .collect()
    })
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

pub(crate) fn receiver_matches_method_type(receiver: &TypeRef, expected: &TypeRef) -> bool {
    if receiver == expected {
        return true;
    }

    match (receiver, expected) {
        (TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never, _) => true,
        (TypeRef::Union(items), expected) => items
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
        TypeRef::Unknown | TypeRef::Any | TypeRef::Dynamic | TypeRef::Never | TypeRef::Union(_)
    )
}

pub(crate) fn host_method_signature_for_type(
    ty: &TypeRef,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    match ty {
        TypeRef::Union(items) => items
            .iter()
            .filter_map(|item| {
                host_method_signature_for_type(item, method_name, host_types, arg_types)
            })
            .reduce(join_function_signatures),
        TypeRef::Named(name) => {
            host_method_signature_for_name(name, method_name, host_types, arg_types)
        }
        TypeRef::Applied { name, .. } => {
            host_method_signature_for_name(name, method_name, host_types, arg_types)
        }
        _ => None,
    }
}

pub(crate) fn host_method_signature_for_name(
    type_name: &str,
    method_name: &str,
    host_types: &[HostType],
    arg_types: Option<&[Option<TypeRef>]>,
) -> Option<FunctionTypeRef> {
    let method = host_types
        .iter()
        .find(|ty| ty.name == type_name)?
        .methods
        .iter()
        .find(|method| method.name == method_name)?;

    let matching = method
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref().cloned())
        .filter(|signature| {
            arg_types.is_none_or(|arg_types| signature.params.len() == arg_types.len())
        })
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return None;
    }

    if let Some(arg_types) = arg_types
        && has_informative_arg_types(arg_types)
        && let Some(index) = best_matching_signature_index(matching.iter(), arg_types)
    {
        return matching.get(index).cloned();
    }

    matching.into_iter().reduce(join_function_signatures)
}

pub(crate) fn join_function_signatures(
    left: FunctionTypeRef,
    right: FunctionTypeRef,
) -> FunctionTypeRef {
    if left.params.len() != right.params.len() {
        return left;
    }

    FunctionTypeRef {
        params: left
            .params
            .iter()
            .zip(right.params.iter())
            .map(|(left, right)| join_types(left, right))
            .collect(),
        ret: Box::new(join_types(left.ret.as_ref(), right.ret.as_ref())),
    }
}
