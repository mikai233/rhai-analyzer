use crate::tests::{parse_valid, slice_range};
use crate::{BodyKind, ScopeKind, SymbolKind, lower_file};
use rhai_syntax::TextSize;

#[test]
fn file_lookup_helpers_find_deepest_scope_and_exact_ranges() {
    let source = r#"
            fn wrap(value) {
                { let nested = value; nested }
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);
    let nested_offset = TextSize::from(u32::try_from(source.find("nested").unwrap()).unwrap());
    let scope_id = hir
        .find_scope_at(nested_offset)
        .expect("expected scope at nested binding");
    assert_eq!(hir.scope(scope_id).kind, ScopeKind::Block);

    let nested_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "nested").then_some((crate::SymbolId(index as u32), symbol.range))
        })
        .expect("expected nested symbol");
    assert_eq!(hir.symbol_at(nested_symbol.1), Some(nested_symbol.0));
}
#[test]
fn query_helpers_support_definition_body_and_visible_symbol_lookups() {
    let source = r#"
            const OUTER = 1;

            fn helper(arg) {
                let before = arg;
                {
                    let value = before;
                    value + arg
                }
            }

            let value = 3;
            let result = helper(OUTER);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let helper_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "helper" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `helper` symbol");
    let helper_body = hir.body_of(helper_symbol).expect("expected helper body");
    assert_eq!(hir.body(helper_body).kind, BodyKind::Function);

    let helper_ref = hir
        .references
        .iter()
        .enumerate()
        .find_map(|(index, reference)| {
            (reference.name == "helper").then_some(crate::ReferenceId(index as u32))
        })
        .expect("expected `helper` reference");
    assert_eq!(hir.definition_of(helper_ref), Some(helper_symbol));

    let value_use_offset =
        TextSize::from(u32::try_from(source.rfind("value + arg").unwrap()).unwrap());
    let visible = hir
        .visible_symbols_at(value_use_offset)
        .into_iter()
        .map(|symbol| hir.symbol(symbol))
        .collect::<Vec<_>>();

    assert!(
        visible
            .iter()
            .any(|symbol| symbol.name == "value" && symbol.range.start() < value_use_offset)
    );
    assert!(
        !visible
            .iter()
            .any(|symbol| symbol.name == "value" && symbol.range.start() > value_use_offset)
    );
    assert!(visible.iter().any(|symbol| symbol.name == "arg"));
    assert!(visible.iter().any(|symbol| symbol.name == "before"));
    assert!(visible.iter().any(|symbol| symbol.name == "helper"));
    assert!(!visible.iter().any(|symbol| symbol.name == "result"));
    assert!(!visible.iter().any(|symbol| symbol.name == "OUTER"));
}

