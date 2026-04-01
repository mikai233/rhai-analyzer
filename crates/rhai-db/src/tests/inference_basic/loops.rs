use crate::tests::{assert_workspace_files_have_no_syntax_diagnostics, symbol_id_by_name};
use crate::{AnalyzerDatabase, ChangeSet};
use rhai_hir::{SymbolKind, TypeRef};
use rhai_vfs::DocumentVersion;
use std::path::Path;

#[test]
fn snapshot_infers_loop_expression_result_types() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let spun = loop { break; };
            let counted = loop { break 1; };
            let checked = while false { break; };
            let iterated = for item in [1, 2, 3] { if item > 1 { break; } };
            let retried = do { break; } while false;
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

    let spun = symbol_id_by_name(&hir, "spun", SymbolKind::Variable);
    let counted = symbol_id_by_name(&hir, "counted", SymbolKind::Variable);
    let checked = symbol_id_by_name(&hir, "checked", SymbolKind::Variable);
    let iterated = symbol_id_by_name(&hir, "iterated", SymbolKind::Variable);
    let retried = symbol_id_by_name(&hir, "retried", SymbolKind::Variable);

    assert_eq!(
        snapshot.inferred_symbol_type(file_id, spun),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, counted),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, checked),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, iterated),
        Some(&TypeRef::Unit)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, retried),
        Some(&TypeRef::Unit)
    );
}
#[test]
fn snapshot_infers_for_loop_binding_types_for_common_iterables() {
    let mut db = AnalyzerDatabase::default();
    db.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let array_item = for item in [1, 2, 3] { break item; };
            let array_index = for (item, index) in [1, 2, 3] { break index; };
            let char_item = for ch in "abc" { break ch; };
            let ranged = for value in 0..10 { break value; };
            let map_value = for entry in #{ name: "Ada" }.values() { break entry; };
            let split_part = for part in "a,b".split(",") { break part; };
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

    let array_item = symbol_id_by_name(&hir, "array_item", SymbolKind::Variable);
    let array_index = symbol_id_by_name(&hir, "array_index", SymbolKind::Variable);
    let char_item = symbol_id_by_name(&hir, "char_item", SymbolKind::Variable);
    let ranged = symbol_id_by_name(&hir, "ranged", SymbolKind::Variable);
    let map_value = symbol_id_by_name(&hir, "map_value", SymbolKind::Variable);
    let split_part = symbol_id_by_name(&hir, "split_part", SymbolKind::Variable);
    let item = symbol_id_by_name(&hir, "item", SymbolKind::Variable);
    let index = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(symbol_index, symbol)| {
            (symbol.name == "index" && symbol.kind == SymbolKind::Variable)
                .then_some(rhai_hir::SymbolId(symbol_index as u32))
        })
        .expect("expected `index` loop binding");
    let ch = symbol_id_by_name(&hir, "ch", SymbolKind::Variable);
    let value = symbol_id_by_name(&hir, "value", SymbolKind::Variable);
    let entry = symbol_id_by_name(&hir, "entry", SymbolKind::Variable);
    let part = symbol_id_by_name(&hir, "part", SymbolKind::Variable);

    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, array_item),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, array_index),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, char_item),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Char)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, ranged),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::Int)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, map_value),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::String)
    ));
    assert!(matches!(
        snapshot.inferred_symbol_type(file_id, split_part),
        Some(TypeRef::Union(items))
            if items.contains(&TypeRef::Unit) && items.contains(&TypeRef::String)
    ));
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, item),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, index),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, ch),
        Some(&TypeRef::Char)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, value),
        Some(&TypeRef::Int)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, entry),
        Some(&TypeRef::String)
    );
    assert_eq!(
        snapshot.inferred_symbol_type(file_id, part),
        Some(&TypeRef::String)
    );
}
