use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::db::rebuild::default_file_stats;
use crate::db::{AnalyzerDatabase, DatabaseSnapshot};
use crate::types::{
    CachedFileAnalysis, CachedMemberCompletionSet, CachedNavigationTarget, PerFileQuerySupport,
};
use rhai_hir::{
    CompletionSymbol, ExprId, ExprKind, FileHir, MemberCompletion, MemberCompletionSource,
    NavigationTarget, SymbolId, TypeRef,
};
use rhai_vfs::FileId;

impl AnalyzerDatabase {
    pub fn query_support_budget(&self) -> Option<usize> {
        self.query_support_budget
    }

    pub fn set_query_support_budget(&mut self, budget: Option<usize>) -> Vec<FileId> {
        self.query_support_budget = budget;
        let evicted = self.enforce_query_support_budget();
        self.refresh_file_stats_metadata();
        evicted
    }

    pub fn warm_query_support(&mut self, file_ids: &[FileId]) -> usize {
        let mut warmed = 0;
        for &file_id in file_ids {
            warmed += usize::from(self.ensure_query_support(file_id));
        }
        let _ = self.enforce_query_support_budget();
        self.refresh_file_stats_metadata();
        warmed
    }

    pub fn warm_workspace_queries(&mut self) -> usize {
        let mut file_ids = self.analysis.keys().copied().collect::<Vec<_>>();
        file_ids.sort_by_key(|file_id| file_id.0);
        self.warm_query_support(&file_ids)
    }

    pub fn snapshot(&self) -> DatabaseSnapshot {
        DatabaseSnapshot {
            vfs: Arc::new(self.vfs.clone()),
            project: Arc::new(self.project.clone()),
            revision: self.revision,
            project_revision: self.project_revision,
            project_semantics: Arc::clone(&self.project_semantics),
            analysis: Arc::new(self.analysis.clone()),
            workspace_symbols: Arc::clone(&self.workspace_indexes.workspace_symbols),
            workspace_module_graphs: Arc::clone(&self.workspace_indexes.workspace_module_graphs),
            workspace_exports: Arc::clone(&self.workspace_indexes.workspace_exports),
            workspace_dependency_graph: Arc::clone(
                &self.workspace_indexes.workspace_dependency_graph,
            ),
            symbol_locations: Arc::clone(&self.workspace_indexes.symbol_locations),
            exports_by_name: Arc::clone(&self.workspace_indexes.exports_by_name),
            linked_imports: Arc::clone(&self.workspace_indexes.linked_imports),
            file_stats: Arc::new(self.file_stats.clone()),
            stats: Arc::new(self.stats.clone()),
        }
    }

    pub(crate) fn ensure_query_support(&mut self, file_id: FileId) -> bool {
        let Some(existing) = self.analysis.get(&file_id).cloned() else {
            return false;
        };
        if existing.query_support.is_some() {
            return false;
        }

        let query_started = Instant::now();
        let query_support = Arc::new(build_query_support(
            file_id,
            existing.dependencies.normalized_path.clone(),
            &existing.hir,
        ));
        self.stats.query_support_rebuilds += 1;
        self.stats.total_query_support_time += query_started.elapsed();

        let mut updated = (*existing).clone();
        updated.query_support = Some(query_support);
        self.analysis.insert(file_id, Arc::new(updated));
        self.touch_query_support(file_id);
        let entry = self
            .file_stats
            .entry(file_id)
            .or_insert_with(|| default_file_stats(file_id));
        entry.normalized_path = existing.dependencies.normalized_path.clone();
        entry.query_support_rebuilds += 1;
        entry.query_support_cached = true;
        true
    }

    pub(crate) fn touch_query_support(&mut self, file_id: FileId) {
        self.next_query_support_ticket += 1;
        self.query_support_tickets
            .insert(file_id, self.next_query_support_ticket);
    }

