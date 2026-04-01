use crate::tests::parse_valid;
use crate::{SemanticDiagnosticKind, TypeRef, lower_file};

#[test]
fn lowers_typed_method_receiver_metadata() {
    let parse = parse_valid(
        r#"
            fn int.do_update(x) {
                this += x;
            }

            fn "Custom-Type".refresh() {
                this = 1;
            }
        "#,
    );

    let hir = lower_file(&parse);
    let method_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.name == "do_update" || symbol.name == "refresh")
                .then_some(crate::SymbolId(index as u32))
        })
        .collect::<Vec<_>>();
    assert_eq!(method_symbols.len(), 2);

    let first = hir
        .function_info(method_symbols[0])
        .expect("expected first function info");
    let second = hir
        .function_info(method_symbols[1])
        .expect("expected second function info");
    assert_eq!(first.this_type, Some(TypeRef::Int));
    assert_eq!(
        second.this_type,
        Some(TypeRef::Named("Custom-Type".to_owned()))
    );
}
#[test]
fn typed_methods_with_distinct_receivers_do_not_conflict() {
    let parse = parse_valid(
        r#"
            fn int.do_update(x) { this += x; }
            fn string.do_update(x) { this += x; }
        "#,
    );

    let hir = lower_file(&parse);
    let duplicates = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::DuplicateDefinition)
        .collect::<Vec<_>>();
    assert!(duplicates.is_empty(), "{duplicates:?}");
}
#[test]
fn typed_methods_with_same_receiver_still_conflict() {
    let parse = parse_valid(
        r#"
            fn int.do_update(x) { this += x; }
            fn int.do_update(x) { this += x; }
        "#,
    );

    let hir = lower_file(&parse);
    let duplicates = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::DuplicateDefinition)
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 1, "{duplicates:?}");
}
