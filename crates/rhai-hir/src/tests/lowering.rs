use crate::tests::{parse_valid, slice_range};
use crate::{
    BinaryOperator, BodyKind, ControlFlowKind, ExprKind, LiteralKind, MergePointKind,
    MutationPathSegment, ReferenceKind, ScopeKind, SymbolKind, SymbolMutationKind, SymbolReadKind,
    UnaryOperator, ValueFlowKind, lower_file,
};
use rhai_syntax::TextSize;

#[test]
fn lowers_symbols_scopes_and_references() {
    let parse = parse_valid(
        r#"
            /// @param value int
            /// @return int
            fn double(value) {
                let local = value;
                { let nested = local; nested }
            }

            const ANSWER = 42;
            import "crypto" as secure;
            let result = double(ANSWER);
        "#,
    );

    let hir = lower_file(&parse);

    assert!(hir.scopes.iter().any(|scope| scope.kind == ScopeKind::File));
    assert!(
        hir.scopes
            .iter()
            .any(|scope| scope.kind == ScopeKind::Function)
    );
    assert!(
        hir.scopes
            .iter()
            .any(|scope| scope.kind == ScopeKind::Block)
    );

    let symbol_names: Vec<_> = hir
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(symbol_names.contains(&"double"));
    assert!(symbol_names.contains(&"value"));
    assert!(symbol_names.contains(&"local"));
    assert!(symbol_names.contains(&"ANSWER"));
    assert!(symbol_names.contains(&"secure"));
    assert!(symbol_names.contains(&"result"));

    assert!(
        hir.references
            .iter()
            .any(|reference| reference.name == "double" && reference.kind == ReferenceKind::Name)
    );
    assert!(
        hir.references
            .iter()
            .any(|reference| reference.name == "ANSWER" && reference.target.is_some())
    );

    assert!(
        hir.bodies
            .iter()
            .any(|body| body.kind == BodyKind::Function)
    );
}

#[test]
fn expression_table_assigns_stable_ids_and_supports_offset_queries() {
    let source = r#"
            let value = helper(1 + 2, data[index]);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    assert!(!hir.exprs.is_empty());

    let call_offset = TextSize::from(u32::try_from(source.find(", data").unwrap()).unwrap());
    let binary_offset = TextSize::from(u32::try_from(source.find(" + ").unwrap() + 1).unwrap());
    let index_offset = TextSize::from(u32::try_from(source.find('[').unwrap()).unwrap());

    let call_expr = hir
        .expr_at_offset(call_offset)
        .expect("expected call expression at callee offset");
    let binary_expr = hir
        .expr_at_offset(binary_offset)
        .expect("expected binary expression at first argument");
    let index_expr = hir
        .expr_at_offset(index_offset)
        .expect("expected index expression at second argument");

    assert_eq!(hir.expr(call_expr).kind, ExprKind::Call);
    assert_eq!(hir.expr(binary_expr).kind, ExprKind::Binary);
    assert_eq!(hir.expr(index_expr).kind, ExprKind::Index);
    assert_ne!(call_expr, binary_expr);
    assert_ne!(binary_expr, index_expr);
    assert_eq!(hir.expr_at(hir.expr(binary_expr).range), Some(binary_expr));

    let callee_offset = TextSize::from(u32::try_from(source.find("helper").unwrap()).unwrap());
    let callee_expr = hir
        .expr_at_offset(callee_offset)
        .expect("expected name expression at callee token");
    assert_eq!(hir.expr(callee_expr).kind, ExprKind::Name);

    let literal_offset = TextSize::from(u32::try_from(source.find("1 + 2").unwrap()).unwrap());
    let literal_expr = hir
        .expr_at_offset(literal_offset)
        .expect("expected literal expression at numeric token");
    assert_eq!(hir.expr(literal_expr).kind, ExprKind::Literal);
    assert_eq!(
        hir.literal(literal_expr).map(|literal| literal.kind),
        Some(LiteralKind::Int)
    );
}

