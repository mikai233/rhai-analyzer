use crate::tests::parse_valid;
use crate::{SymbolKind, lower_file};

#[test]
fn file_symbol_index_exposes_indexable_symbols_with_container_and_export_metadata() {
    let parse = parse_valid(
        r#"
            const LIMIT = 1;

            fn outer() {}
            {
                let local = 1;
            }

            import "crypto" as secure;
            let exported_outer = LIMIT;
            export exported_outer as public_outer;
        "#,
    );
    let hir = lower_file(&parse);

    let index = hir.file_symbol_index();
    let names = index
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"LIMIT"));
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"secure"));
    assert!(names.contains(&"public_outer"));
    assert!(!names.contains(&"local"));

    let outer = index
        .entries
        .iter()
        .find(|entry| entry.name == "outer")
        .expect("expected outer entry");
    assert!(outer.exported);
    assert!(outer.container_name.is_none());

    let public_outer = index
        .entries
        .iter()
        .find(|entry| entry.name == "public_outer")
        .expect("expected public export alias entry");
    assert!(public_outer.exported);
}

#[test]
fn file_backed_symbol_identity_captures_container_path_and_export_status() {
    let parse = parse_valid(
        r#"
            fn outer(arg) {
                let local = arg;
            }

            private fn hidden() {}
            let exported_value = 1;
            export exported_value as public_outer;
        "#,
    );
    let hir = lower_file(&parse);

    let outer = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "outer" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `outer` symbol");
    let arg = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "arg" && symbol.kind == SymbolKind::Parameter)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `arg` symbol");
    let hidden = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "hidden" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `hidden` symbol");

    let outer_identity = hir.file_backed_symbol_identity(outer);
    let arg_identity = hir.file_backed_symbol_identity(arg);
    let hidden_identity = hir.file_backed_symbol_identity(hidden);

    assert!(outer_identity.exported);
    assert!(outer_identity.container_path.is_empty());
    assert_eq!(outer_identity.stable_key.name, "outer");
    assert_eq!(outer_identity.stable_key.ordinal, 0);
    assert_eq!(arg_identity.container_path, vec!["outer"]);
    assert!(!arg_identity.exported);
    assert!(!hidden_identity.exported);
}

#[test]
fn stable_symbol_keys_distinguish_duplicate_indexable_symbols() {
    let parse = parse_valid(
        r#"
            const inner = 1;
            const inner = 2;
        "#,
    );
    let hir = lower_file(&parse);

    let inner_keys = hir
        .workspace_symbols()
        .into_iter()
        .filter(|symbol| symbol.name == "inner")
        .map(|symbol| symbol.stable_key.ordinal)
        .collect::<Vec<_>>();

    assert_eq!(inner_keys, vec![0, 1]);
}
