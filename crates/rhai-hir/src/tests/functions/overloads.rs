use crate::tests::parse_valid;
use crate::{SemanticDiagnosticKind, SymbolKind, lower_file};

#[test]
fn functions_with_distinct_arities_do_not_conflict() {
    let parse = parse_valid(
        r#"
            fn do_something() {}
            fn do_something(value) {}
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
fn functions_with_same_arity_and_distinct_param_types_do_not_conflict() {
    let parse = parse_valid(
        r#"
            /// @param value int
            fn do_something(value) {}

            /// @param value string
            fn do_something(value) {}
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
fn functions_with_same_arity_and_same_param_types_still_conflict() {
    let parse = parse_valid(
        r#"
            /// @param value int
            fn do_something(value) {}

            /// @param value int
            fn do_something(value) {}
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
#[test]
fn local_calls_resolve_to_matching_function_overloads_by_arity() {
    let parse = parse_valid(
        r#"
            fn do_something() {}
            fn do_something(value) {}

            do_something();
            do_something(1);
        "#,
    );

    let hir = lower_file(&parse);
    let functions = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Function && symbol.name == "do_something")
                .then_some(crate::SymbolId(index as u32))
        })
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);

    let zero_arg = functions
        .iter()
        .copied()
        .find(|symbol| hir.function_parameters(*symbol).is_empty())
        .expect("expected zero-arg overload");
    let one_arg = functions
        .iter()
        .copied()
        .find(|symbol| hir.function_parameters(*symbol).len() == 1)
        .expect("expected one-arg overload");

    assert_eq!(hir.calls.len(), 2);
    assert_eq!(hir.calls[0].resolved_callee, Some(zero_arg));
    assert_eq!(hir.calls[1].resolved_callee, Some(one_arg));
    assert!(hir.calls[0].parameter_bindings.is_empty());
    assert_eq!(hir.calls[1].parameter_bindings.len(), 1);

    let call_refs = hir
        .references
        .iter()
        .filter(|reference| reference.name == "do_something")
        .collect::<Vec<_>>();
    assert_eq!(call_refs.len(), 2);
    assert!(
        call_refs
            .iter()
            .any(|reference| reference.target == Some(zero_arg))
    );
    assert!(
        call_refs
            .iter()
            .any(|reference| reference.target == Some(one_arg))
    );
}
#[test]
fn resolves_forward_functions_without_resolving_future_variables() {
    let parse = parse_valid(
        r#"
            let result = later(1);
            let early = value;
            let value = 1;

            fn later(value) {
                value
            }
        "#,
    );

    let hir = lower_file(&parse);
    let later_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "later" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `later` symbol");

    let later_ref = hir
        .references
        .iter()
        .find(|reference| reference.name == "later")
        .expect("expected call to `later`");
    assert_eq!(later_ref.target, Some(later_symbol));

    let value_refs: Vec<_> = hir
        .references
        .iter()
        .filter(|reference| reference.name == "value")
        .collect();
    assert_eq!(value_refs.len(), 2);
    assert!(
        value_refs
            .iter()
            .any(|reference| reference.target.is_none())
    );
    assert!(
        value_refs
            .iter()
            .filter_map(|reference| reference.target)
            .any(|target| hir.symbol(target).kind == SymbolKind::Parameter)
    );
}
