use crate::tests::parse_valid;
use crate::{BodyKind, ExprKind, ScopeKind, SymbolKind, lower_file};
use rhai_syntax::TextSize;

#[test]
fn lowering_tracks_for_iterable_bindings_and_body_metadata() {
    let source = r#"
            for (item, index) in [1, 2, 3] { item + index }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let for_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("for").unwrap()).unwrap(),
        ))
        .expect("expected for expression");
    let for_info = hir.for_expr(for_expr).expect("expected for expr metadata");

    assert_eq!(
        for_info.iterable.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Array)
    );
    assert_eq!(for_info.bindings.len(), 2);
    assert_eq!(hir.symbol(for_info.bindings[0]).name, "item");
    assert_eq!(hir.symbol(for_info.bindings[1]).name, "index");

    let body = for_info.body.expect("expected for body");
    assert_eq!(hir.body(body).kind, BodyKind::Block);
    assert_eq!(
        hir.body_tail_value(body).map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Binary)
    );
}
#[test]
fn lowering_uses_dedicated_catch_and_switch_arm_scopes() {
    let parse = parse_valid(
        r#"
            try { throw err; } catch (error) { error; }

            switch mode {
                "prod" => deploy(),
                _ => fallback(),
            }
        "#,
    );
    let hir = lower_file(&parse);

    assert!(
        hir.scopes
            .iter()
            .any(|scope| scope.kind == ScopeKind::Catch)
    );
    let switch_arm_scopes = hir
        .scopes
        .iter()
        .filter(|scope| scope.kind == ScopeKind::SwitchArm)
        .count();
    assert_eq!(switch_arm_scopes, 2);
}
#[test]
fn lowering_records_shadowing_and_duplicate_metadata() {
    let parse = parse_valid(
        r#"
            let value = 1;
            {
                let value = 2;
                let local = value;
                let local = 3;
            }
        "#,
    );
    let hir = lower_file(&parse);

    let value_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .collect::<Vec<_>>();
    assert_eq!(value_symbols.len(), 2);
    assert_eq!(hir.shadowed_symbol_of(value_symbols[0]), None);
    assert_eq!(
        hir.shadowed_symbol_of(value_symbols[1]),
        Some(value_symbols[0])
    );
    assert_eq!(hir.duplicate_definition_of(value_symbols[0]), None);
    assert_eq!(hir.duplicate_definition_of(value_symbols[1]), None);

    let local_symbols = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.name == "local" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .collect::<Vec<_>>();
    assert_eq!(local_symbols.len(), 2);
    assert_eq!(hir.duplicate_definition_of(local_symbols[0]), None);
    assert_eq!(
        hir.duplicate_definition_of(local_symbols[1]),
        Some(local_symbols[0])
    );
    assert_eq!(hir.shadowed_symbol_of(local_symbols[0]), None);
    assert_eq!(hir.shadowed_symbol_of(local_symbols[1]), None);
}
