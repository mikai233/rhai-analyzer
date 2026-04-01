use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_syntax::TextSize;
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_infers_local_script_method_calls_and_this_types() {
    let mut db = AnalyzerDatabase::default();
    let source = r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            let value = 40;
            let result = value.bump(2);
        "#;
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
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");

    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);
    let this_offset =
        TextSize::from(u32::try_from(source.find("this +").expect("expected this")).unwrap());

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_expr_type_at(file_id, this_offset),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}
#[test]
fn snapshot_prefers_typed_script_methods_over_blanket_methods() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            /// @param delta string
            /// @return string
            fn bump(delta) {
                delta
            }

            let value = 40;
            let result = value.bump(2);
        "#,
        DocumentVersion(1),
    ));

    let snapshot = db.snapshot();
    assert_workspace_files_have_no_syntax_diagnostics(&snapshot);
    let file_id = snapshot
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let hir = snapshot.hir(file_id).expect("expected hir");
    let result = symbol_id_by_name(&hir, "result", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, result),
        Some(&TypeRef::Int)
    );
}
