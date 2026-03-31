use crate::db::DatabaseSnapshot;
use crate::db::imports::{
    imported_global_method_symbols, linked_import_targets_for_path_reference,
};
use crate::db::query_support::cached_navigation_target;
use crate::db::rename::project_reference_kind_rank;
use crate::types::{
    LocatedCallHierarchyItem, LocatedIncomingCall, LocatedNavigationTarget, LocatedOutgoingCall,
    LocatedProjectReference, LocatedSymbolIdentity, LocatedWorkspaceSymbol, ProjectReferenceKind,
    ProjectReferences,
};
use rhai_hir::{FileBackedSymbolIdentity, FileHir, ReferenceId, ScopeId, SymbolId, SymbolKind};
use rhai_syntax::TextSize;
use rhai_vfs::FileId;
use std::collections::BTreeMap;
use std::sync::Arc;

impl DatabaseSnapshot {
    pub fn goto_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedNavigationTarget> {
        if let Some(target) = self.goto_import_module_target(file_id, offset) {
            return vec![target];
        }

        self.project_targets_at(file_id, offset)
            .iter()
            .flat_map(|target| self.navigation_targets_for_identity(&target.symbol))
            .collect()
    }

    pub fn find_references(&self, file_id: FileId, offset: TextSize) -> Option<ProjectReferences> {
        let targets = self.project_targets_at(file_id, offset);
        if targets.is_empty() {
            return None;
        }

        Some(ProjectReferences {
            references: self.collect_project_references(&targets),
            targets,
        })
    }

    fn navigation_targets_for_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedNavigationTarget> {
        let mut targets = self
            .locate_symbol(identity)
            .iter()
            .filter_map(|location| {
                let analysis = self.analysis.get(&location.file_id)?;
                Some(LocatedNavigationTarget {
                    file_id: location.file_id,
                    target: cached_navigation_target(analysis, location.symbol.symbol),
                })
            })
            .collect::<Vec<_>>();

        targets.sort_by(|left, right| {
            left.file_id.0.cmp(&right.file_id.0).then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
        });
        targets
            .dedup_by(|left, right| left.file_id == right.file_id && left.target == right.target);
        targets
    }

    pub(crate) fn project_targets_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedSymbolIdentity> {
        let Some(analysis) = self.analysis.get(&file_id) else {
            return Vec::new();
        };

        if let Some(reference_id) = analysis.hir.reference_at_offset(offset) {
            let path_targets = linked_import_targets_for_path_reference(
                self,
                file_id,
                analysis.hir.as_ref(),
                reference_id,
            );
            if !path_targets.is_empty() {
                return path_targets;
            }

            if let Some(symbol) = analysis.hir.definition_of(reference_id) {
                return self
                    .locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
                    .to_vec();
            }
            if let Some(symbol) =
                resolve_unresolved_name_in_outer_scope(analysis.hir.as_ref(), reference_id)
            {
                return self
                    .locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
                    .to_vec();
            }

            if analysis.hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
                && let Some(access) = analysis
                    .hir
                    .member_accesses
                    .iter()
                    .find(|access| access.field_reference == reference_id)
                && let Some(receiver_ty) = analysis
                    .hir
                    .expr_type(access.receiver, &analysis.type_inference.expr_types)
            {
                let imported = imported_global_method_symbols(
                    self,
                    file_id,
                    receiver_ty,
                    analysis.hir.reference(reference_id).name.as_str(),
                );
                if !imported.is_empty() {
                    return imported;
                }
            }

            return Vec::new();
        }

        let Some(symbol) = analysis.hir.symbol_at_offset(offset) else {
            return Vec::new();
        };

        self.locate_symbol(&analysis.hir.file_backed_symbol_identity(symbol))
            .to_vec()
    }

