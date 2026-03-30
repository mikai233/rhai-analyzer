use std::collections::{BTreeMap, HashSet};

use crate::builtin::signatures::host_type_name_for_type;
use crate::db::DatabaseSnapshot;
use crate::db::query_support::{object_field_member_completions, symbol_for_expr};
use crate::db::rebuild::default_file_stats;
use crate::infer::calls::merge_function_candidate_signatures;
use crate::infer::generics::specialize_signature_with_receiver_and_arg_types;
use crate::types::{
    AutoImportCandidate, CachedFileAnalysis, CompletionInputs, DatabaseDebugView, DebugFileAnalysis,
};
use rhai_hir::{CompletionSymbol, MemberCompletion, MemberCompletionSource, TypeRef};
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
            .filter(|symbol| !visible_names.contains(symbol.symbol.name.as_str()))
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
    let Some(access) = analysis
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
        .min_by_key(|access| access.range.len())
    else {
        return Vec::new();
    };

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

    for member in host_type_member_completions(snapshot, file_id, analysis, access.receiver) {
        members.entry(member.name.clone()).or_insert(member);
    }

    members.into_values().collect()
}

fn host_type_member_completions(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    analysis: &CachedFileAnalysis,
    receiver: rhai_hir::ExprId,
) -> Vec<MemberCompletion> {
    let Some(receiver_ty) = analysis
        .hir
        .expr_type(receiver, &analysis.type_inference.expr_types)
        .cloned()
        .or_else(|| {
            snapshot
                .inferred_expr_type_at(file_id, analysis.hir.expr(receiver).range.start())
                .cloned()
        })
    else {
        return Vec::new();
    };

    let mut members = BTreeMap::<String, MemberCompletion>::new();
    collect_host_type_member_completions(&mut members, snapshot.host_types(), &receiver_ty);
    members.into_values().collect()
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
                return;
            };
            let Some(host_type) = host_types
                .iter()
                .find(|host_type| host_type.name == host_type_name)
            else {
                return;
            };
            add_host_type_members(members, ty, host_type, host_types);
        }
    }
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