    pub(crate) fn enforce_query_support_budget(&mut self) -> Vec<FileId> {
        let Some(budget) = self.query_support_budget else {
            return Vec::new();
        };

        let cached_count = self
            .analysis
            .values()
            .filter(|analysis| analysis.query_support.is_some())
            .count();
        if cached_count <= budget {
            return Vec::new();
        }

        let mut ranked = self
            .query_support_tickets
            .iter()
            .filter_map(|(&file_id, &ticket)| {
                self.analysis
                    .get(&file_id)
                    .and_then(|analysis| analysis.query_support.as_ref().map(|_| (file_id, ticket)))
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(_, ticket)| *ticket);

        let mut evicted = Vec::new();
        while self
            .analysis
            .values()
            .filter(|analysis| analysis.query_support.is_some())
            .count()
            > budget
        {
            let Some((file_id, _)) = ranked.first().copied() else {
                break;
            };
            ranked.remove(0);
            if self.evict_query_support(file_id) {
                evicted.push(file_id);
            }
        }

        evicted.sort_by_key(|file_id| file_id.0);
        evicted
    }

    pub(crate) fn evict_query_support(&mut self, file_id: FileId) -> bool {
        let Some(existing) = self.analysis.get(&file_id).cloned() else {
            return false;
        };
        if existing.query_support.is_none() {
            return false;
        }

        let mut updated = (*existing).clone();
        updated.query_support = None;
        self.analysis.insert(file_id, Arc::new(updated));
        self.query_support_tickets.remove(&file_id);
        self.stats.query_support_evictions += 1;

        let entry = self
            .file_stats
            .entry(file_id)
            .or_insert_with(|| default_file_stats(file_id));
        entry.query_support_evictions += 1;
        entry.query_support_cached = false;
        true
    }

    pub(crate) fn refresh_file_stats_metadata(&mut self) {
        let mut active_file_ids = self.analysis.keys().copied().collect::<Vec<_>>();
        active_file_ids.sort_by_key(|file_id| file_id.0);

        for file_id in active_file_ids {
            let dependency_count = self
                .workspace_indexes
                .workspace_dependency_graph
                .dependencies_by_file
                .get(&file_id)
                .map_or(0, |files| files.len());
            let dependent_count = self
                .workspace_indexes
                .workspace_dependency_graph
                .dependents_by_file
                .get(&file_id)
                .map_or(0, |files| files.len());
            let normalized_path = self
                .analysis
                .get(&file_id)
                .map(|analysis| analysis.dependencies.normalized_path.clone())
                .unwrap_or_default();
            let query_support_cached = self
                .analysis
                .get(&file_id)
                .is_some_and(|analysis| analysis.query_support.is_some());

            let entry = self
                .file_stats
                .entry(file_id)
                .or_insert_with(|| default_file_stats(file_id));
            entry.normalized_path = normalized_path;
            entry.dependency_count = dependency_count;
            entry.dependent_count = dependent_count;
            entry.query_support_cached = query_support_cached;
        }

        self.file_stats
            .retain(|file_id, _| self.analysis.contains_key(file_id));
    }
}

pub(crate) fn build_query_support(
    file_id: FileId,
    normalized_path: PathBuf,
    hir: &FileHir,
) -> PerFileQuerySupport {
    let completion_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            (
                SymbolId(index as u32),
                CompletionSymbol {
                    symbol: SymbolId(index as u32),
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    range: symbol.range,
                    docs: symbol.docs,
                    annotation: symbol.annotation.clone(),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let navigation_targets = hir
        .symbols
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let symbol = SymbolId(index as u32);
            (
                symbol,
                CachedNavigationTarget {
                    symbol: hir.file_backed_symbol_identity(symbol),
                    target: hir.navigation_target(symbol),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let member_completion_sets = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            let symbol = SymbolId(index as u32);
            let members = member_completion_support_for_symbol(hir, symbol);
            (!members.is_empty()).then_some((
                symbol,
                CachedMemberCompletionSet {
                    symbol: hir.file_backed_symbol_identity(symbol),
                    members,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut completion_symbol_entries = completion_symbols.values().cloned().collect::<Vec<_>>();
    completion_symbol_entries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.range.start().cmp(&right.range.start()))
    });

    let mut navigation_target_entries = navigation_targets.values().cloned().collect::<Vec<_>>();
    navigation_target_entries.sort_by(|left, right| {
        left.symbol.name.cmp(&right.symbol.name).then_with(|| {
            left.target
                .full_range
                .start()
                .cmp(&right.target.full_range.start())
        })
    });

    let mut member_completion_entries =
        member_completion_sets.values().cloned().collect::<Vec<_>>();
    member_completion_entries.sort_by(|left, right| left.symbol.name.cmp(&right.symbol.name));

    PerFileQuerySupport {
        file_id,
        normalized_path,
        completion_symbols: Arc::from(completion_symbol_entries),
        navigation_targets: Arc::from(navigation_target_entries),
        member_completion_sets: Arc::from(member_completion_entries),
        completion_symbols_by_symbol: Arc::new(completion_symbols),
        navigation_targets_by_symbol: Arc::new(
            navigation_targets
                .into_iter()
                .map(|(symbol, entry)| (symbol, entry.target))
                .collect(),
        ),
        member_completion_sets_by_symbol: Arc::new(
            member_completion_sets
                .into_iter()
                .map(|(symbol, entry)| (symbol, entry.members))
                .collect(),
        ),
    }
}

fn member_completion_support_for_symbol(
    hir: &FileHir,
    symbol: SymbolId,
) -> Arc<[MemberCompletion]> {
    let mut members = BTreeMap::<String, MemberCompletion>::new();

    for field in hir.documented_fields(symbol) {
        members
            .entry(field.name.clone())
            .or_insert(MemberCompletion {
                name: field.name,
                annotation: Some(field.annotation),
                docs: field.docs,
                range: None,
                source: MemberCompletionSource::DocumentedField,
            });
    }

    for flow in hir.value_flows_into(symbol) {
        for member in object_field_member_completions(hir, flow.expr) {
            members.entry(member.name.clone()).or_insert(member);
        }
    }

    Arc::from(members.into_values().collect::<Vec<_>>())
}

pub(crate) fn object_field_member_completions(
    hir: &FileHir,
    expr: ExprId,
) -> Vec<MemberCompletion> {
    hir.object_fields
        .iter()
        .filter(|field| field.owner == expr)
        .map(|field| MemberCompletion {
            name: field.name.clone(),
            annotation: field
                .value
                .and_then(|value| object_field_annotation_from_expr(hir, value)),
            docs: None,
            range: Some(field.range),
            source: MemberCompletionSource::ObjectLiteralField,
        })
        .collect()
}

fn object_field_annotation_from_expr(hir: &FileHir, expr: ExprId) -> Option<TypeRef> {
    match hir.expr(expr).kind {
        ExprKind::Literal => hir.literal(expr).map(|literal| match literal.kind {
            rhai_hir::LiteralKind::Int => TypeRef::Int,
            rhai_hir::LiteralKind::Float => TypeRef::Float,
            rhai_hir::LiteralKind::String => TypeRef::String,
            rhai_hir::LiteralKind::Char => TypeRef::Char,
            rhai_hir::LiteralKind::Bool => TypeRef::Bool,
        }),
        ExprKind::Object => Some(TypeRef::Object(
            hir.object_fields
                .iter()
                .filter(|field| field.owner == expr)
                .map(|field| {
                    (
                        field.name.clone(),
                        field
                            .value
                            .and_then(|value| object_field_annotation_from_expr(hir, value))
                            .unwrap_or(TypeRef::Unknown),
                    )
                })
                .collect(),
        )),
        ExprKind::Array => Some(TypeRef::Array(Box::new(TypeRef::Unknown))),
        ExprKind::Closure => Some(TypeRef::Function(rhai_hir::FunctionTypeRef {
            params: Vec::new(),
            ret: Box::new(TypeRef::Unknown),
        })),
        ExprKind::Name => {
            symbol_for_expr(hir, expr).and_then(|symbol| hir.declared_symbol_type(symbol).cloned())
        }
        _ => None,
    }
}

pub(crate) fn symbol_for_expr(hir: &FileHir, expr: ExprId) -> Option<SymbolId> {
    match hir.expr(expr).kind {
        ExprKind::Name => hir
            .reference_at(hir.expr(expr).range)
            .and_then(|reference| hir.definition_of(reference)),
        _ => None,
    }
}

pub(crate) fn cached_navigation_target(
    analysis: &CachedFileAnalysis,
    symbol: SymbolId,
) -> NavigationTarget {
    analysis
        .query_support
        .as_ref()
        .and_then(|query_support| {
            query_support
                .navigation_targets_by_symbol
                .get(&symbol)
                .copied()
        })
        .unwrap_or_else(|| analysis.hir.navigation_target(symbol))
}
