use crate::builtin::signatures::{builtin_host_types, register_builtin_global_functions};
use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, TypeRef, parse_type_ref};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig, TypeSpec};

use crate::types::{
    HostConstant, HostFunction, HostFunctionOverload, HostModule, HostType, ProjectSemantics,
};

pub(crate) fn build_project_semantics(project: &ProjectConfig) -> ProjectSemantics {
    let mut external_signatures = ExternalSignatureIndex::default();
    let global_functions = register_builtin_global_functions(&mut external_signatures);
    let mut modules = Vec::new();
    let mut types = builtin_host_types();

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
    let (base_name, generic_params) = parse_host_type_declaration(type_name);
    let mut methods = Vec::new();

    for (name, specs) in &ty.methods {
        methods.push(build_host_function(name, specs));

        let parsed = parsed_host_overloads(specs);
        if parsed.len() == 1 {
            external_signatures.insert(
                format!("{base_name}::{name}"),
                TypeRef::Function(parsed[0].clone()),
            );
        }
    }

    HostType {
        name: base_name,
        generic_params,
        docs: ty.docs.clone(),
        methods,
    }
}

fn parse_host_type_declaration(type_name: &str) -> (String, Vec<String>) {
    match parse_type_ref(type_name) {
        Some(TypeRef::Named(name)) => (name, Vec::new()),
        Some(TypeRef::Applied { name, args }) => {
            let generic_params = args
                .into_iter()
                .filter_map(|arg| match arg {
                    TypeRef::Named(param) => Some(param),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (name, generic_params)
        }
        _ => (type_name.to_owned(), Vec::new()),
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
