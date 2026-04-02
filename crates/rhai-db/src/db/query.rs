use std::collections::{BTreeMap, HashSet};

use crate::builtin::semantic_keys::{BuiltinSemanticKey, builtin_property_access_semantic_key};
use crate::builtin::signatures::{
    builtin_universal_method_names, builtin_universal_method_signature, host_type_name_for_type,
};
use crate::builtin::topics::builtin_property_access_topic;
use crate::db::DatabaseSnapshot;
use crate::db::query_support::{object_field_member_completions, symbol_for_expr};
use crate::db::rebuild::default_file_stats;
use crate::infer::calls::merge_function_candidate_signatures;
use crate::infer::field_value_exprs_from_expr;
use crate::infer::generics::specialize_signature_with_receiver_and_arg_types;
use crate::types::{
    AutoImportCandidate, CachedFileAnalysis, CompletionInputs, DatabaseDebugView, DebugFileAnalysis,
};
use rhai_hir::{CompletionSymbol, ExprId, MemberCompletion, MemberCompletionSource, TypeRef};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;

impl DatabaseSnapshot {
    pub fn completion_inputs(&self, file_id: FileId, offset: TextSize) -> Option<CompletionInputs> {
        let analysis = self.analysis.get(&file_id)?;
        let visible_symbols = visible_completion_symbols(analysis, offset);
        let member_symbols = cached_member_completion_at(self, file_id, analysis, offset);
        let visible_names = visible_symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<HashSet<_>>();
        let project_symbols = self
            .workspace_symbols
            .iter()
            .filter(|symbol| {
                symbol.symbol.exported && !visible_names.contains(symbol.symbol.name.as_str())
            })
            .cloned()
            .collect();

        Some(CompletionInputs {
            file_id,
            offset,
            visible_symbols,
            project_symbols,
            member_symbols,
        })
    }

    pub fn auto_import_candidates(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<AutoImportCandidate> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };
        let Some(reference_id) = analysis.hir.reference_at_offset(offset) else {
            return Vec::new();
        };
        let reference = analysis.hir.reference(reference_id);
        if reference.target.is_some() || reference.kind != rhai_hir::ReferenceKind::Name {
            return Vec::new();
        }

        self.auto_import_candidates_for_name(file_id, reference.name.as_str())
    }

    pub fn auto_import_candidates_for_name(
        &self,
        file_id: FileId,
        name: &str,
    ) -> Vec<AutoImportCandidate> {
        let _ = (file_id, name);
        Vec::new()
    }

    pub fn debug_view(&self) -> DatabaseDebugView {
        let mut files = self
            .analysis
            .iter()
            .map(|(&file_id, analysis)| {
                let dependencies = analysis.dependencies.as_ref().clone();
                DebugFileAnalysis {
                    file_id,
                    normalized_path: dependencies.normalized_path.clone(),
                    document_version: dependencies.document_version,
                    source_root: dependencies.source_root,
                    is_workspace_file: dependencies.is_workspace_file,
                    dependencies,
                    stats: self
                        .file_stats
                        .get(&file_id)
                        .cloned()
                        .unwrap_or_else(|| default_file_stats(file_id)),
                }
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.file_id.0.cmp(&right.file_id.0));

        DatabaseDebugView {
            revision: self.revision,
            project_revision: self.project_revision,
            source_roots: self.source_root_paths(),
            files,
            stats: (*self.stats).clone(),
        }
    }
}

