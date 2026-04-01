use rhai_vfs::FileId;

use crate::hints::signature_help::{host_method_candidates_for_type, signature_help};
use rhai_db::{
    builtin_assignment_operator_topic, builtin_binary_operator_topic, builtin_indexer_topic,
    builtin_property_access_topic, builtin_unary_operator_topic, builtin_universal_method_docs,
    builtin_universal_method_signature,
};
use rhai_hir::{SymbolKind, TypeRef};

use crate::support::convert::{format_field_signature, format_symbol_signature, text_size};
use crate::{FilePosition, HoverResult, HoverSignatureSource, SignatureHelp};

pub(crate) fn hover(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    if let Some(field_hover) = hover_for_object_field(snapshot, position) {
        return Some(field_hover);
    }
    if let Some(module_hover) = hover_for_imported_module_member(snapshot, position) {
        return Some(module_hover);
    }
    if let Some(index_hover) = hover_for_builtin_indexer(snapshot, position) {
        return Some(index_hover);
    }
    if let Some(property_hover) = hover_for_builtin_property_access(snapshot, position) {
        return Some(property_hover);
    }
    if let Some(operator_hover) = hover_for_builtin_operator(snapshot, position) {
        return Some(operator_hover);
    }
    if let Some(assignment_hover) = hover_for_builtin_assignment_operator(snapshot, position) {
        return Some(assignment_hover);
    }
    if let Some(unary_hover) = hover_for_builtin_unary_operator(snapshot, position) {
        return Some(unary_hover);
    }

    let symbol_hover = if let Some(target) = snapshot
        .goto_definition(position.file_id, text_size(position.offset))
        .into_iter()
        .next()
    {
        let hir = snapshot.hir(target.file_id)?;
        Some((target.file_id, hir.symbol_at(target.target.full_range)?))
    } else {
        let hir = snapshot.hir(position.file_id)?;
        hir.definition_at_offset(text_size(position.offset))
            .map(|symbol_id| (position.file_id, symbol_id))
    };

    symbol_hover
        .and_then(|(file_id, symbol_id)| hover_for_symbol(snapshot, file_id, symbol_id))
        .or_else(|| hover_for_callable(snapshot, position))
        .or_else(|| hover_for_member_method(snapshot, position))
        .or_else(|| hover_for_builtin(snapshot, position))
}

