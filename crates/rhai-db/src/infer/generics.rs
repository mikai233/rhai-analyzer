use std::collections::HashMap;

use rhai_hir::{FunctionTypeRef, TypeRef};

use crate::HostType;
use crate::infer::helpers::join_types;

pub(crate) fn specialize_signature_with_arg_types(
    signature: &FunctionTypeRef,
    arg_types: Option<&[Option<TypeRef>]>,
    host_types: &[HostType],
) -> FunctionTypeRef {
    specialize_signature_with_receiver_and_arg_types(signature, None, &[], arg_types, host_types)
}

pub fn specialize_signature_with_receiver_and_arg_types(
    signature: &FunctionTypeRef,
    receiver_type: Option<&TypeRef>,
    receiver_generic_params: &[String],
    arg_types: Option<&[Option<TypeRef>]>,
    host_types: &[HostType],
) -> FunctionTypeRef {
    let generic_names = collect_signature_generic_names(signature, host_types);
    if generic_names.is_empty() {
        return signature.clone();
    }

    let mut bindings = HashMap::<String, TypeRef>::new();
    if let Some(receiver_type) = receiver_type {
        infer_receiver_type_bindings(
            receiver_generic_params,
            receiver_type,
            &generic_names,
            &mut bindings,
        );
    }

    if let Some(arg_types) = arg_types {
        for (expected, actual) in signature.params.iter().zip(arg_types.iter().flatten()) {
            infer_type_bindings(expected, actual, host_types, &generic_names, &mut bindings);
        }
    }

    apply_bindings_to_signature(signature, host_types, &generic_names, &bindings)
}

fn collect_signature_generic_names(
    signature: &FunctionTypeRef,
    host_types: &[HostType],
) -> Vec<String> {
    let mut names = Vec::new();
    for parameter in &signature.params {
        collect_type_generic_names(parameter, host_types, &mut names);
    }
    collect_type_generic_names(signature.ret.as_ref(), host_types, &mut names);
    names
}

fn infer_receiver_type_bindings(
    receiver_generic_params: &[String],
    receiver_type: &TypeRef,
    generic_names: &[String],
    bindings: &mut HashMap<String, TypeRef>,
) {
    match receiver_type {
        TypeRef::Applied { args, .. } if args.len() == receiver_generic_params.len() => {
            for (param, actual) in receiver_generic_params.iter().zip(args.iter()) {
                if generic_names.iter().any(|generic| generic == param) {
                    bind_type_parameter(bindings, param, actual.clone());
                }
            }
        }
        TypeRef::Nullable(inner) => {
            infer_receiver_type_bindings(receiver_generic_params, inner, generic_names, bindings);
        }
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                infer_receiver_type_bindings(
                    receiver_generic_params,
                    item,
                    generic_names,
                    bindings,
                );
            }
        }
        _ => {}
    }
}

fn collect_type_generic_names(ty: &TypeRef, host_types: &[HostType], names: &mut Vec<String>) {
    match ty {
        TypeRef::Named(name) if is_generic_type_parameter(name, host_types) => {
            if !names.iter().any(|existing| existing == name) {
                names.push(name.clone());
            }
        }
        TypeRef::Applied { args, .. } | TypeRef::Union(args) | TypeRef::Ambiguous(args) => {
            for arg in args {
                collect_type_generic_names(arg, host_types, names);
            }
        }
        TypeRef::Array(inner) | TypeRef::Nullable(inner) => {
            collect_type_generic_names(inner, host_types, names);
        }
        TypeRef::Map(key, value) => {
            collect_type_generic_names(key, host_types, names);
            collect_type_generic_names(value, host_types, names);
        }
        TypeRef::Object(fields) => {
            for value in fields.values() {
                collect_type_generic_names(value, host_types, names);
            }
        }
        TypeRef::Function(signature) => {
            for parameter in &signature.params {
                collect_type_generic_names(parameter, host_types, names);
            }
            collect_type_generic_names(signature.ret.as_ref(), host_types, names);
        }
        _ => {}
    }
}