fn visible_completion_symbols(
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<CompletionSymbol> {
    let Some(query_support) = analysis.query_support.as_ref() else {
        return analysis.hir.completion_symbols_at(offset);
    };

    analysis
        .hir
        .visible_symbols_at(offset)
        .into_iter()
        .filter_map(|symbol| {
            query_support
                .completion_symbols_by_symbol
                .get(&symbol)
                .cloned()
        })
        .collect()
}

fn cached_member_completion_at(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<MemberCompletion> {
    let access = analysis
        .hir
        .member_accesses
        .iter()
        .filter(|access| {
            access.range.contains(offset)
                || analysis
                    .hir
                    .reference(access.field_reference)
                    .range
                    .contains(offset)
        })
        .min_by_key(|access| access.range.len());

    if let Some(access) = access {
        let mut members = BTreeMap::<String, MemberCompletion>::new();
        for member in object_field_member_completions(&analysis.hir, access.receiver) {
            members.entry(member.name.clone()).or_insert(member);
        }

        if let Some(query_support) = analysis.query_support.as_ref()
            && let Some(symbol) = symbol_for_expr(&analysis.hir, access.receiver)
            && let Some(cached) = query_support.member_completion_sets_by_symbol.get(&symbol)
        {
            for member in cached.iter().cloned() {
                members.entry(member.name.clone()).or_insert(member);
            }
        } else {
            for member in analysis.hir.member_completions_for_expr(access.receiver) {
                members.entry(member.name.clone()).or_insert(member);
            }
        }

        enrich_member_completions_with_inference(&mut members, analysis, access.receiver);

        for member in host_type_member_completions(snapshot, file_id, analysis, access.receiver) {
            if should_skip_ambiguous_host_member(&members, &member, analysis, access.receiver) {
                continue;
            }
            merge_member_completion(&mut members, member);
        }

        return members.into_values().collect();
    }

    fallback_member_completion_at(snapshot, file_id, analysis, offset)
}

fn host_type_member_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    analysis: &CachedFileAnalysis,
    receiver: rhai_hir::ExprId,
) -> Vec<MemberCompletion> {
    let Some(receiver_ty) = receiver_type_for_member_completion(analysis, receiver).or_else(|| {
        snapshot
            .inferred_expr_type_at(file_id, analysis.hir.expr(receiver).range.start())
            .cloned()
    }) else {
        return Vec::new();
    };

    let mut members = BTreeMap::<String, MemberCompletion>::new();
    collect_host_type_member_completions(&mut members, snapshot.host_types(), &receiver_ty);
    members.into_values().collect()
}

fn fallback_member_completion_at(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<MemberCompletion> {
    let mut members = BTreeMap::<String, MemberCompletion>::new();
    if let Some(receiver_name) = incomplete_member_receiver_name(snapshot, file_id, offset)
        && let Some(symbol) = analysis
            .hir
            .visible_symbols_at(offset)
            .into_iter()
            .rev()
            .find(|symbol| analysis.hir.symbol(*symbol).name == receiver_name)
    {
        if let Some(query_support) = analysis.query_support.as_ref()
            && let Some(cached) = query_support.member_completion_sets_by_symbol.get(&symbol)
        {
            for member in cached.iter().cloned() {
                members.entry(member.name.clone()).or_insert(member);
            }
        }

        if let Some(receiver_ty) = snapshot.inferred_symbol_type(file_id, symbol).cloned() {
            enrich_member_completions_with_type(&mut members, &receiver_ty);
            let mut host_members = BTreeMap::<String, MemberCompletion>::new();
            collect_host_type_member_completions(
                &mut host_members,
                snapshot.host_types(),
                &receiver_ty,
            );
            for member in host_members.into_values() {
                if should_skip_ambiguous_host_member_for_type(&members, &member, &receiver_ty) {
                    continue;
                }
                merge_member_completion(&mut members, member);
            }
        }

        return members.into_values().collect();
    }

    let Some(receiver_expr) = incomplete_member_receiver_expr(snapshot, file_id, analysis, offset)
    else {
        return Vec::new();
    };

    for member in object_field_member_completions(&analysis.hir, receiver_expr) {
        members.entry(member.name.clone()).or_insert(member);
    }

    for member in analysis.hir.member_completions_for_expr(receiver_expr) {
        members.entry(member.name.clone()).or_insert(member);
    }

    enrich_member_completions_with_inference(&mut members, analysis, receiver_expr);

    for member in host_type_member_completions(snapshot, file_id, analysis, receiver_expr) {
        if should_skip_ambiguous_host_member(&members, &member, analysis, receiver_expr) {
            continue;
        }
        merge_member_completion(&mut members, member);
    }

    members.into_values().collect()
}

fn incomplete_member_receiver_name(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    offset: TextSize,
) -> Option<String> {
    let text = snapshot.file_text(file_id)?;
    let offset = clamp_to_char_boundary(
        text.as_ref(),
        usize::try_from(u32::from(offset)).ok()?.min(text.len()),
    );
    let bytes = text.as_bytes();
    let mut prefix_start = offset;

    while prefix_start > 0 && is_identifier_byte(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    if prefix_start == 0 || bytes[prefix_start - 1] != b'.' {
        return None;
    }

    let mut receiver_end = prefix_start - 1;
    while receiver_end > 0 && bytes[receiver_end - 1].is_ascii_whitespace() {
        receiver_end -= 1;
    }
    let mut receiver_start = receiver_end;
    while receiver_start > 0 && is_identifier_byte(bytes[receiver_start - 1]) {
        receiver_start -= 1;
    }
    if receiver_start == receiver_end {
        return None;
    }

    text.get(receiver_start..receiver_end).map(str::to_owned)
}

fn incomplete_member_receiver_expr(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Option<rhai_hir::ExprId> {
    let text = snapshot.file_text(file_id)?;
    let offset = clamp_to_char_boundary(
        text.as_ref(),
        usize::try_from(u32::from(offset)).ok()?.min(text.len()),
    );
    if offset == 0 || text.as_bytes().get(offset - 1).copied() != Some(b'.') {
        return None;
    }

    let dot_offset = TextSize::from((offset - 1) as u32);
    analysis
        .hir
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(index, expr)| {
            (expr.range.end() == dot_offset)
                .then_some((rhai_hir::ExprId(index as u32), expr.range.len()))
        })
        .max_by_key(|(_, len)| *len)
        .map(|(expr, _)| expr)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn clamp_to_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn collect_host_type_member_completions(
    members: &mut BTreeMap<String, MemberCompletion>,
    host_types: &[crate::HostType],
    ty: &TypeRef,
) {
    match ty {
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            for item in items {
                collect_host_type_member_completions(members, host_types, item);
            }
        }
        TypeRef::Named(name) | TypeRef::Applied { name, .. } => {
            if let Some(host_type) = host_types.iter().find(|host_type| host_type.name == *name) {
                add_host_type_members(members, ty, host_type, host_types);
            }
        }
        _ => {
            let Some(host_type_name) = host_type_name_for_type(ty) else {
                add_builtin_property_members(members, ty);
                add_universal_method_members(members);
                return;
            };
            let Some(host_type) = host_types
                .iter()
                .find(|host_type| host_type.name == host_type_name)
            else {
                add_universal_method_members(members);
                return;
            };
            add_host_type_members(members, ty, host_type, host_types);
        }
    }

    add_builtin_property_members(members, ty);
    add_universal_method_members(members);
}

fn add_host_type_members(
    members: &mut BTreeMap<String, MemberCompletion>,
    receiver_ty: &TypeRef,
    host_type: &crate::HostType,
    host_types: &[crate::HostType],
) {
    for method in &host_type.methods {
        members
            .entry(method.name.clone())
            .or_insert(MemberCompletion {
                name: method.name.clone(),
                annotation: method_signature_annotation(receiver_ty, host_type, method, host_types),
                docs: method_docs(method),
                range: None,
                source: MemberCompletionSource::HostTypeMember,
            });
    }
}

fn method_signature_annotation(
    receiver_ty: &TypeRef,
    host_type: &crate::HostType,
    method: &crate::HostFunction,
    host_types: &[crate::HostType],
) -> Option<TypeRef> {
    let signatures = method
        .overloads
        .iter()
        .filter_map(|overload| overload.signature.as_ref())
        .map(|signature| {
            specialize_signature_with_receiver_and_arg_types(
                signature,
                Some(receiver_ty),
                host_type.generic_params.as_slice(),
                None,
                host_types,
            )
        })
        .collect::<Vec<_>>();
    merge_function_candidate_signatures(signatures, None).map(TypeRef::Function)
}

fn add_universal_method_members(members: &mut BTreeMap<String, MemberCompletion>) {
    for method_name in builtin_universal_method_names() {
        members
            .entry((*method_name).to_owned())
            .or_insert(MemberCompletion {
                name: (*method_name).to_owned(),
                annotation: builtin_universal_method_signature(method_name).map(TypeRef::Function),
                docs: crate::builtin::signatures::builtin_universal_method_docs(method_name),
                range: None,
                source: MemberCompletionSource::HostTypeMember,
            });
    }
}

fn add_builtin_property_members(members: &mut BTreeMap<String, MemberCompletion>, ty: &TypeRef) {
    if builtin_property_access_semantic_key(ty, "tag")
        != Some(BuiltinSemanticKey::DynamicTagPropertyAccess)
    {
        return;
    }

    let Some(topic) = builtin_property_access_topic(ty, "tag") else {
        return;
    };

    match members.entry(String::from("tag")) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(MemberCompletion {
                name: String::from("tag"),
                annotation: Some(TypeRef::Int),
                docs: Some(topic.docs),
                range: None,
                source: MemberCompletionSource::HostTypeMember,
            });
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let member = entry.get_mut();
            member.annotation = merge_member_annotation(member.annotation.as_ref(), &TypeRef::Int);
            member.docs = match member.docs.take() {
                Some(existing)
                    if existing.contains("dynamic value tag")
                        || existing.contains("object map is ambiguous") =>
                {
                    Some(existing)
                }
                Some(existing) => Some(format!("{existing}\n\n---\n\n{}", topic.docs)),
                None => Some(topic.docs),
            };
        }
    }
}

fn merge_member_annotation(current: Option<&TypeRef>, added: &TypeRef) -> Option<TypeRef> {
    match current {
        Some(current) => Some(crate::infer::join_types(current, added)),
        None => Some(added.clone()),
    }
}

fn merge_member_completion(
    members: &mut BTreeMap<String, MemberCompletion>,
    incoming: MemberCompletion,
) {
    match members.entry(incoming.name.clone()) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(incoming);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let existing = entry.get_mut();
            if let Some(annotation) = incoming.annotation.as_ref() {
                existing.annotation =
                    merge_member_annotation(existing.annotation.as_ref(), annotation);
            }
            if existing.docs.is_none() {
                existing.docs = incoming.docs;
            } else if let Some(incoming_docs) = incoming.docs
                && !existing
                    .docs
                    .as_ref()
                    .is_some_and(|docs| docs.contains(incoming_docs.as_str()))
            {
                let current_docs = existing.docs.take().expect("checked docs presence");
                existing.docs = Some(format!("{current_docs}\n\n---\n\n{incoming_docs}"));
            }
        }
    }
}