fn hover_for_builtin_indexer(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let index = hir
        .index_exprs
        .iter()
        .filter(|candidate| hir.expr(candidate.owner).range.contains(offset))
        .min_by_key(|candidate| hir.expr(candidate.owner).range.len())?;

    if index
        .receiver
        .is_some_and(|receiver| hir.expr(receiver).range.contains(offset))
    {
        return None;
    }
    if index
        .index
        .is_some_and(|index_expr| hir.expr(index_expr).range.contains(offset))
    {
        return None;
    }

    let inference = snapshot.type_inference(position.file_id)?;
    let receiver_ty = index
        .receiver
        .and_then(|receiver| hir.expr_type(receiver, &inference.expr_types))?;
    let index_ty = index
        .index
        .and_then(|index_expr| hir.expr_type(index_expr, &inference.expr_types));
    let topic = builtin_indexer_topic(receiver_ty, index_ty)?;

    Some(HoverResult {
        signature: topic.signature.clone(),
        docs: Some(topic.docs),
        source: HoverSignatureSource::Structural,
        declared_signature: Some(topic.signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: topic.notes,
    })
}

fn hover_for_builtin_property_access(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let access = hir
        .member_accesses
        .iter()
        .filter(|candidate| candidate.range.contains(offset))
        .min_by_key(|candidate| candidate.range.len())?;

    if hir.expr(access.receiver).range.contains(offset) {
        return None;
    }
    if hir.reference(access.field_reference).range.contains(offset) {
        return None;
    }

    let inference = snapshot.type_inference(position.file_id)?;
    let receiver_ty = hir.expr_type(access.receiver, &inference.expr_types)?;
    let topic = builtin_property_access_topic(receiver_ty)?;

    Some(HoverResult {
        signature: topic.signature.clone(),
        docs: Some(topic.docs),
        source: HoverSignatureSource::Structural,
        declared_signature: Some(topic.signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: topic.notes,
    })
}

fn hover_for_builtin_operator(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let binary = hir
        .binary_exprs
        .iter()
        .filter(|candidate| {
            candidate
                .operator_range
                .is_some_and(|operator_range| operator_range.contains(offset))
        })
        .min_by_key(|candidate| hir.expr(candidate.owner).range.len())?;

    let inference = snapshot.type_inference(position.file_id)?;
    let lhs_ty = binary
        .lhs
        .and_then(|lhs| hir.expr_type(lhs, &inference.expr_types));
    let rhs_ty = binary
        .rhs
        .and_then(|rhs| hir.expr_type(rhs, &inference.expr_types));
    let topic = builtin_binary_operator_topic(binary.operator, lhs_ty, rhs_ty)?;

    Some(HoverResult {
        signature: topic.signature.clone(),
        docs: Some(topic.docs),
        source: HoverSignatureSource::Structural,
        declared_signature: Some(topic.signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: topic.notes,
    })
}

fn hover_for_builtin_assignment_operator(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let assign = hir
        .assign_exprs
        .iter()
        .filter(|candidate| {
            candidate
                .operator_range
                .is_some_and(|operator_range| operator_range.contains(offset))
        })
        .min_by_key(|candidate| hir.expr(candidate.owner).range.len())?;

    let inference = snapshot.type_inference(position.file_id)?;
    let lhs_ty = assign
        .lhs
        .and_then(|lhs| hir.expr_type(lhs, &inference.expr_types));
    let rhs_ty = assign
        .rhs
        .and_then(|rhs| hir.expr_type(rhs, &inference.expr_types));
    let topic = builtin_assignment_operator_topic(assign.operator, lhs_ty, rhs_ty)?;

    Some(HoverResult {
        signature: topic.signature.clone(),
        docs: Some(topic.docs),
        source: HoverSignatureSource::Structural,
        declared_signature: Some(topic.signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: topic.notes,
    })
}

fn hover_for_builtin_unary_operator(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let unary = hir
        .unary_exprs
        .iter()
        .filter(|candidate| {
            candidate
                .operator_range
                .is_some_and(|operator_range| operator_range.contains(offset))
        })
        .min_by_key(|candidate| hir.expr(candidate.owner).range.len())?;

    let inference = snapshot.type_inference(position.file_id)?;
    let operand_ty = unary
        .operand
        .and_then(|operand| hir.expr_type(operand, &inference.expr_types));
    let topic = builtin_unary_operator_topic(unary.operator, operand_ty)?;

    Some(HoverResult {
        signature: topic.signature.clone(),
        docs: Some(topic.docs),
        source: HoverSignatureSource::Structural,
        declared_signature: Some(topic.signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: topic.notes,
    })
}

fn hover_for_imported_module_member(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let reference_id = hir.reference_at_offset(offset)?;
    let reference = hir.reference(reference_id);
    if reference.target.is_some() {
        return None;
    }

    let expr = hir.expr_at_offset(offset)?;
    let imported = hir.imported_module_path(expr)?;
    let (member_name, module_path) = imported.parts.split_last()?;
    if reference.name != *member_name {
        return None;
    }

    let completion = snapshot
        .imported_module_completions(position.file_id, module_path)
        .into_iter()
        .find(|completion| completion.name == *member_name)?;

    let declared_signature = Some(format_symbol_signature(
        completion.name.as_str(),
        completion.kind,
        completion.annotation.as_ref(),
    ));

    Some(HoverResult {
        signature: declared_signature
            .clone()
            .unwrap_or_else(|| completion.name.clone()),
        docs: completion.docs,
        source: HoverSignatureSource::Declared,
        declared_signature,
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: vec![
            "Imported module member is provided by workspace or external module metadata."
                .to_owned(),
        ],
    })
}

fn hover_for_object_field(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let field = snapshot.object_field_hover(position.file_id, text_size(position.offset))?;
    let declared_signature = field
        .declared_annotation
        .as_ref()
        .map(|annotation| format_field_signature(field.name.as_str(), Some(annotation)));
    let inferred_signature = field
        .inferred_annotation
        .as_ref()
        .filter(|annotation| !matches!(annotation, TypeRef::Unknown))
        .map(|annotation| format_field_signature(field.name.as_str(), Some(annotation)));

    let (signature, source) = if let Some(signature) = declared_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Declared)
    } else if let Some(signature) = inferred_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Inferred)
    } else {
        (
            format_field_signature(field.name.as_str(), None),
            HoverSignatureSource::Structural,
        )
    };

    let mut notes = Vec::new();
    if declared_signature.is_none() && inferred_signature.is_some() {
        notes.push(
            "Field type is inferred from structural object flows and object literal analysis."
                .to_owned(),
        );
    }
    if let (Some(declared), Some(inferred)) =
        (declared_signature.as_ref(), inferred_signature.as_ref())
        && declared != inferred
    {
        notes.push(format!("Inferred type: {inferred}"));
    }
    Some(HoverResult {
        signature,
        docs: field.docs,
        source,
        declared_signature,
        inferred_signature,
        overload_signatures: Vec::new(),
        notes,
    })
}

pub(crate) fn hover_for_symbol(
    snapshot: &rhai_db::DatabaseSnapshot,
    file_id: FileId,
    symbol_id: rhai_hir::SymbolId,
) -> Option<HoverResult> {
    let hir = snapshot.hir(file_id)?;
    let symbol = hir.symbol(symbol_id);
    if symbol.kind == SymbolKind::ImportAlias
        && let Some(hover) = hover_for_import_alias(snapshot, file_id, hir.as_ref(), symbol_id)
    {
        return Some(hover);
    }
    if symbol.kind == SymbolKind::ExportAlias
        && let Some(hover) = hover_for_export_alias(snapshot, file_id, hir.as_ref(), symbol_id)
    {
        return Some(hover);
    }
    let docs = symbol.docs.map(|docs| hir.doc_block(docs).text.clone());
    let declared_signature = symbol
        .annotation
        .as_ref()
        .filter(|annotation| !matches!(annotation, TypeRef::Unknown))
        .map(|annotation| {
            format_symbol_signature(symbol.name.as_str(), symbol.kind, Some(annotation))
        });
    let inferred_annotation = snapshot
        .inferred_symbol_type(file_id, symbol_id)
        .filter(|annotation| !matches!(annotation, TypeRef::Unknown));
    let inferred_signature = inferred_annotation.map(|annotation| {
        format_symbol_signature(symbol.name.as_str(), symbol.kind, Some(annotation))
    });

    let (signature, source) = if let Some(signature) = declared_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Declared)
    } else if let Some(signature) = inferred_signature.as_ref() {
        (signature.clone(), HoverSignatureSource::Inferred)
    } else {
        (
            format_symbol_signature(symbol.name.as_str(), symbol.kind, None),
            HoverSignatureSource::Structural,
        )
    };

    let mut notes = Vec::new();
    if declared_signature.is_none() && inferred_signature.is_some() {
        notes
            .push("Signature shown is inferred from local flow and call-site analysis.".to_owned());
    }
    if let (Some(declared), Some(inferred)) =
        (declared_signature.as_ref(), inferred_signature.as_ref())
        && declared != inferred
    {
        notes.push(format!("Inferred type: {inferred}"));
    }
    if matches!(inferred_annotation, Some(TypeRef::Ambiguous(_))) {
        notes.push("Multiple call candidates remain viable at this location.".to_owned());
    }

    Some(HoverResult {
        signature,
        docs,
        source,
        declared_signature,
        inferred_signature,
        overload_signatures: Vec::new(),
        notes,
    })
}

