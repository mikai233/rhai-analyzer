use std::path::Path;

use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet, DatabaseSnapshot};
use rhai_hir::{SymbolId, SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use rhai_vfs::FileId;

pub(crate) mod member_reads;
pub(crate) mod nullability;
pub(crate) mod type_guards;

pub(crate) fn load_snapshot(source: &str) -> (DatabaseSnapshot, FileId) {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        source,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected file id");

    (snapshot, file_id)
}

pub(crate) fn variable_id(snapshot: &DatabaseSnapshot, file_id: FileId, name: &str) -> SymbolId {
    let hir = snapshot.hir(file_id).expect("expected hir");
    symbol_id_by_name(&hir, name, SymbolKind::Variable)
}

pub(crate) fn assert_variable_type(
    snapshot: &DatabaseSnapshot,
    file_id: FileId,
    name: &str,
    expected: TypeRef,
) {
    let symbol = variable_id(snapshot, file_id, name);
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, symbol),
        Some(&expected)
    );
}
