use std::collections::HashMap;

use rhai_hir::{ExternalSignatureIndex, FileHir, FunctionTypeRef, SymbolId, TypeRef};

use crate::{FileTypeInference, HostFunction, HostType};

pub(crate) mod calls;
mod exprs;
pub(crate) mod generics;
mod helpers;
mod loops;
mod objects;
mod propagation;

pub(crate) use crate::infer::calls::{
    callable_targets_for_call, join_callable_target_signatures, named_callable_targets_at_offset,
};
pub(crate) use crate::infer::helpers::{ReadTargetKey, read_target_key_for_expr};
pub(crate) use crate::infer::objects::{
    field_value_exprs_from_expr, field_value_exprs_from_symbol, largest_inner_expr,
    string_literal_value,
};

use crate::infer::exprs::infer_expr_types;
use crate::infer::propagation::{
    infer_function_signatures, merge_symbol_type, propagate_call_argument_types,
    propagate_expected_types, propagate_for_binding_types, propagate_symbol_mutations,
    propagate_value_flows,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedMethodSignature {
    pub name: String,
    pub receiver: TypeRef,
    pub signature: FunctionTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedModuleMember {
    pub module_path: Vec<String>,
    pub name: String,
    pub ty: TypeRef,
}

pub(crate) fn infer_file_types(
    hir: &FileHir,
    external: &ExternalSignatureIndex,
    globals: &[HostFunction],
    host_types: &[HostType],
    imported_methods: &[ImportedMethodSignature],
    imported_members: &[ImportedModuleMember],
    seed_symbol_types: &HashMap<SymbolId, TypeRef>,
) -> FileTypeInference {
    let mut inference = FileTypeInference {
        expr_types: hir.new_type_slot_assignments(),
        symbol_types: HashMap::new(),
    };

    for (&symbol, ty) in seed_symbol_types {
        merge_symbol_type(&mut inference, symbol, ty.clone());
    }

    for (index, symbol) in hir.symbols.iter().enumerate() {
        if let Some(annotation) = symbol.annotation.clone() {
            merge_symbol_type(&mut inference, SymbolId(index as u32), annotation);
        }
    }

    let max_iterations =
        hir.exprs.len() + hir.symbols.len() + hir.calls.len() + hir.bodies.len() + 1;
    for _ in 0..max_iterations.max(1) {
        let mut changed = false;

        changed |= infer_expr_types(
            hir,
            external,
            globals,
            host_types,
            imported_methods,
            imported_members,
            &mut inference,
        );
        changed |= propagate_for_binding_types(hir, &mut inference);
        changed |= propagate_expected_types(
            hir,
            external,
            globals,
            host_types,
            imported_methods,
            &mut inference,
        );
        changed |= propagate_call_argument_types(hir, imported_methods, &mut inference);
        changed |= propagate_value_flows(hir, &mut inference);
        changed |= propagate_symbol_mutations(hir, &mut inference);
        changed |= infer_function_signatures(hir, &mut inference);

        if !changed {
            break;
        }
    }

    inference
}