fn hover_for_import_alias(
    snapshot: &rhai_db::DatabaseSnapshot,
    file_id: FileId,
    hir: &rhai_hir::FileHir,
    symbol_id: rhai_hir::SymbolId,
) -> Option<HoverResult> {
    let import_index = hir
        .imports
        .iter()
        .position(|import| import.alias == Some(symbol_id))?;
    let import = hir.import(import_index);
    let alias = hir.symbol(symbol_id);
    let module_text = import.module_text.as_deref().unwrap_or("<dynamic module>");
    let signature = format!("import {module_text} as {}", alias.name);
    let mut docs = None;
    let mut notes = Vec::new();

    if let Some(module_name) = parse_import_module_name(module_text) {
        if let Some(module) = snapshot
            .host_modules()
            .iter()
            .find(|module| module.name == module_name)
        {
            docs = module.docs.clone();
            notes.push("Resolved from host module metadata.".to_owned());
            let member_count = module.functions.len() + module.constants.len();
            if member_count > 0 {
                notes.push(format!(
                    "Module exposes {member_count} member{}.",
                    if member_count == 1 { "" } else { "s" }
                ));
            }
        } else if let Some(linked_import) = snapshot.linked_import(file_id, import_index) {
            if let Some(path) = snapshot.normalized_path(linked_import.provider_file_id) {
                notes.push(format!("Linked workspace module: {}", path.display()));
            } else {
                notes.push("Linked to a workspace module file.".to_owned());
            }
        } else if snapshot
            .comment_directives(file_id)
            .is_some_and(|directives| directives.external_modules.contains(module_name.as_str()))
        {
            notes.push("Resolved from inline external module directives.".to_owned());
        }
    }

    Some(HoverResult {
        signature: signature.clone(),
        docs,
        source: HoverSignatureSource::Declared,
        declared_signature: Some(signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes,
    })
}

fn hover_for_export_alias(
    _snapshot: &rhai_db::DatabaseSnapshot,
    _file_id: FileId,
    hir: &rhai_hir::FileHir,
    symbol_id: rhai_hir::SymbolId,
) -> Option<HoverResult> {
    let export = hir
        .exports
        .iter()
        .find(|export| export.alias == Some(symbol_id))?;
    let alias = hir.symbol(symbol_id);
    let target_text = export.target_text.as_deref().unwrap_or("<export target>");
    let signature = format!("export {target_text} as {}", alias.name);
    let docs = alias.docs.map(|docs| hir.doc_block(docs).text.clone());
    let mut notes = Vec::new();

    if let Some(target_symbol) = export.target_symbol.or_else(|| {
        export
            .target_reference
            .and_then(|reference| hir.definition_of(reference))
    }) {
        let target = hir.symbol(target_symbol);
        let target_signature = format_symbol_signature(
            target.name.as_str(),
            target.kind,
            target.annotation.as_ref(),
        );
        notes.push(format!("Re-exports: {target_signature}"));
    } else {
        notes.push("Re-exports a module-level symbol through this alias.".to_owned());
    }

    if hir.scope(alias.scope).kind == rhai_hir::ScopeKind::File {
        notes.push("Export alias is visible to importing modules.".to_owned());
    }

    Some(HoverResult {
        signature: signature.clone(),
        docs,
        source: HoverSignatureSource::Declared,
        declared_signature: Some(signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes,
    })
}

fn hover_for_builtin(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let reference_id = hir.reference_at_offset(text_size(position.offset))?;
    let reference = hir.reference(reference_id);
    if reference.target.is_some() || !matches!(reference.kind, rhai_hir::ReferenceKind::Name) {
        return None;
    }

    let function = snapshot.global_function(reference.name.as_str())?;
    let declared_signature = builtin_function_signature(function);
    let signature = declared_signature.clone().unwrap_or_else(|| {
        format_symbol_signature(function.name.as_str(), SymbolKind::Function, None)
    });
    let mut notes = Vec::new();
    if function.overloads.len() > 1 {
        notes.push(format!(
            "{} overloads are available for this builtin function.",
            function.overloads.len()
        ));
    }
    let overload_signatures = function
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .map(|signature| {
            format_symbol_signature(
                function.name.as_str(),
                SymbolKind::Function,
                Some(&TypeRef::Function(signature.clone())),
            )
        })
        .filter(|label| label != &signature)
        .collect();

    Some(HoverResult {
        signature,
        docs: builtin_function_docs(function),
        source: HoverSignatureSource::Declared,
        declared_signature,
        inferred_signature: None,
        overload_signatures,
        notes,
    })
}

fn hover_for_callable(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let call = hir
        .call_at_offset(offset)
        .map(|call_id| hir.call(call_id))?;
    if !call
        .callee_range
        .is_some_and(|range| range.contains(offset))
    {
        return None;
    }

    let help = signature_help(snapshot, position.file_id, offset)?;
    let method_docs = call
        .callee_range
        .and_then(|range| hir.expr_at(range))
        .and_then(|expr| hir.member_access(expr))
        .map(|access| hir.reference(access.field_reference).name.as_str())
        .and_then(builtin_universal_method_docs);
    hover_from_signature_help(help, method_docs)
}

fn hover_for_member_method(
    snapshot: &rhai_db::DatabaseSnapshot,
    position: FilePosition,
) -> Option<HoverResult> {
    let hir = snapshot.hir(position.file_id)?;
    let offset = text_size(position.offset);
    let reference_id = hir.reference_at_offset(offset)?;
    let reference = hir.reference(reference_id);
    if reference.target.is_some() {
        return None;
    }

    let expr = hir.expr_at_offset(offset)?;
    let access = hir.member_access(expr)?;
    if access.field_reference != reference_id {
        return None;
    }

    let inference = snapshot.type_inference(position.file_id)?;
    let receiver_ty = hir.expr_type(access.receiver, &inference.expr_types)?;
    let method_name = reference.name.as_str();

    let candidates =
        host_method_candidates_for_type(snapshot.host_types(), receiver_ty, method_name, &[]);
    if !candidates.is_empty() {
        let active = candidates
            .iter()
            .position(|candidate| candidate.docs.is_some())
            .unwrap_or(0);
        let selected = &candidates[active];
        let mut notes = Vec::new();
        if candidates.len() > 1 {
            notes.push(format!(
                "{} overloads are available for this method.",
                candidates.len()
            ));
        }

        let signature = format_symbol_signature(
            method_name,
            SymbolKind::Function,
            Some(&TypeRef::Function(selected.signature.clone())),
        );
        let overload_signatures = candidates
            .iter()
            .map(|candidate| {
                format_symbol_signature(
                    method_name,
                    SymbolKind::Function,
                    Some(&TypeRef::Function(candidate.signature.clone())),
                )
            })
            .filter(|label| label != &signature)
            .collect();
        return Some(HoverResult {
            signature: signature.clone(),
            docs: selected.docs.clone(),
            source: HoverSignatureSource::Declared,
            declared_signature: Some(signature),
            inferred_signature: None,
            overload_signatures,
            notes,
        });
    }

    let signature = builtin_universal_method_signature(method_name)?;
    let declared_signature = format_symbol_signature(
        method_name,
        SymbolKind::Function,
        Some(&TypeRef::Function(signature)),
    );
    Some(HoverResult {
        signature: declared_signature.clone(),
        docs: builtin_universal_method_docs(method_name),
        source: HoverSignatureSource::Declared,
        declared_signature: Some(declared_signature),
        inferred_signature: None,
        overload_signatures: Vec::new(),
        notes: vec!["Builtin universal Rhai method available on any value.".to_owned()],
    })
}

fn hover_from_signature_help(
    help: SignatureHelp,
    docs_fallback: Option<String>,
) -> Option<HoverResult> {
    let signature = help
        .signatures
        .get(help.active_signature)
        .or_else(|| help.signatures.first())?;
    let mut notes = Vec::new();
    if help.signatures.len() > 1 {
        notes.push(format!(
            "{} overloads are available for this callable.",
            help.signatures.len()
        ));
    }

    Some(HoverResult {
        signature: signature.label.clone(),
        docs: signature.docs.clone().or(docs_fallback),
        source: HoverSignatureSource::Declared,
        declared_signature: Some(signature.label.clone()),
        inferred_signature: None,
        overload_signatures: help
            .signatures
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != help.active_signature)
            .map(|(_, signature)| signature.label.clone())
            .collect(),
        notes,
    })
}

fn builtin_function_signature(function: &rhai_db::HostFunction) -> Option<String> {
    if function.overloads.len() != 1 {
        return None;
    }

    function
        .overloads
        .first()
        .and_then(|overload| overload.signature.as_ref())
        .map(|signature| {
            format_symbol_signature(
                function.name.as_str(),
                SymbolKind::Function,
                Some(&TypeRef::Function(signature.clone())),
            )
        })
}

fn builtin_function_docs(function: &rhai_db::HostFunction) -> Option<String> {
    let mut docs = function
        .overloads
        .iter()
        .filter_map(|overload| overload.docs.as_deref())
        .map(str::trim)
        .filter(|docs| !docs.is_empty())
        .collect::<Vec<_>>();
    docs.sort_unstable();
    docs.dedup();
    (!docs.is_empty()).then(|| docs.join("\n\n"))
}

fn parse_import_module_name(module_text: &str) -> Option<String> {
    module_text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .map(str::to_owned)
        .or_else(|| {
            module_text
                .strip_prefix('`')
                .and_then(|text| text.strip_suffix('`'))
                .map(str::to_owned)
        })
}