#[test]
fn expression_metadata_tracks_literals_operators_and_call_argument_exprs() {
    let source = r#"
            let value = helper(-1, "x" + "y", true ?? false);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let unary_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("-1").unwrap()).unwrap(),
        ))
        .expect("expected unary expression");
    let first_literal = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("1,").unwrap()).unwrap(),
        ))
        .expect("expected int literal");
    let add_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find(" + ").unwrap() + 1).unwrap(),
        ))
        .expect("expected additive expression");
    let coalesce_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find(" ?? ").unwrap() + 1).unwrap(),
        ))
        .expect("expected null-coalescing expression");
    let call_offset = TextSize::from(u32::try_from(source.find(", true").unwrap()).unwrap());
    let call_id = hir.call_at_offset(call_offset).expect("expected call site");

    assert_eq!(
        hir.unary_expr(unary_expr).map(|unary| unary.operator),
        Some(UnaryOperator::Minus)
    );
    assert_eq!(
        hir.unary_expr(unary_expr).and_then(|unary| unary.operand),
        Some(first_literal)
    );
    assert_eq!(
        hir.literal(first_literal).map(|literal| literal.kind),
        Some(LiteralKind::Int)
    );
    assert_eq!(
        hir.binary_expr(add_expr).map(|binary| binary.operator),
        Some(BinaryOperator::Add)
    );
    assert_eq!(
        hir.binary_expr(coalesce_expr).map(|binary| binary.operator),
        Some(BinaryOperator::NullCoalesce)
    );
    assert_eq!(hir.call_argument_expr(call_id, 0), Some(unary_expr));
    assert_eq!(hir.call_argument_expr(call_id, 1), Some(add_expr));
    assert_eq!(hir.call_argument_expr(call_id, 2), Some(coalesce_expr));
}

#[test]
fn expression_metadata_tracks_blocks_branches_indexes_and_members() {
    let source = r#"
            let block = { let value = 1; value };
            let choice = if flag { block } else { 2 };
            let picked = switch mode { 0 => [1, 2][0], _ => #{ value: 3 }.value };
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let block_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("{ let value = 1; value }").unwrap()).unwrap(),
        ))
        .expect("expected block expression");
    let if_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("if flag").unwrap()).unwrap(),
        ))
        .expect("expected if expression");
    let switch_expr = hir
        .expr_at_offset(TextSize::from(
            u32::try_from(source.find("switch mode").unwrap()).unwrap(),
        ))
        .expect("expected switch expression");
    let index_expr = hir
        .exprs
        .iter()
        .enumerate()
        .find_map(|(index, expr)| {
            (expr.kind == ExprKind::Index && slice_range(source, expr.range) == "[1, 2][0]")
                .then_some(crate::ExprId(index as u32))
        })
        .expect("expected index expression");
    let field_expr = hir
        .exprs
        .iter()
        .enumerate()
        .find_map(|(index, expr)| {
            (expr.kind == ExprKind::Field
                && slice_range(source, expr.range) == "#{ value: 3 }.value")
                .then_some(crate::ExprId(index as u32))
        })
        .expect("expected field expression");

    let block_info = hir.block_expr(block_expr).expect("expected block info");
    let tail_expr = hir
        .body_tail_value(block_info.body)
        .expect("expected block tail value");
    assert_eq!(hir.expr(tail_expr).kind, ExprKind::Name);

    let if_info = hir.if_expr(if_expr).expect("expected if info");
    assert_eq!(
        if_info.condition.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Name)
    );
    assert_eq!(
        if_info.then_branch.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Block)
    );
    assert_eq!(
        if_info.else_branch.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Block)
    );

    let switch_info = hir.switch_expr(switch_expr).expect("expected switch info");
    assert_eq!(switch_info.arms.len(), 2);
    assert_eq!(
        switch_info.arms[0].map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Index)
    );
    assert_eq!(
        switch_info.arms[1].map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Field)
    );
    let switch_arms = hir.switch_arms(switch_expr).collect::<Vec<_>>();
    assert_eq!(switch_arms.len(), 2);
    assert_eq!(switch_arms[0].patterns.len(), 1);
    assert!(!switch_arms[0].wildcard);
    assert_eq!(
        switch_arms[0].patterns[0],
        hir.expr_at_offset(TextSize::from(
            u32::try_from(source.find("0 =>").expect("expected first switch pattern"))
                .expect("pattern offset")
        ))
        .expect("expected pattern expr"),
    );
    assert!(switch_arms[1].wildcard);

    let index_info = hir.index_expr(index_expr).expect("expected index info");
    assert_eq!(
        index_info.receiver.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Array)
    );
    assert_eq!(
        index_info.index.map(|expr| hir.expr(expr).kind),
        Some(ExprKind::Literal)
    );

    let access = hir
        .member_access(field_expr)
        .expect("expected member access");
    assert_eq!(hir.expr(access.receiver).kind, ExprKind::Object);
    assert_eq!(hir.reference(access.field_reference).name, "value");
}