#[test]
fn visible_symbol_queries_track_scope_distance() {
    let source = r#"
            fn helper() {
                let outer_value = 1;
                {
                    let inner_value = outer_value;
                    inner_value + outer_value
                }
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let use_offset =
        TextSize::from(u32::try_from(source.rfind("inner_value + outer_value").unwrap()).unwrap());
    let visible = hir.visible_symbols_with_scope_distance_at(use_offset);

    let inner_distance = visible
        .iter()
        .find_map(|(symbol, distance)| {
            (hir.symbol(*symbol).name == "inner_value").then_some(*distance)
        })
        .expect("expected inner_value distance");
    let outer_distance = visible
        .iter()
        .find_map(|(symbol, distance)| {
            (hir.symbol(*symbol).name == "outer_value").then_some(*distance)
        })
        .expect("expected outer_value distance");

    assert_eq!(inner_distance, 0);
    assert_eq!(outer_distance, 1);
}
#[test]
fn offset_based_query_helpers_support_navigation_workflows() {
    let source = r#"
            fn helper(value) {
                value
            }

            let result = helper(1);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let helper_decl_offset = TextSize::from(u32::try_from(source.find("helper").unwrap()).unwrap());
    let helper_call_offset =
        TextSize::from(u32::try_from(source.rfind("helper").unwrap()).unwrap());
    let value_ref_offset = TextSize::from(u32::try_from(source.rfind("value").unwrap()).unwrap());

    let helper_symbol = hir
        .symbol_at_offset(helper_decl_offset)
        .expect("expected helper symbol at declaration");
    let helper_reference = hir
        .reference_at_offset(helper_call_offset)
        .expect("expected helper reference at call");
    assert_eq!(hir.definition_of(helper_reference), Some(helper_symbol));
    assert_eq!(
        hir.definition_at_offset(helper_call_offset),
        Some(helper_symbol)
    );
    assert_eq!(
        hir.definition_at_offset(helper_decl_offset),
        Some(helper_symbol)
    );

    let helper_refs = hir.references_at_offset(helper_decl_offset);
    assert_eq!(helper_refs.len(), 1);
    assert_eq!(helper_refs[0], helper_reference);

    let value_reference = hir
        .reference_at_offset(value_ref_offset)
        .expect("expected value reference in function body");
    let value_symbol = hir
        .definition_at_offset(value_ref_offset)
        .expect("expected definition for value reference");
    assert_eq!(hir.definition_of(value_reference), Some(value_symbol));
    assert_eq!(
        hir.references_at_offset(value_ref_offset),
        vec![value_reference]
    );
}
#[test]
fn navigation_helpers_return_single_file_definition_and_reference_results() {
    let source = r#"
            fn helper(value) {
                value
            }

            let result = helper(1);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let helper_decl_offset = TextSize::from(u32::try_from(source.find("helper").unwrap()).unwrap());
    let helper_call_offset =
        TextSize::from(u32::try_from(source.rfind("helper").unwrap()).unwrap());
    let missing_offset = TextSize::from(u32::try_from(source.find("result").unwrap()).unwrap());

    let helper_target = hir
        .goto_definition(helper_call_offset)
        .expect("expected goto-definition result for helper call");
    let helper_symbol = hir
        .symbol_at_offset(helper_decl_offset)
        .expect("expected helper symbol at declaration");
    assert_eq!(helper_target.symbol, helper_symbol);
    assert_eq!(helper_target.kind, SymbolKind::Function);
    assert_eq!(helper_target.full_range, hir.symbol(helper_symbol).range);

    let declaration_target = hir
        .goto_definition(helper_decl_offset)
        .expect("expected goto-definition result on declaration");
    assert_eq!(declaration_target, helper_target);

    let helper_references = hir
        .find_references(helper_call_offset)
        .expect("expected find-references result for helper call");
    assert_eq!(helper_references.symbol, helper_symbol);
    assert_eq!(helper_references.declaration, helper_target);
    assert_eq!(helper_references.references.len(), 1);
    assert_eq!(
        slice_range(source, helper_references.references[0].range),
        "helper"
    );

    let declaration_references = hir
        .find_references(helper_decl_offset)
        .expect("expected find-references result on declaration");
    assert_eq!(declaration_references, helper_references);

    assert!(hir.goto_definition(missing_offset).is_some());
    assert!(hir.find_references(missing_offset).is_some());
}
#[test]
fn goto_definition_distinguishes_local_function_overloads_by_arity() {
    let source = r#"
            fn do_something() {}
            fn do_something(value) {}

            do_something();
            do_something(1);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let decl_offsets = source
        .match_indices("do_something")
        .map(|(index, _)| TextSize::from(index as u32))
        .collect::<Vec<_>>();
    let zero_decl = decl_offsets[0];
    let one_decl = decl_offsets[1];
    let zero_call = decl_offsets[2];
    let one_call = decl_offsets[3];

    assert_eq!(
        hir.definition_at_offset(zero_call),
        hir.symbol_at_offset(zero_decl)
    );
    assert_eq!(
        hir.definition_at_offset(one_call),
        hir.symbol_at_offset(one_decl)
    );
}
