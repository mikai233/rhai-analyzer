use crate::tests::parse_valid;
use crate::{ReferenceKind, lower_file};

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

#[test]
fn path_queries_preserve_base_and_import_alias_semantics() {
    let parse = parse_valid(
        r#"
            import "provider" as tools;

            fn run() {
                tools::sub::helper
            }
        "#,
    );

    let hir = lower_file(&parse);
    let path_expr = hir
        .exprs
        .iter()
        .enumerate()
        .find_map(|(index, expr)| {
            (expr.kind == crate::ExprKind::Path).then_some(crate::ExprId(index as u32))
        })
        .expect("expected path expr");

    let path = hir.path_expr(path_expr).expect("expected path info");
    assert!(!path.rooted_global);
    assert!(path.base.is_some());
    assert_eq!(
        hir.qualified_path_parts(path_expr),
        Some(vec![
            "tools".to_owned(),
            "sub".to_owned(),
            "helper".to_owned()
        ])
    );

    let imported = hir
        .imported_module_path(path_expr)
        .expect("expected imported module path");
    assert_eq!(imported.import, 0);
    assert_eq!(hir.symbol(imported.alias).name, "tools");
    assert_eq!(
        imported.parts,
        vec!["tools".to_owned(), "sub".to_owned(), "helper".to_owned()]
    );
}