#[test]
fn value_flows_capture_initializers_and_assignments() {
    let parse = parse_valid(
        r#"
            fn bump(input) { input + 1 }

            let value = 1;
            value = bump(value);

            const LIMIT = 99;
        "#,
    );
    let hir = lower_file(&parse);

    let value_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `value` symbol");
    let limit_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "LIMIT" && symbol.kind == SymbolKind::Constant)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `LIMIT` symbol");

    let value_flows = hir.value_flows_into(value_symbol).collect::<Vec<_>>();
    assert_eq!(value_flows.len(), 2);
    assert_eq!(value_flows[0].kind, ValueFlowKind::Initializer);
    assert_eq!(hir.expr(value_flows[0].expr).kind, ExprKind::Literal);
    assert_eq!(value_flows[1].kind, ValueFlowKind::Assignment);
    assert_eq!(hir.expr(value_flows[1].expr).kind, ExprKind::Call);

    let limit_flows = hir.value_flows_into(limit_symbol).collect::<Vec<_>>();
    assert_eq!(limit_flows.len(), 1);
    assert_eq!(limit_flows[0].kind, ValueFlowKind::Initializer);
    assert_eq!(hir.expr(limit_flows[0].expr).kind, ExprKind::Literal);
}

#[test]
fn lowering_records_symbol_mutations_for_simple_field_and_index_assignments() {
    let parse = parse_valid(
        r#"
            let user = #{};
            user.name = "Ada";

            let items = [];
            items[0] = 1;

            let nested = #{};
            nested.profile.name = "ignored";
        "#,
    );
    let hir = lower_file(&parse);

    let user_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "user" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `user` symbol");
    let items_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "items" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `items` symbol");
    let nested_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "nested" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `nested` symbol");

    let user_mutations = hir.symbol_mutations_into(user_symbol).collect::<Vec<_>>();
    assert_eq!(user_mutations.len(), 1);
    assert_eq!(
        user_mutations[0].kind,
        SymbolMutationKind::Path {
            segments: vec![MutationPathSegment::Field {
                name: "name".to_owned(),
            }]
        }
    );
    assert_eq!(hir.expr(user_mutations[0].value).kind, ExprKind::Literal);

    let item_mutations = hir.symbol_mutations_into(items_symbol).collect::<Vec<_>>();
    assert_eq!(item_mutations.len(), 1);
    assert!(matches!(
        &item_mutations[0].kind,
        SymbolMutationKind::Path { segments }
            if matches!(segments.as_slice(), [MutationPathSegment::Index { .. }])
    ));
    assert_eq!(hir.expr(item_mutations[0].value).kind, ExprKind::Literal);

    assert!(
        hir.symbol_mutations_into(nested_symbol).any(|mutation| {
            mutation.kind
                == SymbolMutationKind::Path {
                    segments: vec![
                        MutationPathSegment::Field {
                            name: "profile".to_owned(),
                        },
                        MutationPathSegment::Field {
                            name: "name".to_owned(),
                        },
                    ],
                }
        }),
        "nested field chains should be recorded as path-aware mutations"
    );
}

