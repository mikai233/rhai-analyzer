use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, TypeRef, parse_type_ref};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig, TypeSpec};

use crate::types::{
    HostConstant, HostFunction, HostFunctionOverload, HostModule, HostType, ProjectSemantics,
};

pub(crate) fn build_project_semantics(project: &ProjectConfig) -> ProjectSemantics {
    let mut external_signatures = ExternalSignatureIndex::default();
    let global_functions = builtin_global_functions(&mut external_signatures);
    let mut modules = Vec::new();
    let mut types = Vec::new();

    for (name, module) in &project.modules {
        modules.push(build_host_module(name, module, &mut external_signatures));
    }

    for (name, ty) in &project.types {
        types.push(build_host_type(name, ty, &mut external_signatures));
    }

    ProjectSemantics {
        external_signatures,
        global_functions,
        modules,
        types,
        disabled_symbols: project.engine.disabled_symbols.clone(),
        custom_syntaxes: project.engine.custom_syntaxes.clone(),
    }
}

fn builtin_global_functions(external_signatures: &mut ExternalSignatureIndex) -> Vec<HostFunction> {
    let blob_return = TypeRef::Blob;
    external_signatures.insert(
        "blob",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(blob_return.clone()),
        }),
    );
    external_signatures.insert(
        "timestamp",
        TypeRef::Function(FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Timestamp),
        }),
    );
    external_signatures.insert(
        "Fn",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::FnPtr),
        }),
    );
    external_signatures.insert(
        "is_def_var",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String],
            ret: Box::new(TypeRef::Bool),
        }),
    );
    external_signatures.insert(
        "is_def_fn",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::String, TypeRef::Int],
            ret: Box::new(TypeRef::Bool),
        }),
    );
    external_signatures.insert(
        "type_of",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Any],
            ret: Box::new(TypeRef::String),
        }),
    );

    vec![
        HostFunction {
            name: "blob".to_owned(),
            overloads: vec![
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: Vec::new(),
                        ret: Box::new(blob_return.clone()),
                    }),
                    docs: Some("Create an empty BLOB.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::Int],
                        ret: Box::new(blob_return.clone()),
                    }),
                    docs: Some("Create a BLOB with the given length.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::Int, TypeRef::Int],
                        ret: Box::new(blob_return),
                    }),
                    docs: Some("Create a BLOB filled with the given byte value.".to_owned()),
                },
            ],
        },
        HostFunction {
            name: "timestamp".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: Vec::new(),
                    ret: Box::new(TypeRef::Timestamp),
                }),
                docs: Some("Create a timestamp for the current instant.".to_owned()),
            }],
        },
        HostFunction {
            name: "Fn".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::FnPtr),
                }),
                docs: Some("Create a function pointer from a function name.".to_owned()),
            }],
        },
        HostFunction {
            name: "is_def_var".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::String],
                    ret: Box::new(TypeRef::Bool),
                }),
                docs: Some("Check whether a variable is defined.".to_owned()),
            }],
        },
        HostFunction {
            name: "is_def_fn".to_owned(),
            overloads: vec![
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Bool),
                    }),
                    docs: Some("Check whether a function is defined.".to_owned()),
                },
                HostFunctionOverload {
                    signature: Some(FunctionTypeRef {
                        params: vec![TypeRef::String, TypeRef::String, TypeRef::Int],
                        ret: Box::new(TypeRef::Bool),
                    }),
                    docs: Some("Check whether a typed method is defined.".to_owned()),
                },
            ],
        },
        HostFunction {
            name: "type_of".to_owned(),
            overloads: vec![HostFunctionOverload {
                signature: Some(FunctionTypeRef {
                    params: vec![TypeRef::Any],
                    ret: Box::new(TypeRef::String),
                }),
                docs: Some("Get the type name of a value.".to_owned()),
            }],
        },
    ]
}

fn build_host_module(
    module_name: &str,
    module: &ModuleSpec,
    external_signatures: &mut ExternalSignatureIndex,
) -> HostModule {
    let mut functions = Vec::new();
    let mut constants = Vec::new();

    for (name, specs) in &module.functions {
        functions.push(build_host_function(name, specs));

        let parsed = parsed_host_overloads(specs);
        if parsed.len() == 1 {
            external_signatures.insert(
                format!("{module_name}::{name}"),
                TypeRef::Function(parsed[0].clone()),
            );
        }
    }

    for (name, constant) in &module.constants {
        let ty = parse_type_ref(&constant.type_name);
        if let Some(parsed) = ty.clone() {
            external_signatures.insert(format!("{module_name}::{name}"), parsed);
        }

        constants.push(HostConstant {
            name: name.clone(),
            ty,
            docs: constant.docs.clone(),
        });
    }

    HostModule {
        name: module_name.to_owned(),
        docs: module.docs.clone(),
        functions,
        constants,
    }
}

fn build_host_type(
    type_name: &str,
    ty: &TypeSpec,
    external_signatures: &mut ExternalSignatureIndex,
) -> HostType {
    let mut methods = Vec::new();

    for (name, specs) in &ty.methods {
        methods.push(build_host_function(name, specs));

        let parsed = parsed_host_overloads(specs);
        if parsed.len() == 1 {
            external_signatures.insert(
                format!("{type_name}::{name}"),
                TypeRef::Function(parsed[0].clone()),
            );
        }
    }

    HostType {
        name: type_name.to_owned(),
        docs: ty.docs.clone(),
        methods,
    }
}

fn build_host_function(name: &str, specs: &[FunctionSpec]) -> HostFunction {
    HostFunction {
        name: name.to_owned(),
        overloads: specs
            .iter()
            .map(|spec| HostFunctionOverload {
                signature: parse_function_spec(spec),
                docs: spec.docs.clone(),
            })
            .collect(),
    }
}

fn parsed_host_overloads(specs: &[FunctionSpec]) -> Vec<FunctionTypeRef> {
    specs.iter().filter_map(parse_function_spec).collect()
}

fn parse_function_spec(spec: &FunctionSpec) -> Option<FunctionTypeRef> {
    let mut signature = match parse_type_ref(&spec.signature)? {
        TypeRef::Function(signature) => signature,
        _ => return None,
    };

    if let Some(return_type) = spec.return_type.as_deref().and_then(parse_type_ref) {
        signature.ret = Box::new(return_type);
    }

    Some(signature)
}
