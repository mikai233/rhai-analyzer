use crate::DatabaseSnapshot;
use rhai_hir::{FileHir, SymbolId, SymbolKind};
use rhai_syntax::TextSize;

mod inference_basic;
mod inference_cross_file;
mod inference_flow;
mod methods;
mod queries;
mod snapshot;
mod workspace;

fn offset_in(text: &str, needle: &str) -> TextSize {
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("expected to find `{needle}` in:\n{text}"));
    TextSize::from(u32::try_from(offset).expect("expected offset to fit into u32"))
}

fn assert_workspace_files_have_no_syntax_diagnostics(snapshot: &DatabaseSnapshot) {
    for file in snapshot.workspace_files() {
        let diagnostics = snapshot.syntax_diagnostics(file.file_id);
        assert!(
            diagnostics.is_empty(),
            "expected valid Rhai syntax for {:?}, got diagnostics: {:?}",
            snapshot.normalized_path(file.file_id),
            diagnostics
        );
    }
}

fn symbol_id_by_name(hir: &FileHir, name: &str, kind: SymbolKind) -> SymbolId {
    let index = hir
        .symbols
        .iter()
        .position(|symbol| symbol.name == name && symbol.kind == kind)
        .unwrap_or_else(|| panic!("expected symbol `{name}` with kind {kind:?}"));
    SymbolId(index as u32)
}
