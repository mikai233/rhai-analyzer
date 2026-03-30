use crate::{DatabaseSnapshot, HostFunction};
use rhai_hir::{FileHir, FunctionTypeRef, SymbolId, SymbolKind};
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

fn global_function_by_name<'a>(snapshot: &'a DatabaseSnapshot, name: &str) -> &'a HostFunction {
    snapshot
        .global_functions()
        .iter()
        .find(|function| function.name == name)
        .unwrap_or_else(|| panic!("expected builtin global function `{name}`"))
}

fn assert_global_functions_include(snapshot: &DatabaseSnapshot, expected_names: &[&str]) {
    let global_function_names = snapshot
        .global_functions()
        .iter()
        .map(|function| function.name.as_str())
        .collect::<Vec<_>>();

    for expected in expected_names {
        assert!(
            global_function_names.contains(expected),
            "expected builtin global function `{expected}`, got {global_function_names:?}"
        );
    }
}

fn assert_global_function_has_signature(
    snapshot: &DatabaseSnapshot,
    name: &str,
    expected_signature: &FunctionTypeRef,
) {
    assert!(
        global_function_by_name(snapshot, name)
            .overloads
            .iter()
            .filter_map(|overload| overload.signature.as_ref())
            .any(|signature| signature == expected_signature),
        "expected builtin global function `{name}` to include signature {expected_signature:?}"
    );
}
