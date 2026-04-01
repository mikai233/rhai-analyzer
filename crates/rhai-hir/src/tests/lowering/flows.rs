use crate::tests::{parse_valid, slice_range};
use crate::{
    ExprKind, MutationPathSegment, SymbolKind, SymbolMutationKind, SymbolReadKind, ValueFlowKind,
    lower_file,
};

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
