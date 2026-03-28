use rhai_hir::{ExternalSignatureIndex, FunctionTypeRef, TypeRef, parse_type_ref};
use rhai_project::{FunctionSpec, ModuleSpec, ProjectConfig, TypeSpec};

use crate::types::{
    HostConstant, HostFunction, HostFunctionOverload, HostModule, HostType, ProjectSemantics,
};

pub(crate) fn build_project_semantics(project: &ProjectConfig) -> ProjectSemantics {
    let mut external_signatures = ExternalSignatureIndex::default();
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
