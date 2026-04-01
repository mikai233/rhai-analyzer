use crate::db::DatabaseSnapshot;
use crate::db::imports::{
    imported_global_method_symbols, linked_import_targets_for_path_reference,
};
use crate::db::query_support::cached_navigation_target;
use crate::types::{LocatedCallHierarchyItem, LocatedIncomingCall, LocatedOutgoingCall};
use rhai_hir::FileBackedSymbolIdentity;
use rhai_syntax::TextSize;
use rhai_vfs::FileId;
use std::collections::BTreeMap;
use std::sync::Arc;

impl DatabaseSnapshot {
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

    pub(crate) fn call_hierarchy_item_from_identity(
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
