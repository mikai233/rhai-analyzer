use rhai_db::DatabaseSnapshot;

use crate::support::convert::call_hierarchy_item_from_db;
use crate::{CallHierarchyItem, FilePosition, IncomingCall, OutgoingCall};

pub(crate) fn prepare_call_hierarchy(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> Vec<CallHierarchyItem> {
    snapshot
        .prepare_call_hierarchy(
            position.file_id,
            crate::support::convert::text_size(position.offset),
        )
        .iter()
        .map(call_hierarchy_item_from_db)
        .collect()
}

pub(crate) fn incoming_calls(
    snapshot: &DatabaseSnapshot,
    item: &CallHierarchyItem,
) -> Vec<IncomingCall> {
    let Some(identity) = call_hierarchy_identity(snapshot, item) else {
        return Vec::new();
    };

    snapshot
        .incoming_calls(&identity)
        .iter()
        .map(|call| IncomingCall {
            from: call_hierarchy_item_from_db(&call.from),
            from_ranges: call.from_ranges.iter().copied().collect(),
        })
        .collect()
}

pub(crate) fn outgoing_calls(
    snapshot: &DatabaseSnapshot,
    item: &CallHierarchyItem,
) -> Vec<OutgoingCall> {
    let Some(identity) = call_hierarchy_identity(snapshot, item) else {
        return Vec::new();
    };

    snapshot
        .outgoing_calls(&identity)
        .iter()
        .map(|call| OutgoingCall {
            to: call_hierarchy_item_from_db(&call.to),
            from_ranges: call.from_ranges.iter().copied().collect(),
        })
        .collect()
}

fn call_hierarchy_identity(
    snapshot: &DatabaseSnapshot,
    item: &CallHierarchyItem,
) -> Option<rhai_hir::FileBackedSymbolIdentity> {
    snapshot
        .prepare_call_hierarchy(item.file_id, item.focus_range.start())
        .into_iter()
        .find(|candidate| {
            candidate.target.focus_range == item.focus_range && candidate.symbol.name == item.name
        })
        .map(|candidate| candidate.symbol)
}