#[test]
fn lowering_records_compound_assignments_with_assignment_metadata() {
    let parse = parse_valid(
        r#"
            let count = 1;
            count += 2;

            let obj = #{};
            obj.value ??= 3;

            let arr = [];
            arr[0] += 4;
        "#,
    );
    let hir = lower_file(&parse);

    let assign_exprs = hir.assign_exprs.iter().collect::<Vec<_>>();
    assert_eq!(assign_exprs.len(), 3);
    assert_eq!(assign_exprs[0].operator, crate::AssignmentOperator::Add);
    assert_eq!(
        assign_exprs[1].operator,
        crate::AssignmentOperator::NullCoalesce
    );
    assert_eq!(assign_exprs[2].operator, crate::AssignmentOperator::Add);

    let obj_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "obj" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `obj` symbol");
    let arr_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "arr" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `arr` symbol");

    assert!(hir.symbol_mutations_into(obj_symbol).any(|mutation| {
        mutation.kind
            == SymbolMutationKind::Path {
                segments: vec![MutationPathSegment::Field {
                    name: "value".to_owned(),
                }],
            }
            && hir.expr(mutation.value).kind == ExprKind::Assign
    }));
    assert!(hir.symbol_mutations_into(arr_symbol).any(|mutation| {
        matches!(
            &mutation.kind,
            SymbolMutationKind::Path { segments }
                if matches!(segments.as_slice(), [MutationPathSegment::Index { .. }])
        ) && hir.expr(mutation.value).kind == ExprKind::Assign
    }));
}

#[test]
fn lowering_records_mixed_member_and_index_mutation_paths() {
    let parse = parse_valid(
        r#"
            let root = #{};
            let slot = 0;
            root.items[slot].value += 1;
        "#,
    );
    let hir = lower_file(&parse);

    let root_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "root" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `root` symbol");

    assert!(hir.symbol_mutations_into(root_symbol).any(|mutation| {
        mutation.kind
            == SymbolMutationKind::Path {
                segments: vec![
                    MutationPathSegment::Field {
                        name: "items".to_owned(),
                    },
                    MutationPathSegment::Index {
                        index: hir
                            .exprs
                            .iter()
                            .enumerate()
                            .find_map(|(index, expr)| {
                                (expr.kind == ExprKind::Name
                                    && slice_range(parse.text(), expr.range) == "slot")
                                    .then_some(crate::ExprId(index as u32))
                            })
                            .expect("expected slot index expression"),
                    },
                    MutationPathSegment::Field {
                        name: "value".to_owned(),
                    },
                ],
            }
            && hir.expr(mutation.value).kind == ExprKind::Assign
    }));
}

#[test]
fn lowering_records_ordered_symbol_reads_for_field_and_index_accesses() {
    let parse = parse_valid(
        r#"
            let root = #{};
            let slot = 0;
            let value = root.items[slot].value;
        "#,
    );
    let hir = lower_file(&parse);

    let root_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "root" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `root` symbol");

    let reads = hir.symbol_reads_into(root_symbol).collect::<Vec<_>>();
    assert_eq!(reads.len(), 3);
    assert!(reads.iter().any(|read| {
        matches!(
            &read.kind,
            SymbolReadKind::Path { segments }
                if matches!(
                    segments.as_slice(),
                    [MutationPathSegment::Field { name }] if name == "items"
                )
        )
    }));
    assert!(reads.iter().any(|read| {
        matches!(
            &read.kind,
            SymbolReadKind::Path { segments }
                if matches!(
                    segments.as_slice(),
                    [
                        MutationPathSegment::Field { name },
                        MutationPathSegment::Index { .. }
                    ] if name == "items"
                )
        )
    }));
    assert!(reads.iter().any(|read| {
        matches!(
            &read.kind,
            SymbolReadKind::Path { segments }
                if matches!(
                    segments.as_slice(),
                    [
                        MutationPathSegment::Field { name: first },
                        MutationPathSegment::Index { .. },
                        MutationPathSegment::Field { name: last },
                    ] if first == "items" && last == "value"
                )
        )
    }));
}

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
