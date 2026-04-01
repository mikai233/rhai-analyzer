use crate::tests::{parse_valid, slice_range};
use crate::{
    BodyKind, ControlFlowKind, ExprKind, MergePointKind, ScopeKind, SymbolKind, lower_file,
};
use rhai_syntax::TextSize;

#[test]
fn expression_result_slots_are_stable_and_queryable() {
    let source = r#"
            let value = helper(1 + 2);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let call_offset = TextSize::from(u32::try_from(source.rfind('1').unwrap()).unwrap());
    let binary_offset = TextSize::from(u32::try_from(source.find(" + ").unwrap() + 1).unwrap());

    let call_expr = hir
        .expr_at_offset(call_offset)
        .expect("expected call expression");
    let binary_expr = hir
        .expr_at_offset(binary_offset)
        .expect("expected binary expression");

    let call_slot = hir.expr_result_slot(call_expr);
    let binary_slot = hir.expr_result_slot(binary_expr);

    assert_ne!(call_slot, binary_slot);
    assert_eq!(hir.type_slot(call_slot).range, hir.expr(call_expr).range);
    assert_eq!(
        hir.type_slot(binary_slot).range,
        hir.expr(binary_expr).range
    );
    assert_eq!(hir.expr_result_slot_at_offset(call_offset), Some(call_slot));
    assert_eq!(
        hir.expr_result_slot_at_offset(binary_offset),
        Some(binary_slot)
    );
}
#[test]
fn body_summaries_collect_return_throw_values_and_merge_points() {
    let parse = parse_valid(
        r#"
            fn sample(flag, mode, err) {
                if flag { return 1; } else { throw err; }
                switch mode { 0 => 1, _ => 2 }
                while flag { break; }
            }
        "#,
    );
    let hir = lower_file(&parse);

    let function_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "sample" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `sample` symbol");
    let body = hir
        .body_of(function_symbol)
        .expect("expected function body");

    let return_values = hir.body_return_values(body).collect::<Vec<_>>();
    let throw_values = hir.body_throw_values(body).collect::<Vec<_>>();
    let merge_kinds = hir
        .body_merge_points(body)
        .map(|merge| merge.kind)
        .collect::<Vec<_>>();

    assert_eq!(return_values.len(), 1);
    assert_eq!(hir.expr(return_values[0]).kind, ExprKind::Literal);
    assert_eq!(throw_values.len(), 1);
    assert_eq!(hir.expr(throw_values[0]).kind, ExprKind::Name);
    assert!(merge_kinds.contains(&MergePointKind::IfElse));
    assert!(merge_kinds.contains(&MergePointKind::Switch));
    assert!(merge_kinds.contains(&MergePointKind::LoopIteration));
}
#[test]
fn body_summaries_track_tail_values_for_functions_blocks_and_closures() {
    let source = r#"
            fn sample() {
                let inner = { let value = 1; value };
                inner
            }

            let closure = |value| value + 1;
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let function_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "sample" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `sample` symbol");
    let function_body = hir
        .body_of(function_symbol)
        .expect("expected function body");
    let function_tail = hir
        .body_tail_value(function_body)
        .expect("expected function tail value");
    assert_eq!(hir.expr(function_tail).kind, ExprKind::Name);

    let block_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("{ let value = 1; value }").unwrap()).unwrap(),
        ))
        .expect("expected block expression");
    let block_body = hir
        .block_expr(block_expr)
        .expect("expected block info")
        .body;
    assert!(hir.body_tail_value(block_body).is_some());

    let closure_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("|value|").unwrap()).unwrap(),
        ))
        .expect("expected closure expression");
    let closure_body = hir
        .closure_expr(closure_expr)
        .expect("expected closure info")
        .body;
    assert_eq!(
        hir.body_tail_value(closure_body)
            .map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Binary)
    );
}
#[test]
fn body_control_flow_accumulates_nested_blocks_without_crossing_closures() {
    let parse = parse_valid(
        r#"
            fn outer(flag) {
                while flag {
                    if flag { break; }
                    continue;
                }

                if flag {
                    return 1;
                }

                let callback = || {
                    return 2;
                };

                throw "boom";
            }
        "#,
    );

    let hir = lower_file(&parse);
    let outer_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "outer" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `outer` symbol");
    let outer_body_id = hir
        .bodies
        .iter()
        .enumerate()
        .find_map(|(index, body)| {
            (body.kind == BodyKind::Function && body.owner == Some(outer_symbol))
                .then_some(crate::BodyId(index as u32))
        })
        .expect("expected `outer` body");

    let outer_flow: Vec<_> = hir
        .body_control_flow(outer_body_id)
        .map(|event| event.kind)
        .collect();
    assert_eq!(
        outer_flow,
        vec![
            ControlFlowKind::Break,
            ControlFlowKind::Continue,
            ControlFlowKind::Return,
            ControlFlowKind::Throw,
        ]
    );

    let closure_body_id = hir
        .bodies
        .iter()
        .enumerate()
        .find_map(|(index, body)| {
            (body.kind == BodyKind::Closure).then_some(crate::BodyId(index as u32))
        })
        .expect("expected closure body");
    let closure_flow: Vec<_> = hir
        .body_control_flow(closure_body_id)
        .map(|event| event.kind)
        .collect();
    assert_eq!(closure_flow, vec![ControlFlowKind::Return]);
}
#[test]
fn control_flow_events_capture_optional_value_ranges() {
    let source = r#"
            fn sample(flag, err) {
                loop {
                    if flag { break flag; }
                    continue;
                }

                return foo(flag);
                throw err;
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);
    let function_body = hir
        .bodies
        .iter()
        .enumerate()
        .find_map(|(index, body)| {
            (body.kind == BodyKind::Function).then_some(crate::BodyId(index as u32))
        })
        .expect("expected function body");

    let events = hir.body_control_flow(function_body).collect::<Vec<_>>();
    let break_event = events
        .iter()
        .find(|event| event.kind == ControlFlowKind::Break)
        .expect("expected break event");
    let continue_event = events
        .iter()
        .find(|event| event.kind == ControlFlowKind::Continue)
        .expect("expected continue event");
    let return_event = events
        .iter()
        .find(|event| event.kind == ControlFlowKind::Return)
        .expect("expected return event");
    let throw_event = events
        .iter()
        .find(|event| event.kind == ControlFlowKind::Throw)
        .expect("expected throw event");

    assert_eq!(
        slice_range(source, break_event.value_range.expect("break value")),
        "flag"
    );
    assert!(break_event.target_loop.is_some());
    assert!(continue_event.target_loop.is_some());
    assert!(continue_event.value_range.is_none());
    assert_eq!(
        slice_range(source, return_event.value_range.expect("return value")),
        "foo(flag)"
    );
    assert_eq!(
        slice_range(source, throw_event.value_range.expect("throw value")),
        "err"
    );
    assert!(return_event.target_loop.is_none());
    assert!(throw_event.target_loop.is_none());
}
#[test]
fn body_summaries_track_loop_targets_fallthrough_and_unreachable_ranges() {
    let source = r#"
            fn sample(flag) {
                while flag {
                    if flag { break; }
                    continue;
                    let loop_unreachable = 1;
                }

                return 1;
                let function_unreachable = 2;
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let function_body = hir
        .bodies
        .iter()
        .enumerate()
        .find_map(|(index, body)| {
            (body.kind == BodyKind::Function).then_some(crate::BodyId(index as u32))
        })
        .expect("expected function body");
    let loop_body = hir
        .bodies
        .iter()
        .enumerate()
        .find_map(|(index, body)| {
            (body.kind == BodyKind::Block && hir.scope(body.scope).kind == ScopeKind::Loop)
                .then_some(crate::BodyId(index as u32))
        })
        .expect("expected loop body");

    let loop_events = hir.body_control_flow(loop_body).collect::<Vec<_>>();
    assert!(loop_events.iter().all(|event| matches!(
        event.kind,
        ControlFlowKind::Break | ControlFlowKind::Continue
    )));
    assert!(loop_events.iter().all(|event| event.target_loop.is_some()));
    assert!(!hir.body_may_fall_through(loop_body));
    assert_eq!(
        hir.body_unreachable_ranges(loop_body)
            .map(|range| slice_range(source, range).trim())
            .collect::<Vec<_>>(),
        vec!["let loop_unreachable = 1;"]
    );

    assert!(!hir.body_may_fall_through(function_body));
    assert_eq!(
        hir.body_unreachable_ranges(function_body)
            .map(|range| slice_range(source, range).trim())
            .collect::<Vec<_>>(),
        vec!["let function_unreachable = 2;"]
    );
}
