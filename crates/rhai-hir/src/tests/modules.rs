use crate::tests::parse_valid;
use crate::{ReferenceKind, SymbolKind, lower_file};

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

#[test]
fn module_graph_index_preserves_import_and_export_linkage_shapes() {
    let parse = parse_valid(
        r#"
            fn exported_fn() {}
            private fn hidden() {}
            let module_name = "crypto";
            let module_value = 1;
            import "crypto" as secure;
            import module_name as local_alias;
            export module_value as public_api;
        "#,
    );
    let hir = lower_file(&parse);
    let module_index = hir.module_graph_index();

    assert_eq!(module_index.imports.len(), 2);
    assert_eq!(module_index.exports.len(), 2);

    let literal_import = &module_index.imports[0];
    assert!(matches!(
        literal_import.module,
        Some(crate::ModuleSpecifier::Text(ref text)) if text == "\"crypto\""
    ));
    assert_eq!(
        literal_import
            .alias
            .as_ref()
            .map(|alias| alias.name.as_str()),
        Some("secure")
    );

    let local_import = &module_index.imports[1];
    assert!(matches!(
        local_import.module,
        Some(crate::ModuleSpecifier::LocalSymbol(ref symbol)) if symbol.name == "module_name"
    ));
    assert_eq!(
        local_import.alias.as_ref().map(|alias| alias.name.as_str()),
        Some("local_alias")
    );

    let implicit_export = module_index
        .exports
        .iter()
        .find(|export| export.exported_name.as_deref() == Some("exported_fn"))
        .expect("expected implicit function export");
    assert_eq!(
        implicit_export
            .target
            .as_ref()
            .map(|target| target.name.as_str()),
        Some("exported_fn")
    );
    assert!(implicit_export.alias.is_none());

    let export = module_index
        .exports
        .iter()
        .find(|export| export.exported_name.as_deref() == Some("public_api"))
        .expect("expected explicit export");
    assert_eq!(
        export.target.as_ref().map(|target| target.name.as_str()),
        Some("module_value")
    );
    assert_eq!(
        export.alias.as_ref().map(|alias| alias.name.as_str()),
        Some("public_api")
    );
    assert!(
        !module_index
            .exports
            .iter()
            .any(|export| export.exported_name.as_deref() == Some("hidden"))
    );
}

#[test]
fn lowering_records_exported_declarations_and_alias_targets() {
    let parse = parse_valid(
        r#"
            export const ANSWER = 42;
            let value = 1;
            export value as public_value;
        "#,
    );
    let hir = lower_file(&parse);

    assert_eq!(hir.exports.len(), 2);

    let answer_export = &hir.exports[0];
    assert_eq!(answer_export.target_text.as_deref(), Some("ANSWER"));
    assert!(answer_export.target_symbol.is_some());
    assert!(answer_export.target_reference.is_none());
    assert!(answer_export.alias.is_none());

    let value_export = &hir.exports[1];
    assert_eq!(value_export.target_text.as_deref(), Some("value"));
    assert!(value_export.target_symbol.is_none());
    assert!(value_export.target_reference.is_some());
    assert_eq!(
        value_export
            .alias
            .map(|symbol| hir.symbol(symbol).name.as_str()),
        Some("public_value")
    );
}

#[test]
fn global_path_root_does_not_create_name_reference() {
    let parse = parse_valid(
        r#"
            fn run() {
                global::crypto::sha256
            }
        "#,
    );

    let hir = lower_file(&parse);
    assert!(
        !hir.references
            .iter()
            .any(|reference| reference.name == "global")
    );

    let path_segments: Vec<_> = hir
        .references
        .iter()
        .filter(|reference| reference.kind == ReferenceKind::PathSegment)
        .map(|reference| reference.name.as_str())
        .collect();
    assert_eq!(path_segments, vec!["crypto", "sha256"]);
}
