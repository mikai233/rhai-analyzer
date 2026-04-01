use crate::tests::parse_valid;
use crate::{FunctionTypeRef, SymbolKind, TypeRef, lower_file};

#[test]
fn attaches_doc_blocks_and_type_annotations() {
    let parse = parse_valid(
        r#"
            /// counter docs
            /// @type int
            let count = 1;
        "#,
    );

    let hir = lower_file(&parse);
    let count = hir
        .symbols
        .iter()
        .find(|symbol| symbol.name == "count")
        .expect("expected `count` symbol");

    let docs = count.docs.expect("expected docs on `count`");
    assert!(hir.docs[docs.0 as usize].text.contains("counter docs"));
    assert_eq!(count.annotation, Some(TypeRef::Int));
}
#[test]
fn attaches_docs_to_more_declaration_kinds() {
    let parse = parse_valid(
        r#"
            /** outer docs */
            fn outer() {}

            //! helper docs
            fn helper() {}

            /// const docs
            const LIMIT = 1;
            let exported_limit = LIMIT;

            /// import docs
            import "crypto" as secure;

            /// export docs
            export exported_limit as public_outer;
        "#,
    );

    let hir = lower_file(&parse);
    let docs_for = |name: &str, kind: SymbolKind| {
        let symbol = hir
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.kind == kind)
            .expect("expected symbol");
        hir.doc_block(symbol.docs.expect("expected docs"))
            .text
            .clone()
    };

    assert!(docs_for("outer", SymbolKind::Function).contains("outer docs"));
    assert!(docs_for("helper", SymbolKind::Function).contains("helper docs"));
    assert!(docs_for("LIMIT", SymbolKind::Constant).contains("const docs"));
    assert!(docs_for("secure", SymbolKind::ImportAlias).contains("import docs"));
    assert!(docs_for("public_outer", SymbolKind::ExportAlias).contains("export docs"));
}
#[test]
fn synthesizes_function_and_parameter_annotations_from_docs() {
    let parse = parse_valid(
        r#"
            /// @param left int
            /// @param right string
            /// @return bool
            fn check(left, right) {
                left == right
            }
        "#,
    );

    let hir = lower_file(&parse);
    let check = hir
        .symbols
        .iter()
        .find(|symbol| symbol.name == "check" && symbol.kind == SymbolKind::Function)
        .expect("expected `check` function");
    assert_eq!(
        check.annotation,
        Some(TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int, TypeRef::String],
            ret: Box::new(TypeRef::Bool),
        }))
    );

    let left = hir
        .symbols
        .iter()
        .find(|symbol| symbol.name == "left" && symbol.kind == SymbolKind::Parameter)
        .expect("expected `left` parameter");
    let right = hir
        .symbols
        .iter()
        .find(|symbol| symbol.name == "right" && symbol.kind == SymbolKind::Parameter)
        .expect("expected `right` parameter");
    assert_eq!(left.annotation, Some(TypeRef::Int));
    assert_eq!(right.annotation, Some(TypeRef::String));
}