    pub(crate) fn collect_project_references(
        &self,
        targets: &[LocatedSymbolIdentity],
    ) -> Vec<LocatedProjectReference> {
        let mut references = Vec::new();

        for target in targets {
            let Some(analysis) = self.analysis.get(&target.file_id) else {
                continue;
            };

            references.push(LocatedProjectReference {
                file_id: target.file_id,
                range: target.symbol.declaration_range,
                kind: ProjectReferenceKind::Definition,
            });

            references.extend(analysis.hir.references_to(target.symbol.symbol).map(
                |reference_id| LocatedProjectReference {
                    file_id: target.file_id,
                    range: analysis.hir.reference(reference_id).range,
                    kind: ProjectReferenceKind::Reference,
                },
            ));
            references.extend(
                unresolved_outer_scope_references_to_symbol(
                    analysis.hir.as_ref(),
                    target.symbol.symbol,
                )
                .into_iter()
                .map(|reference_id| LocatedProjectReference {
                    file_id: target.file_id,
                    range: analysis.hir.reference(reference_id).range,
                    kind: ProjectReferenceKind::Reference,
                }),
            );

            for (&candidate_file_id, candidate_analysis) in self.analysis.iter() {
                references.extend(
                    candidate_analysis
                        .hir
                        .references
                        .iter()
                        .enumerate()
                        .flat_map(|(index, reference)| {
                            linked_import_targets_for_path_reference(
                                self,
                                candidate_file_id,
                                candidate_analysis.hir.as_ref(),
                                rhai_hir::ReferenceId(index as u32),
                            )
                            .into_iter()
                            .filter(move |resolved| resolved.symbol == target.symbol)
                            .map(move |_| LocatedProjectReference {
                                file_id: candidate_file_id,
                                range: reference.range,
                                kind: ProjectReferenceKind::Reference,
                            })
                        }),
                );
            }
        }

        references.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| left.range.start().cmp(&right.range.start()))
                .then_with(|| {
                    project_reference_kind_rank(left.kind)
                        .cmp(&project_reference_kind_rank(right.kind))
                })
        });
        references.dedup_by(|left, right| {
            left.file_id == right.file_id && left.range == right.range && left.kind == right.kind
        });
        references
    }

    pub(crate) fn call_hierarchy_items_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<LocatedCallHierarchyItem> {
        let mut items = self
            .project_targets_at(file_id, offset)
            .into_iter()
            .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
            .filter_map(|target| self.call_hierarchy_item_from_identity(&target.symbol))
            .collect::<Vec<_>>();

        items.sort_by(|left, right| {
            left.file_id
                .0
                .cmp(&right.file_id.0)
                .then_with(|| {
                    left.target
                        .full_range
                        .start()
                        .cmp(&right.target.full_range.start())
                })
                .then_with(|| left.symbol.name.cmp(&right.symbol.name))
        });
        items.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
        items
    }

    pub(crate) fn call_hierarchy_incoming_calls(
        &self,
        item: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedIncomingCall> {
        let mut grouped = BTreeMap::<
            (FileId, u32, u32, String),
            (LocatedCallHierarchyItem, Vec<rhai_syntax::TextRange>),
        >::new();

        for (&file_id, analysis) in self.analysis.iter() {
            for call in &analysis.hir.calls {
                let Some(caller) = analysis
                    .hir
                    .enclosing_function_symbol_at(call.range.start())
                    .filter(|caller| {
                        analysis.hir.symbol(*caller).kind == rhai_hir::SymbolKind::Function
                    })
                else {
                    continue;
                };

                if !call_targets_symbol(self, file_id, analysis.hir.as_ref(), call, item) {
                    continue;
                }

                let caller_identity = analysis.hir.file_backed_symbol_identity(caller);
                let Some(caller_item) = self.call_hierarchy_item_from_identity(&caller_identity)
                else {
                    continue;
                };
                let key = (
                    caller_item.file_id,
                    u32::from(caller_item.target.full_range.start()),
                    u32::from(caller_item.target.full_range.end()),
                    caller_item.symbol.name.clone(),
                );
                grouped
                    .entry(key)
                    .or_insert_with(|| (caller_item, Vec::new()))
                    .1
                    .push(call.callee_range.unwrap_or(call.range));
            }
        }

        grouped
            .into_values()
            .map(|(from, mut from_ranges)| {
                from_ranges.sort_by_key(|range| (range.start(), range.end()));
                from_ranges.dedup();
                LocatedIncomingCall {
                    from,
                    from_ranges: Arc::<[rhai_syntax::TextRange]>::from(from_ranges),
                }
            })
            .collect()
    }

    pub(crate) fn call_hierarchy_outgoing_calls(
        &self,
        item: &FileBackedSymbolIdentity,
    ) -> Vec<LocatedOutgoingCall> {
        let mut grouped = BTreeMap::<
            (FileId, u32, u32, String),
            (LocatedCallHierarchyItem, Vec<rhai_syntax::TextRange>),
        >::new();

        for location in self.locate_symbol(item) {
            let Some(analysis) = self.analysis.get(&location.file_id) else {
                continue;
            };
            let symbol = location.symbol.symbol;
            if analysis.hir.symbol(symbol).kind != rhai_hir::SymbolKind::Function {
                continue;
            }

            for call in analysis.hir.calls.iter().filter(|call| {
                analysis
                    .hir
                    .enclosing_function_symbol_at(call.range.start())
                    == Some(symbol)
            }) {
                for target in call_target_items(self, location.file_id, analysis.hir.as_ref(), call)
                {
                    if target.symbol == *item {
                        continue;
                    }
                    let key = (
                        target.file_id,
                        u32::from(target.target.full_range.start()),
                        u32::from(target.target.full_range.end()),
                        target.symbol.name.clone(),
                    );
                    grouped
                        .entry(key)
                        .or_insert_with(|| (target, Vec::new()))
                        .1
                        .push(call.callee_range.unwrap_or(call.range));
                }
            }
        }

        grouped
            .into_values()
            .map(|(to, mut from_ranges)| {
                from_ranges.sort_by_key(|range| (range.start(), range.end()));
                from_ranges.dedup();
                LocatedOutgoingCall {
                    to,
                    from_ranges: Arc::<[rhai_syntax::TextRange]>::from(from_ranges),
                }
            })
            .collect()
    }

    fn call_hierarchy_item_from_identity(
        &self,
        identity: &FileBackedSymbolIdentity,
    ) -> Option<LocatedCallHierarchyItem> {
        let location = self.locate_symbol(identity).first()?.clone();
        let analysis = self.analysis.get(&location.file_id)?;

        Some(LocatedCallHierarchyItem {
            file_id: location.file_id,
            symbol: location.symbol.clone(),
            target: cached_navigation_target(analysis, location.symbol.symbol),
        })
    }

    fn goto_import_module_target(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<LocatedNavigationTarget> {
        let analysis = self.analysis.get(&file_id)?;
        let import_index = analysis.hir.imports.iter().position(|import| {
            import
                .module_range
                .is_some_and(|module_range| module_range.contains(offset))
        })?;
        let linked_import = self.linked_import(file_id, import_index)?;
        self.file_navigation_target(linked_import.provider_file_id)
    }

    fn file_navigation_target(&self, file_id: FileId) -> Option<LocatedNavigationTarget> {
        let analysis = self.analysis.get(&file_id)?;
        let target = analysis
            .document_symbols
            .first()
            .map(|symbol| rhai_hir::NavigationTarget {
                symbol: symbol.symbol,
                kind: symbol.kind,
                full_range: symbol.full_range,
                focus_range: symbol.focus_range,
            })
            .or_else(|| {
                (!analysis.hir.symbols.is_empty())
                    .then(|| cached_navigation_target(analysis, SymbolId(0)))
            })?;

        Some(LocatedNavigationTarget { file_id, target })
    }
}

fn call_targets_symbol(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &rhai_hir::FileHir,
    call: &rhai_hir::CallSite,
    target: &FileBackedSymbolIdentity,
) -> bool {
    call_target_items(snapshot, file_id, hir, call)
        .into_iter()
        .any(|item| item.symbol == *target)
}

fn call_target_items(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    hir: &rhai_hir::FileHir,
    call: &rhai_hir::CallSite,
) -> Vec<LocatedCallHierarchyItem> {
    let mut items = Vec::new();

    if let Some(callee) = call.resolved_callee
        && hir.symbol(callee).kind == rhai_hir::SymbolKind::Function
    {
        let identity = hir.file_backed_symbol_identity(callee);
        if let Some(item) = snapshot.call_hierarchy_item_from_identity(&identity) {
            items.push(item);
        }
    }

    if let Some(callee_range) = call.callee_range {
        items.extend(
            hir.references
                .iter()
                .enumerate()
                .filter_map(|(index, reference)| {
                    callee_range
                        .contains_range(reference.range)
                        .then_some(rhai_hir::ReferenceId(index as u32))
                })
                .flat_map(|reference_id| {
                    linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                })
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
        );
    }

    if let Some(reference_id) = call.callee_reference {
        items.extend(
            linked_import_targets_for_path_reference(snapshot, file_id, hir, reference_id)
                .into_iter()
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
        );

        if hir.reference(reference_id).kind == rhai_hir::ReferenceKind::Field
            && let Some(access) = hir
                .member_accesses
                .iter()
                .find(|access| access.field_reference == reference_id)
            && let Some(receiver_ty) = snapshot
                .type_inference(file_id)
                .and_then(|inference| hir.expr_type(access.receiver, &inference.expr_types))
                .cloned()
                .or_else(|| {
                    snapshot
                        .inferred_expr_type_at(file_id, hir.expr(access.receiver).range.start())
                        .cloned()
                })
        {
            items.extend(
                imported_global_method_symbols(
                    snapshot,
                    file_id,
                    &receiver_ty,
                    hir.reference(reference_id).name.as_str(),
                )
                .into_iter()
                .filter(|target| target.symbol.kind == rhai_hir::SymbolKind::Function)
                .filter_map(|target| snapshot.call_hierarchy_item_from_identity(&target.symbol)),
            );
        }
    }

    items.sort_by(|left, right| {
        left.file_id
            .0
            .cmp(&right.file_id.0)
            .then_with(|| {
                left.target
                    .full_range
                    .start()
                    .cmp(&right.target.full_range.start())
            })
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
    });
    items.dedup_by(|left, right| left.file_id == right.file_id && left.symbol == right.symbol);
    items
}

pub(crate) fn workspace_symbol_match_rank(
    symbol: &LocatedWorkspaceSymbol,
    query: &str,
) -> (u8, u8, String) {
    let name = symbol.symbol.name.to_ascii_lowercase();
    let container = symbol
        .symbol
        .stable_key
        .container_path
        .join("::")
        .to_ascii_lowercase();

    let name_rank = if query.is_empty() || name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else if container.contains(query) {
        3
    } else {
        4
    };

    let export_rank = if symbol.symbol.exported { 0 } else { 1 };
    (name_rank, export_rank, name)
}

fn resolve_unresolved_name_in_outer_scope(
    hir: &FileHir,
    reference_id: ReferenceId,
) -> Option<SymbolId> {
    let reference = hir.reference(reference_id);
    if reference.kind != rhai_hir::ReferenceKind::Name || reference.target.is_some() {
        return None;
    }

    let function_scope = enclosing_function_scope(hir, reference.scope)?;
    let capture = resolve_name_in_outer_scopes(
        hir,
        hir.scope(function_scope).parent?,
        reference.name.as_str(),
        reference.range.start(),
    )?;

    if matches!(
        hir.symbol(capture).kind,
        SymbolKind::Function | SymbolKind::ImportAlias | SymbolKind::ExportAlias
    ) {
        return None;
    }

    Some(capture)
}

fn unresolved_outer_scope_references_to_symbol(
    hir: &FileHir,
    target_symbol: SymbolId,
) -> Vec<ReferenceId> {
    let target = hir.symbol(target_symbol);
    hir.references
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            if reference.kind != rhai_hir::ReferenceKind::Name
                || reference.target.is_some()
                || reference.name != target.name
            {
                return None;
            }
            let reference_id = ReferenceId(index as u32);
            (resolve_unresolved_name_in_outer_scope(hir, reference_id) == Some(target_symbol))
                .then_some(reference_id)
        })
        .collect()
}

fn enclosing_function_scope(hir: &FileHir, mut scope: ScopeId) -> Option<ScopeId> {
    loop {
        let scope_data = hir.scope(scope);
        if scope_data.kind == rhai_hir::ScopeKind::Function {
            return Some(scope);
        }
        scope = scope_data.parent?;
    }
}

fn resolve_name_in_outer_scopes(
    hir: &FileHir,
    mut scope: ScopeId,
    name: &str,
    reference_start: TextSize,
) -> Option<SymbolId> {
    loop {
        if let Some(symbol) = hir
            .scope(scope)
            .symbols
            .iter()
            .rev()
            .copied()
            .find(|symbol_id| {
                let symbol = hir.symbol(*symbol_id);
                symbol.name == name && symbol.range.start() <= reference_start
            })
        {
            return Some(symbol);
        }
        scope = hir.scope(scope).parent?;
    }
}
