use crate::tests::parse_valid;
use crate::{ImportExposureKind, ImportLinkageKind, ModuleSpecifier, lower_file};

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
        Some(ModuleSpecifier::Text(ref text)) if text == "\"crypto\""
    ));
    assert_eq!(literal_import.linkage, ImportLinkageKind::StaticText);
    assert_eq!(literal_import.exposure, ImportExposureKind::Aliased);
    assert!(literal_import.is_global);
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
        Some(ModuleSpecifier::LocalSymbol(ref symbol)) if symbol.name == "module_name"
    ));
    assert_eq!(local_import.linkage, ImportLinkageKind::LocalSymbol);
    assert_eq!(local_import.exposure, ImportExposureKind::Aliased);
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
fn module_graph_index_marks_dynamic_imports_without_static_specifier() {
    let parse = parse_valid(
        r#"
            let module_name = "crypto";
            import module_name;
        "#,
    );

    let hir = lower_file(&parse);
    let module_index = hir.module_graph_index();
    let import = &module_index.imports[0];

    assert!(import.module.is_some());
    assert_eq!(import.linkage, ImportLinkageKind::LocalSymbol);
    assert_eq!(import.exposure, ImportExposureKind::Bare);
}