fn enrich_member_completions_with_inference(
    members: &mut BTreeMap<String, MemberCompletion>,
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
) {
    if let Some(receiver_ty) = receiver_type_for_member_completion(analysis, receiver) {
        enrich_member_completions_with_type(members, &receiver_ty);
    }

    let member_names = members.keys().cloned().collect::<Vec<_>>();
    for name in member_names {
        let Some(annotation) = inferred_member_annotation(analysis, receiver, name.as_str()) else {
            continue;
        };

        if let Some(member) = members.get_mut(name.as_str()) {
            member.annotation = Some(annotation);
        }
    }
}

fn inferred_member_annotation(
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
    field_name: &str,
) -> Option<TypeRef> {
    if let Some(TypeRef::Object(fields)) = receiver_type_for_member_completion(analysis, receiver)
        && let Some(annotation) = fields.get(field_name)
    {
        return Some(annotation.clone());
    }

    field_value_exprs_from_expr(&analysis.hir, receiver, field_name)
        .into_iter()
        .filter_map(|expr| {
            analysis
                .hir
                .expr_type(expr, &analysis.type_inference.expr_types)
                .cloned()
        })
        .reduce(|left, right| crate::infer::join_types(&left, &right))
}

fn should_skip_ambiguous_host_member(
    members: &BTreeMap<String, MemberCompletion>,
    incoming: &MemberCompletion,
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
) -> bool {
    let Some(receiver_ty) = receiver_type_for_member_completion(analysis, receiver) else {
        return false;
    };

    should_skip_ambiguous_host_member_for_type(members, incoming, &receiver_ty)
}