fn infer_type_bindings(
    expected: &TypeRef,
    actual: &TypeRef,
    host_types: &[HostType],
    generic_names: &[String],
    bindings: &mut HashMap<String, TypeRef>,
) {
    match expected {
        TypeRef::Named(name)
            if generic_names.iter().any(|generic| generic == name)
                && is_generic_type_parameter(name, host_types) =>
        {
            bind_type_parameter(bindings, name, actual.clone());
        }
        TypeRef::Array(inner) => match actual {
            TypeRef::Array(actual_inner) => {
                infer_type_bindings(inner, actual_inner, host_types, generic_names, bindings);
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => {}
        },
        TypeRef::Map(expected_key, expected_value) => match actual {
            TypeRef::Map(actual_key, actual_value) => {
                infer_type_bindings(
                    expected_key,
                    actual_key,
                    host_types,
                    generic_names,
                    bindings,
                );
                infer_type_bindings(
                    expected_value,
                    actual_value,
                    host_types,
                    generic_names,
                    bindings,
                );
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => {}
        },
        TypeRef::Nullable(inner) => match actual {
            TypeRef::Nullable(actual_inner) => {
                infer_type_bindings(inner, actual_inner, host_types, generic_names, bindings);
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => infer_type_bindings(inner, actual, host_types, generic_names, bindings),
        },
        TypeRef::Applied {
            name: expected_name,
            args: expected_args,
        } => match actual {
            TypeRef::Applied {
                name: actual_name,
                args: actual_args,
            } if expected_name == actual_name && expected_args.len() == actual_args.len() => {
                for (expected_arg, actual_arg) in expected_args.iter().zip(actual_args.iter()) {
                    infer_type_bindings(
                        expected_arg,
                        actual_arg,
                        host_types,
                        generic_names,
                        bindings,
                    );
                }
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => {}
        },
        TypeRef::Object(expected_fields) => match actual {
            TypeRef::Object(actual_fields) => {
                for (name, expected_field) in expected_fields {
                    let Some(actual_field) = actual_fields.get(name) else {
                        continue;
                    };
                    infer_type_bindings(
                        expected_field,
                        actual_field,
                        host_types,
                        generic_names,
                        bindings,
                    );
                }
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => {}
        },
        TypeRef::Function(expected_signature) => match actual {
            TypeRef::Function(actual_signature)
                if expected_signature.params.len() == actual_signature.params.len() =>
            {
                for (expected_param, actual_param) in expected_signature
                    .params
                    .iter()
                    .zip(actual_signature.params.iter())
                {
                    infer_type_bindings(
                        expected_param,
                        actual_param,
                        host_types,
                        generic_names,
                        bindings,
                    );
                }
                infer_type_bindings(
                    expected_signature.ret.as_ref(),
                    actual_signature.ret.as_ref(),
                    host_types,
                    generic_names,
                    bindings,
                );
            }
            TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
                for item in items {
                    infer_type_bindings(expected, item, host_types, generic_names, bindings);
                }
            }
            _ => {}
        },
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                infer_type_bindings(item, actual, host_types, generic_names, bindings);
            }
        }
        _ => {}
    }
}

fn bind_type_parameter(bindings: &mut HashMap<String, TypeRef>, name: &str, actual: TypeRef) {
    let next = match bindings.get(name) {
        Some(current) => join_types(current, &actual),
        None => actual,
    };
    bindings.insert(name.to_owned(), next);
}

fn apply_bindings_to_signature(
    signature: &FunctionTypeRef,
    host_types: &[HostType],
    generic_names: &[String],
    bindings: &HashMap<String, TypeRef>,
) -> FunctionTypeRef {
    FunctionTypeRef {
        params: signature
            .params
            .iter()
            .map(|parameter| apply_bindings_to_type(parameter, host_types, generic_names, bindings))
            .collect(),
        ret: Box::new(apply_bindings_to_type(
            signature.ret.as_ref(),
            host_types,
            generic_names,
            bindings,
        )),
    }
}

fn apply_bindings_to_type(
    ty: &TypeRef,
    host_types: &[HostType],
    generic_names: &[String],
    bindings: &HashMap<String, TypeRef>,
) -> TypeRef {
    match ty {
        TypeRef::Named(name)
            if generic_names.iter().any(|generic| generic == name)
                && is_generic_type_parameter(name, host_types) =>
        {
            bindings.get(name).cloned().unwrap_or_else(|| ty.clone())
        }
        TypeRef::Applied { name, args } => TypeRef::Applied {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| apply_bindings_to_type(arg, host_types, generic_names, bindings))
                .collect(),
        },
        TypeRef::Object(fields) => TypeRef::Object(
            fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        apply_bindings_to_type(value, host_types, generic_names, bindings),
                    )
                })
                .collect(),
        ),
        TypeRef::Array(inner) => TypeRef::Array(Box::new(apply_bindings_to_type(
            inner,
            host_types,
            generic_names,
            bindings,
        ))),
        TypeRef::Map(key, value) => TypeRef::Map(
            Box::new(apply_bindings_to_type(
                key,
                host_types,
                generic_names,
                bindings,
            )),
            Box::new(apply_bindings_to_type(
                value,
                host_types,
                generic_names,
                bindings,
            )),
        ),
        TypeRef::Nullable(inner) => TypeRef::Nullable(Box::new(apply_bindings_to_type(
            inner,
            host_types,
            generic_names,
            bindings,
        ))),
        TypeRef::Union(items) => TypeRef::Union(
            items
                .iter()
                .map(|item| apply_bindings_to_type(item, host_types, generic_names, bindings))
                .collect(),
        ),
        TypeRef::Ambiguous(items) => TypeRef::Ambiguous(
            items
                .iter()
                .map(|item| apply_bindings_to_type(item, host_types, generic_names, bindings))
                .collect(),
        ),
        TypeRef::Function(signature) => TypeRef::Function(apply_bindings_to_signature(
            signature,
            host_types,
            generic_names,
            bindings,
        )),
        _ => ty.clone(),
    }
}

fn is_generic_type_parameter(name: &str, host_types: &[HostType]) -> bool {
    !host_types.iter().any(|host_type| host_type.name == name)
        && !matches!(
            name,
            "bool"
                | "int"
                | "float"
                | "decimal"
                | "string"
                | "char"
                | "blob"
                | "timestamp"
                | "Fn"
                | "FnPtr"
                | "Dynamic"
                | "range"
                | "range="
                | "unknown"
                | "any"
                | "never"
        )
}
