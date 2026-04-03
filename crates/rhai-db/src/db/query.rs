use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::builtin::semantic_keys::{BuiltinSemanticKey, builtin_property_access_semantic_key};
use crate::builtin::signatures::{
    builtin_universal_method_names, builtin_universal_method_signature, host_type_name_for_type,
};
use crate::builtin::topics::builtin_property_access_topic;
use crate::db::DatabaseSnapshot;
use crate::db::imports::export_symbol_docs;
use crate::db::query_support::{object_field_member_completions, symbol_for_expr};
use crate::db::rebuild::default_file_stats;
use crate::infer::calls::{merge_function_candidate_signatures, preferred_completion_signature};
use crate::infer::field_value_exprs_from_expr;
use crate::infer::generics::specialize_signature_with_receiver_and_arg_types;
use crate::infer::infer_member_type_from_expr;
use crate::types::{
    AutoImportCandidate, CachedFileAnalysis, CompletionInputs, DatabaseDebugView, DebugFileAnalysis,
};
use rhai_hir::{
    CompletionSymbol, ExprId, FileHir, MemberCompletion, MemberCompletionSource, TypeRef,
};
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
        let Some(reference_id) = analysis.hir.reference_at_cursor(offset) else {
            return Vec::new();
        };
        let reference = analysis.hir.reference(reference_id);
        if reference.target.is_some() {
            return Vec::new();
        }

        match reference.kind {
            rhai_hir::ReferenceKind::Name => self.auto_import_candidates_for_name_at(
                file_id,
                reference.name.as_str(),
                reference.range,
            ),
            rhai_hir::ReferenceKind::PathSegment => {
                let Some(expr_id) = analysis.hir.expr_at_offset(reference.range.start()) else {
                    return Vec::new();
                };
                let Some(path_expr) = analysis.hir.path_expr(expr_id) else {
                    return Vec::new();
                };
                if path_expr.segments.last().copied() != Some(reference_id) {
                    return Vec::new();
                }
                let path_parts = if path_expr.rooted_global {
                    let mut parts = vec![String::from("global")];
                    parts.extend(
                        path_expr
                            .segments
                            .iter()
                            .map(|segment| analysis.hir.reference(*segment).name.clone()),
                    );
                    parts
                } else {
                    let Some(path_parts) = analysis.hir.qualified_path_parts(expr_id) else {
                        return Vec::new();
                    };
                    path_parts
                };
                let Some((member_name, module_path)) = path_parts.split_last() else {
                    return Vec::new();
                };
                self.auto_import_candidates_for_module_path_at(
                    file_id,
                    module_path,
                    member_name.as_str(),
                    reference.range,
                )
            }
            _ => Vec::new(),
        }
    }

    pub fn auto_import_candidates_for_name(
        &self,
        file_id: FileId,
        name: &str,
    ) -> Vec<AutoImportCandidate> {
        self.auto_import_candidates_for_name_at(
            file_id,
            name,
            rhai_syntax::TextRange::empty(TextSize::from(0)),
        )
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

impl DatabaseSnapshot {
    fn auto_import_candidates_for_name_at(
        &self,
        file_id: FileId,
        name: &str,
        replace_range: rhai_syntax::TextRange,
    ) -> Vec<AutoImportCandidate> {
        let Some(importer_path) = self.normalized_path(file_id) else {
            return Vec::new();
        };
        let Some(hir) = self.hir(file_id) else {
            return Vec::new();
        };
        let Some(file_text) = self.file_text(file_id) else {
            return Vec::new();
        };
        let taken_aliases = existing_import_aliases(hir.as_ref());

        let name_lower = name.to_ascii_lowercase();
        let mut candidates = self
            .workspace_exports()
            .iter()
            .filter(|export| {
                export
                    .export
                    .exported_name
                    .as_deref()
                    .is_some_and(|exported_name| {
                        let exported_name = exported_name.to_ascii_lowercase();
                        name_lower.is_empty()
                            || exported_name.starts_with(name_lower.as_str())
                            || exported_name.contains(name_lower.as_str())
                    })
            })
            .filter_map(|export| {
                let identity = crate::db::imports::project_identity_for_export(export)?;
                let exported_name = export.export.exported_name.as_ref()?;
                if export.file_id == file_id {
                    return None;
                }

                let provider_path = self.normalized_path(export.file_id)?.to_path_buf();
                let module_name = auto_import_module_name(importer_path, provider_path.as_path())?;
                let existing_alias = existing_auto_import_alias(self, file_id, export.file_id);
                let alias = existing_alias.clone().unwrap_or_else(|| {
                    auto_import_alias_for_module(module_name.as_str(), &taken_aliases)
                });
                let insert_text = existing_alias.as_ref().map_or_else(
                    || {
                        auto_import_insert_text(
                            hir.as_ref(),
                            file_text.as_ref(),
                            module_name.as_str(),
                            alias.as_str(),
                        )
                    },
                    |_| Some(String::new()),
                )?;
                let insertion_offset = if insert_text.is_empty() {
                    TextSize::from(0)
                } else {
                    auto_import_insertion_offset(hir.as_ref(), file_text.as_ref())
                };
                let qualified_reference_text = format!("{alias}::{exported_name}");
                let annotation_symbol = export
                    .export
                    .target
                    .as_ref()
                    .or(export.export.alias.as_ref())
                    .unwrap_or(identity);
                let annotation = self
                    .inferred_symbol_type(export.file_id, annotation_symbol.symbol)
                    .cloned()
                    .or_else(|| {
                        self.hir(export.file_id).and_then(|provider_hir| {
                            provider_hir
                                .declared_symbol_type(annotation_symbol.symbol)
                                .cloned()
                        })
                    });
                let docs = export_symbol_docs(self, export);

                Some(AutoImportCandidate {
                    file_id,
                    provider_file_id: export.file_id,
                    provider_path,
                    symbol: annotation_symbol.symbol,
                    name: exported_name.clone(),
                    kind: annotation_symbol.kind,
                    annotation,
                    docs,
                    module_name: module_name.clone(),
                    alias,
                    replace_range,
                    qualified_reference_text,
                    insertion_offset,
                    insert_text,
                    import_cost: if existing_alias.is_some() {
                        0
                    } else {
                        auto_import_cost(module_name.as_str())
                    },
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            left.import_cost
                .cmp(&right.import_cost)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.provider_file_id.0.cmp(&right.provider_file_id.0))
                .then_with(|| left.symbol.0.cmp(&right.symbol.0))
        });
        candidates.dedup_by(|left, right| {
            left.provider_file_id == right.provider_file_id
                && left.alias == right.alias
                && left.symbol == right.symbol
        });
        candidates
    }

    fn auto_import_candidates_for_module_path_at(
        &self,
        file_id: FileId,
        module_path: &[String],
        member_name: &str,
        replace_range: rhai_syntax::TextRange,
    ) -> Vec<AutoImportCandidate> {
        let Some(importer_path) = self.normalized_path(file_id) else {
            return Vec::new();
        };
        let Some(hir) = self.hir(file_id) else {
            return Vec::new();
        };
        let Some(file_text) = self.file_text(file_id) else {
            return Vec::new();
        };
        let taken_aliases = existing_import_aliases(hir.as_ref());

        let (desired_alias, restrict_default_alias) = match module_path {
            [segment] if segment == "global" => {
                if taken_aliases.contains("global") {
                    return Vec::new();
                }
                (String::from("global"), None)
            }
            [segment] => {
                if taken_aliases.contains(segment.as_str()) {
                    return Vec::new();
                }
                (segment.clone(), Some(segment.as_str()))
            }
            _ => return Vec::new(),
        };

        let member_name_lower = member_name.to_ascii_lowercase();
        let mut candidates = self
            .workspace_exports()
            .iter()
            .filter(|export| {
                export
                    .export
                    .exported_name
                    .as_deref()
                    .is_some_and(|exported_name| {
                        let exported_name = exported_name.to_ascii_lowercase();
                        member_name_lower.is_empty()
                            || exported_name.starts_with(member_name_lower.as_str())
                            || exported_name.contains(member_name_lower.as_str())
                    })
            })
            .filter_map(|export| {
                let identity = crate::db::imports::project_identity_for_export(export)?;
                let exported_name = export.export.exported_name.as_ref()?;
                if export.file_id == file_id {
                    return None;
                }

                let provider_path = self.normalized_path(export.file_id)?.to_path_buf();
                let module_name = auto_import_module_name(importer_path, provider_path.as_path())?;
                if let Some(restrict_default_alias) = restrict_default_alias
                    && auto_import_alias(module_name.as_str()) != restrict_default_alias
                {
                    return None;
                }

                let insert_text = auto_import_insert_text(
                    hir.as_ref(),
                    file_text.as_ref(),
                    module_name.as_str(),
                    desired_alias.as_str(),
                )?;
                let insertion_offset =
                    auto_import_insertion_offset(hir.as_ref(), file_text.as_ref());
                let annotation_symbol = export
                    .export
                    .target
                    .as_ref()
                    .or(export.export.alias.as_ref())
                    .unwrap_or(identity);
                let annotation = self
                    .inferred_symbol_type(export.file_id, annotation_symbol.symbol)
                    .cloned()
                    .or_else(|| {
                        self.hir(export.file_id).and_then(|provider_hir| {
                            provider_hir
                                .declared_symbol_type(annotation_symbol.symbol)
                                .cloned()
                        })
                    });
                let docs = export_symbol_docs(self, export);

                Some(AutoImportCandidate {
                    file_id,
                    provider_file_id: export.file_id,
                    provider_path,
                    symbol: annotation_symbol.symbol,
                    name: exported_name.clone(),
                    kind: annotation_symbol.kind,
                    annotation,
                    docs,
                    module_name: module_name.clone(),
                    alias: desired_alias.clone(),
                    replace_range,
                    qualified_reference_text: exported_name.clone(),
                    insertion_offset,
                    insert_text,
                    import_cost: auto_import_cost(module_name.as_str()),
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            left.import_cost
                .cmp(&right.import_cost)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.provider_file_id.0.cmp(&right.provider_file_id.0))
                .then_with(|| left.symbol.0.cmp(&right.symbol.0))
        });
        candidates.dedup_by(|left, right| {
            left.provider_file_id == right.provider_file_id
                && left.alias == right.alias
                && left.symbol == right.symbol
        });
        candidates
    }
}

fn auto_import_module_name(importer_path: &Path, provider_path: &Path) -> Option<String> {
    let relative = strip_rhai_extension(provider_path);
    let importer_dir = importer_path.parent().unwrap_or_else(|| Path::new(""));
    let candidate = relative
        .strip_prefix(importer_dir)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(relative.as_path());
    let module_name = candidate.to_string_lossy().replace('\\', "/");
    (!module_name.is_empty()).then_some(module_name)
}

fn strip_rhai_extension(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension == "rhai")
    {
        path.with_extension("")
    } else {
        path.to_path_buf()
    }
}

fn auto_import_alias(module_name: &str) -> String {
    module_name
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(module_name)
        .to_owned()
}

fn auto_import_alias_for_module(module_name: &str, taken_aliases: &HashSet<String>) -> String {
    let base = auto_import_alias(module_name);
    if !taken_aliases.contains(base.as_str()) {
        return base;
    }

    (1..)
        .map(|index| format!("{base}_{index}"))
        .find(|candidate| !taken_aliases.contains(candidate.as_str()))
        .unwrap_or(base)
}

fn existing_auto_import_alias(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    provider_file_id: FileId,
) -> Option<String> {
    let hir = snapshot.hir(file_id)?;
    let linked_import = snapshot
        .linked_imports(file_id)
        .iter()
        .find(|linked_import| linked_import.provider_file_id == provider_file_id)?;
    let alias = hir.import(linked_import.import).alias?;
    Some(hir.symbol(alias).name.clone())
}

fn existing_import_aliases(hir: &FileHir) -> HashSet<String> {
    hir.imports
        .iter()
        .filter_map(|import| import.alias)
        .map(|alias| hir.symbol(alias).name.clone())
        .collect()
}

fn auto_import_insertion_offset(hir: &FileHir, file_text: &str) -> TextSize {
    if let Some(import) = hir.imports.last() {
        return import.range.end();
    }

    let offset = file_text
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    TextSize::from(offset as u32)
}

fn auto_import_insert_text(
    hir: &FileHir,
    file_text: &str,
    module_name: &str,
    alias: &str,
) -> Option<String> {
    let import_stmt = format!("import \"{module_name}\" as {alias};");
    if hir.imports.is_empty() {
        return Some(if file_text.trim().is_empty() {
            import_stmt
        } else {
            format!("{import_stmt}\n\n")
        });
    }

    Some(format!("\n{import_stmt}"))
}

fn auto_import_cost(module_name: &str) -> u8 {
    module_name
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count()
        .saturating_sub(1)
        .min(6) as u8
}

fn visible_completion_symbols(
    analysis: &CachedFileAnalysis,
    offset: TextSize,
) -> Vec<CompletionSymbol> {
    let Some(query_support) = analysis.query_support.as_ref() else {
        return analysis.hir.completion_symbols_at_cursor(offset);
    };

    analysis
        .hir
        .visible_symbols_with_scope_distance_at_cursor(offset)
        .into_iter()
        .filter_map(|(symbol, scope_distance)| {
            query_support
                .completion_symbols_by_symbol
                .get(&symbol)
                .cloned()
                .map(|mut completion| {
                    completion.scope_distance = scope_distance;
                    completion
                })
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

        enrich_member_completions_with_inference(&mut members, snapshot, analysis, access.receiver);

        for member in host_type_member_completions(snapshot, file_id, analysis, access.receiver) {
            if should_skip_ambiguous_host_member(
                &members,
                &member,
                snapshot,
                analysis,
                access.receiver,
            ) {
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
    let Some(receiver_ty) = receiver_type_for_member_completion(snapshot, analysis, receiver)
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
            .visible_symbols_at_cursor(offset)
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

    enrich_member_completions_with_inference(&mut members, snapshot, analysis, receiver_expr);

    for member in host_type_member_completions(snapshot, file_id, analysis, receiver_expr) {
        if should_skip_ambiguous_host_member(&members, &member, snapshot, analysis, receiver_expr) {
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
    let bytes = text.as_bytes();
    let mut prefix_start = offset;
    while prefix_start > 0 && is_identifier_byte(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }

    if prefix_start == 0 || bytes.get(prefix_start - 1).copied() != Some(b'.') {
        return None;
    }

    let dot_offset = TextSize::from((prefix_start - 1) as u32);
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
        let callable_overloads =
            method_completion_overloads(receiver_ty, host_type, method, host_types);
        members
            .entry(method.name.clone())
            .or_insert(MemberCompletion {
                name: method.name.clone(),
                annotation: completion_annotation_for_overloads(callable_overloads.as_slice()),
                callable_overloads,
                docs: method_docs(method),
                range: None,
                source: MemberCompletionSource::HostTypeMember,
            });
    }
}

fn method_completion_overloads(
    receiver_ty: &TypeRef,
    host_type: &crate::HostType,
    method: &crate::HostFunction,
    host_types: &[crate::HostType],
) -> Vec<rhai_hir::FunctionTypeRef> {
    method
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
        .collect()
}

fn completion_annotation_for_overloads(overloads: &[rhai_hir::FunctionTypeRef]) -> Option<TypeRef> {
    match overloads {
        [] => None,
        [signature] => Some(TypeRef::Function(signature.clone())),
        _ => merge_function_candidate_signatures(overloads.to_vec(), None)
            .or_else(|| preferred_completion_signature(overloads.iter().cloned()))
            .map(TypeRef::Function),
    }
}

fn add_universal_method_members(members: &mut BTreeMap<String, MemberCompletion>) {
    for method_name in builtin_universal_method_names() {
        let callable_overloads = builtin_universal_method_signature(method_name)
            .into_iter()
            .collect::<Vec<_>>();
        members
            .entry((*method_name).to_owned())
            .or_insert(MemberCompletion {
                name: (*method_name).to_owned(),
                annotation: completion_annotation_for_overloads(callable_overloads.as_slice()),
                callable_overloads,
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
                callable_overloads: Vec::new(),
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
            for overload in incoming.callable_overloads {
                if !existing.callable_overloads.contains(&overload) {
                    existing.callable_overloads.push(overload);
                }
            }
            if !existing.callable_overloads.is_empty() {
                existing.annotation =
                    completion_annotation_for_overloads(existing.callable_overloads.as_slice());
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
    snapshot: &DatabaseSnapshot,
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
) {
    if let Some(receiver_ty) = receiver_type_for_member_completion(snapshot, analysis, receiver) {
        enrich_member_completions_with_type(members, &receiver_ty);
    }

    let member_names = members.keys().cloned().collect::<Vec<_>>();
    for name in member_names {
        let Some(annotation) =
            inferred_member_annotation(snapshot, analysis, receiver, name.as_str())
        else {
            continue;
        };

        if let Some(member) = members.get_mut(name.as_str()) {
            member.annotation = Some(annotation);
        }
    }
}

fn inferred_member_annotation(
    snapshot: &DatabaseSnapshot,
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
    field_name: &str,
) -> Option<TypeRef> {
    if let Some(TypeRef::Object(fields)) =
        receiver_type_for_member_completion(snapshot, analysis, receiver)
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
    snapshot: &DatabaseSnapshot,
    analysis: &CachedFileAnalysis,
    receiver: ExprId,
) -> bool {
    let Some(receiver_ty) = receiver_type_for_member_completion(snapshot, analysis, receiver)
    else {
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
    )
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
                        callable_overloads: Vec::new(),
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
    snapshot: &DatabaseSnapshot,
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
        .or_else(|| infer_field_expr_type_for_member_completion(snapshot, analysis, receiver))
}

fn infer_field_expr_type_for_member_completion(
    _snapshot: &DatabaseSnapshot,
    analysis: &CachedFileAnalysis,
    expr: ExprId,
) -> Option<TypeRef> {
    let access = analysis.hir.member_access(expr)?;
    let field_name = analysis.hir.reference(access.field_reference).name.as_str();
    let read_ty = infer_member_type_from_expr(
        &analysis.hir,
        &analysis.type_inference,
        access.receiver,
        field_name,
    );
    let method_ty = builtin_universal_method_signature(field_name).map(TypeRef::Function);

    match (read_ty, method_ty) {
        (Some(read_ty), Some(method_ty))
            if crate::receiver_supports_field_method_ambiguity(
                &analysis.hir,
                &analysis.type_inference,
                access.receiver,
            ) =>
        {
            Some(crate::infer::join_types(&read_ty, &method_ty))
        }
        (Some(read_ty), _) => Some(read_ty),
        (_, Some(method_ty)) => Some(method_ty),
        _ => None,
    }
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