fn should_skip_ambiguous_host_member_for_type(
    members: &BTreeMap<String, MemberCompletion>,
    incoming: &MemberCompletion,
    receiver_ty: &TypeRef,
) -> bool {
    if incoming.source != MemberCompletionSource::HostTypeMember
        || !type_supports_field_method_ambiguity(receiver_ty)
    {
        return false;
    }

    let Some(existing) = members.get(incoming.name.as_str()) else {
        return false;
    };

    matches!(
        existing.source,
        MemberCompletionSource::DocumentedField | MemberCompletionSource::ObjectLiteralField
    ) && existing
        .annotation
        .as_ref()
        .is_some_and(type_contains_callable)
}

fn enrich_member_completions_with_type(
    members: &mut BTreeMap<String, MemberCompletion>,
    receiver_ty: &TypeRef,
) {
    if let TypeRef::Object(fields) = receiver_ty {
        for (name, annotation) in fields {
            match members.entry(name.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(MemberCompletion {
                        name: name.clone(),
                        annotation: Some(annotation.clone()),
                        docs: None,
                        range: None,
                        source: MemberCompletionSource::ObjectLiteralField,
                    });
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().annotation = Some(annotation.clone());
                }
            }
        }
    }
}

fn receiver_type_for_member_completion(
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
) -> Option<TypeRef> {
    analysis
        .hir
        .expr_type(receiver, &analysis.type_inference.expr_types)
        .cloned()
        .or_else(|| {
            symbol_for_expr(&analysis.hir, receiver)
                .and_then(|symbol| analysis.type_inference.symbol_types.get(&symbol).cloned())
        })
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

fn type_contains_callable(ty: &TypeRef) -> bool {
    match ty {
        TypeRef::Function(_) => true,
        TypeRef::Union(items) | TypeRef::Ambiguous(items) => {
            items.iter().any(type_contains_callable)
        }
        _ => false,
    }
}

fn method_docs(method: &crate::HostFunction) -> Option<String> {
    let mut docs = method
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
