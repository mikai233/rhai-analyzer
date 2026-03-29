use crate::{
    BinaryOperator, BodyKind, ControlFlowKind, ExprKind, ExternalSignatureIndex, FunctionTypeRef,
    LiteralKind, MemberCompletionSource, MergePointKind, MutationPathSegment, ReferenceKind,
    ScopeKind, SemanticDiagnosticKind, SymbolKind, SymbolMutationKind, TypeRef, UnaryOperator,
    ValueFlowKind, lower_file,
};
use rhai_syntax::{TextRange, TextSize, parse_text};

fn slice_range(source: &str, range: TextRange) -> &str {
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    &source[start as usize..end as usize]
}

fn parse_valid(source: &str) -> rhai_syntax::Parse {
    let parse = parse_text(source);
    assert!(
        parse.errors().is_empty(),
        "expected valid Rhai syntax, got errors: {:?}",
        parse.errors()
    );
    parse
}

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
fn type_query_helpers_support_external_signatures_and_slot_assignments() {
    let source = r#"
            fn helper(value) { value }
            let result = helper(1);
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
    let call_expr_offset =
        TextSize::from(u32::try_from(source.find("helper(1)").unwrap()).unwrap());
    let call_id = crate::CallSiteId(0);
    let call_expr = hir
        .expr_at_offset(call_expr_offset)
        .expect("expected call expression");

    let mut external = ExternalSignatureIndex::default();
    external.insert(
        "helper",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Bool),
        }),
    );

    assert_eq!(
        hir.effective_symbol_type(helper_symbol, Some(&external)),
        Some(TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Bool),
        }))
    );

    let signature = hir
        .call_signature(call_id, Some(&external))
        .expect("expected call signature");
    assert_eq!(signature.params, vec![TypeRef::Int]);
    assert_eq!(*signature.ret, TypeRef::Bool);

    let mut assignments = hir.new_type_slot_assignments();
    assignments.set(hir.expr_result_slot(call_expr), TypeRef::Bool);
    assert_eq!(hir.expr_type(call_expr, &assignments), Some(&TypeRef::Bool));
    assert_eq!(
        hir.expr_type_at_offset(call_expr_offset, &assignments),
        Some(&TypeRef::Bool)
    );
}

#[test]
fn call_signature_falls_back_to_external_names_for_unresolved_builtin_calls() {
    let source = r#"
            let bytes = blob(10);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);
    let call_id = crate::CallSiteId(0);

    let mut external = ExternalSignatureIndex::default();
    external.insert(
        "blob",
        TypeRef::Function(FunctionTypeRef {
            params: vec![TypeRef::Int],
            ret: Box::new(TypeRef::Blob),
        }),
    );

    let signature = hir
        .call_signature(call_id, Some(&external))
        .expect("expected call signature for builtin function");
    assert_eq!(signature.params, vec![TypeRef::Int]);
    assert_eq!(*signature.ret, TypeRef::Blob);
}

#[test]
fn file_symbol_index_exposes_indexable_symbols_with_container_and_export_metadata() {
    let parse = parse_valid(
        r#"
            const LIMIT = 1;

            fn outer() {}
            {
                let local = 1;
            }

            import "crypto" as secure;
            let exported_outer = LIMIT;
            export exported_outer as public_outer;
        "#,
    );
    let hir = lower_file(&parse);

    let index = hir.file_symbol_index();
    let names = index
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"LIMIT"));
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"secure"));
    assert!(names.contains(&"public_outer"));
    assert!(!names.contains(&"local"));

    let outer = index
        .entries
        .iter()
        .find(|entry| entry.name == "outer")
        .expect("expected outer entry");
    assert!(outer.exported);
    assert!(outer.container_name.is_none());

    let public_outer = index
        .entries
        .iter()
        .find(|entry| entry.name == "public_outer")
        .expect("expected public export alias entry");
    assert!(public_outer.exported);
}

#[test]
fn file_backed_symbol_identity_captures_container_path_and_export_status() {
    let parse = parse_valid(
        r#"
            fn outer(arg) {
                let local = arg;
            }

            private fn hidden() {}
            let exported_value = 1;
            export exported_value as public_outer;
        "#,
    );
    let hir = lower_file(&parse);

    let outer = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "outer" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `outer` symbol");
    let arg = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "arg" && symbol.kind == SymbolKind::Parameter)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `arg` symbol");
    let hidden = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "hidden" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `hidden` symbol");

    let outer_identity = hir.file_backed_symbol_identity(outer);
    let arg_identity = hir.file_backed_symbol_identity(arg);
    let hidden_identity = hir.file_backed_symbol_identity(hidden);

    assert!(outer_identity.exported);
    assert!(outer_identity.container_path.is_empty());
    assert_eq!(outer_identity.stable_key.name, "outer");
    assert_eq!(outer_identity.stable_key.ordinal, 0);
    assert_eq!(arg_identity.container_path, vec!["outer"]);
    assert!(!arg_identity.exported);
    assert!(!hidden_identity.exported);
}

#[test]
fn stable_symbol_keys_distinguish_duplicate_indexable_symbols() {
    let parse = parse_valid(
        r#"
            const inner = 1;
            const inner = 2;
        "#,
    );
    let hir = lower_file(&parse);

    let inner_keys = hir
        .workspace_symbols()
        .into_iter()
        .filter(|symbol| symbol.name == "inner")
        .map(|symbol| symbol.stable_key.ordinal)
        .collect::<Vec<_>>();

    assert_eq!(inner_keys, vec![0, 1]);
}

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
        Some(crate::ModuleSpecifier::Text(ref text)) if text == "\"crypto\""
    ));
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
        Some(crate::ModuleSpecifier::LocalSymbol(ref symbol)) if symbol.name == "module_name"
    ));
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
fn editable_rename_occurrence_classifies_definitions_and_references_only() {
    let source = r#"
            fn helper() {}
            let value = helper();
            let text = "helper";
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let def_offset = TextSize::from(u32::try_from(source.find("helper").unwrap()).unwrap());
    let ref_offset = TextSize::from(u32::try_from(source.find("helper();").unwrap()).unwrap());
    let string_offset =
        TextSize::from(u32::try_from(source.find("\"helper\"").unwrap() + 1).unwrap());

    let def_occurrence = hir
        .editable_rename_occurrence_at(def_offset)
        .expect("expected definition occurrence");
    let ref_occurrence = hir
        .editable_rename_occurrence_at(ref_offset)
        .expect("expected reference occurrence");

    assert_eq!(def_occurrence.kind, crate::RenameOccurrenceKind::Definition);
    assert_eq!(ref_occurrence.kind, crate::RenameOccurrenceKind::Reference);
    assert!(hir.editable_rename_occurrence_at(string_offset).is_none());
}

#[test]
fn rename_plan_tracks_occurrences_aliases_and_preflight_issues() {
    let parse = parse_valid(
        r#"
            let taken = 0;
            let helper = 1;
            import helper as helper_alias;
            export helper as public_api;

            {
                let public_helper = 1;
                helper;
            }
        "#,
    );
    let hir = lower_file(&parse);

    let helper = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "helper" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `helper` symbol");

    let plan = hir.rename_plan(helper, "public_helper");

    assert_eq!(plan.target.name, "helper");
    assert_eq!(plan.new_name, "public_helper");
    assert_eq!(
        plan.occurrences
            .iter()
            .filter(|occurrence| occurrence.kind == crate::RenameOccurrenceKind::Definition)
            .count(),
        1
    );
    assert_eq!(
        plan.occurrences
            .iter()
            .filter(|occurrence| occurrence.kind == crate::RenameOccurrenceKind::Reference)
            .count(),
        3
    );
    assert_eq!(plan.linked_aliases.len(), 2);
    assert!(plan.linked_aliases.iter().any(|alias| {
        alias.kind == crate::LinkedAliasKind::ImportAlias && alias.symbol.name == "helper_alias"
    }));
    assert!(plan.linked_aliases.iter().any(|alias| {
        alias.kind == crate::LinkedAliasKind::ExportAlias && alias.symbol.name == "public_api"
    }));
    assert!(plan.issues.iter().any(|issue| {
        issue.kind == crate::RenamePreflightIssueKind::ReferenceCollision
            && issue
                .related_symbol
                .as_ref()
                .map(|symbol| symbol.name.as_str())
                == Some("public_helper")
    }));
    assert!(
        !plan
            .issues
            .iter()
            .any(|issue| issue.kind == crate::RenamePreflightIssueKind::DuplicateDefinition)
    );
}

#[test]
fn rename_preflight_reports_duplicate_definitions_in_same_scope() {
    let parse = parse_valid(
        r#"
            let taken = 1;
            let value = 2;
        "#,
    );
    let hir = lower_file(&parse);

    let value = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `value` symbol");

    let plan = hir.rename_plan(value, "taken");
    assert!(plan.issues.iter().any(|issue| {
        issue.kind == crate::RenamePreflightIssueKind::DuplicateDefinition
            && issue
                .related_symbol
                .as_ref()
                .map(|symbol| symbol.name.as_str())
                == Some("taken")
    }));
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
fn completion_symbols_follow_visible_scope_and_preserve_metadata() {
    let source = r#"
            /// helper docs
            /// @param arg int
            /// @return int
            fn helper(arg) {
                let before = arg;
                {
                    /// local docs
                    /// @type string
                    let value = before;
                    value + arg
                }
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);
    let value_use_offset =
        TextSize::from(u32::try_from(source.rfind("value + arg").unwrap()).unwrap());

    let completions = hir.completion_symbols_at(value_use_offset);
    let names = completions
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["value", "before", "arg", "helper"]);

    let value_completion = completions
        .iter()
        .find(|item| item.name == "value")
        .expect("expected local value completion");
    assert_eq!(value_completion.kind, SymbolKind::Variable);
    assert_eq!(value_completion.annotation, Some(TypeRef::String));
    assert!(value_completion.docs.is_some());

    let helper_completion = completions
        .iter()
        .find(|item| item.name == "helper")
        .expect("expected helper completion");
    assert_eq!(helper_completion.kind, SymbolKind::Function);
    assert!(helper_completion.docs.is_some());
    assert!(matches!(
        helper_completion.annotation,
        Some(TypeRef::Function(_))
    ));
}

#[test]
fn member_completion_uses_doc_fields_and_object_literal_shapes() {
    let source = r#"
            /// @field name string
            /// @field age int
            let user = #{ name: "Ada", age: 42 };

            let temp = #{ enabled: true, count: 1 };

            user.name;
            temp.enabled;
        "#;
    let parse = parse_valid(source);
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
    let documented_fields = hir.documented_fields(user_symbol);
    assert_eq!(documented_fields.len(), 2);
    assert_eq!(documented_fields[0].name, "name");
    assert_eq!(documented_fields[0].annotation, TypeRef::String);
    assert_eq!(documented_fields[1].name, "age");
    assert_eq!(documented_fields[1].annotation, TypeRef::Int);

    let user_member_offset = TextSize::from(u32::try_from(source.rfind("name;").unwrap()).unwrap());
    let user_members = hir.member_completion_at(user_member_offset);
    assert_eq!(
        user_members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        vec!["age", "name"]
    );
    assert!(user_members.iter().all(|member| {
        member.source == MemberCompletionSource::DocumentedField && member.annotation.is_some()
    }));

    let temp_member_offset =
        TextSize::from(u32::try_from(source.rfind("enabled;").unwrap()).unwrap());
    let temp_members = hir.member_completion_at(temp_member_offset);
    assert_eq!(
        temp_members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        vec!["count", "enabled"]
    );
    assert!(temp_members.iter().all(|member| {
        member.source == MemberCompletionSource::ObjectLiteralField && member.range.is_some()
    }));
}

#[test]
fn parameter_hints_follow_resolved_function_calls() {
    let source = r#"
            /// @param left int
            /// @param right string
            /// @return bool
            fn check(left, right) {
                left == right
            }

            let result = check(1, value);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let first_arg_offset = TextSize::from(u32::try_from(source.find("1, value").unwrap()).unwrap());
    let second_arg_offset = TextSize::from(u32::try_from(source.find("value);").unwrap()).unwrap());

    let first_hint = hir
        .parameter_hint_at(first_arg_offset)
        .expect("expected parameter hint on first argument");
    assert_eq!(first_hint.callee_name, "check");
    assert_eq!(first_hint.callee.kind, SymbolKind::Function);
    assert_eq!(first_hint.active_parameter, 0);
    assert_eq!(first_hint.parameters.len(), 2);
    assert_eq!(first_hint.parameters[0].name, "left");
    assert_eq!(first_hint.parameters[0].annotation, Some(TypeRef::Int));
    assert_eq!(first_hint.parameters[1].name, "right");
    assert_eq!(first_hint.parameters[1].annotation, Some(TypeRef::String));
    assert_eq!(first_hint.return_type, Some(TypeRef::Bool));

    let second_hint = hir
        .parameter_hint_at(second_arg_offset)
        .expect("expected parameter hint on second argument");
    assert_eq!(second_hint.call, first_hint.call);
    assert_eq!(second_hint.active_parameter, 1);

    let call = hir.call(first_hint.call);
    let callee = call.resolved_callee.expect("expected resolved callee");
    assert_eq!(callee, first_hint.callee.symbol);
    assert_eq!(
        hir.call_parameter_binding(first_hint.call, 0),
        first_hint.parameters[0].symbol
    );
    assert_eq!(
        hir.call_parameter_binding(first_hint.call, 1),
        first_hint.parameters[1].symbol
    );
}

#[test]
fn import_alias_calls_retain_resolved_callee_without_local_parameter_bindings() {
    let parse = parse_valid(
        r#"
            import shared_tools as tools;

            tools(1);
        "#,
    );
    let hir = lower_file(&parse);

    let call = hir.calls.first().expect("expected call");
    let alias = hir
        .imports
        .first()
        .and_then(|import| import.alias)
        .expect("expected import alias");

    assert_eq!(call.resolved_callee, Some(alias));
    assert_eq!(hir.symbol(alias).kind, SymbolKind::ImportAlias);
    assert_eq!(call.parameter_bindings, vec![None]);
}

#[test]
fn caller_scope_calls_record_caller_scope_metadata() {
    let parse = parse_valid(
        r#"
            fn helper(value) {
                value
            }

            helper!(1);
            call!(helper, 2);
        "#,
    );
    let hir = lower_file(&parse);

    assert_eq!(hir.calls.len(), 2);
    assert!(hir.calls.iter().all(|call| call.caller_scope));
    assert_eq!(hir.calls[0].parameter_bindings.len(), 1);
    assert_eq!(hir.calls[1].parameter_bindings.len(), 2);
    assert_eq!(hir.calls[1].resolved_callee, hir.calls[0].resolved_callee);
    assert_eq!(hir.calls[1].parameter_bindings[0], None);
}

#[test]
fn caller_scope_parameter_hints_skip_the_dispatch_argument() {
    let source = r#"
            /// @param value int
            fn helper(value) {
                value
            }

            call!(helper, answer);
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let helper_offset = TextSize::from(u32::try_from(source.find("helper,").unwrap()).unwrap());
    let answer_offset = TextSize::from(u32::try_from(source.find("answer);").unwrap()).unwrap());

    assert!(hir.parameter_hint_at(helper_offset).is_none());

    let hint = hir
        .parameter_hint_at(answer_offset)
        .expect("expected parameter hint on caller-scope argument");
    assert_eq!(hint.callee_name, "helper");
    assert_eq!(hint.active_parameter, 0);
    assert_eq!(hint.parameters.len(), 1);
    assert_eq!(hint.parameters[0].name, "value");
}

#[test]
fn symbol_reverse_references_follow_scope_resolution() {
    let parse = parse_valid(
        r#"
            let value = 1;
            {
                let value = 2;
                value;
            }
            value;
        "#,
    );

    let hir = lower_file(&parse);
    let value_symbols: Vec<_> = hir
        .symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol.name == "value" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .collect();
    assert_eq!(value_symbols.len(), 2);

    let outer_refs = hir.references_to(value_symbols[0]).collect::<Vec<_>>();
    let inner_refs = hir.references_to(value_symbols[1]).collect::<Vec<_>>();
    assert_eq!(outer_refs.len(), 1);
    assert_eq!(inner_refs.len(), 1);

    let outer_ref = hir.reference(outer_refs[0]);
    let inner_ref = hir.reference(inner_refs[0]);
    assert!(outer_ref.range.start() > inner_ref.range.start());
}

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
fn lowering_models_this_as_dedicated_reference_kind() {
    let parse = parse_valid(
        r#"
            fn sample() {
                this.value;
                this;
            }
        "#,
    );
    let hir = lower_file(&parse);

    let this_refs = hir
        .references
        .iter()
        .filter(|reference| reference.kind == ReferenceKind::This)
        .collect::<Vec<_>>();
    assert_eq!(this_refs.len(), 2);
    assert!(this_refs.iter().all(|reference| reference.name == "this"));
}

#[test]
fn query_exposes_this_type_inside_function_contexts() {
    let source = r#"
            fn int.bump(delta) {
                this + delta
            }

            fn show() {
                this
            }
        "#;
    let parse = parse_valid(source);
    let hir = lower_file(&parse);

    let typed_offset =
        TextSize::from(u32::try_from(source.find("this +").expect("expected typed this")).unwrap());
    let blanket_offset = TextSize::from(
        u32::try_from(source.rfind("this").expect("expected blanket this")).unwrap(),
    );

    assert_eq!(hir.this_type_at(typed_offset), Some(TypeRef::Int));
    assert_eq!(hir.this_type_at(blanket_offset), Some(TypeRef::Unknown));
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
fn document_and_workspace_symbol_apis_expose_indexing_handoff() {
    let parse = parse_valid(
        r#"
            fn outer() {}

            const LIMIT = 1;
            let exported_limit = LIMIT;
            export exported_limit as public_outer;
        "#,
    );
    let hir = lower_file(&parse);

    let document_symbols = hir.document_symbols();
    assert_eq!(
        document_symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["outer", "LIMIT", "exported_limit", "public_outer"]
    );
    assert!(document_symbols[0].children.is_empty());

    let workspace_symbols = hir.workspace_symbols();
    assert!(
        workspace_symbols
            .iter()
            .all(|symbol| !symbol.stable_key.name.is_empty())
    );

    let handoff = hir.indexing_handoff();
    assert_eq!(handoff.file_symbols.entries.len(), workspace_symbols.len());
    assert_eq!(handoff.workspace_symbols.len(), workspace_symbols.len());
    assert_eq!(handoff.module_graph.exports.len(), 2);
}

#[test]
fn project_completion_symbols_filter_out_visible_locals() {
    let parse = parse_valid(
        r#"
            fn helper() {}
            fn use_it() {
                helper();
            }
        "#,
    );
    let hir = lower_file(&parse);
    let offset = TextSize::from(
        u32::try_from(
            r#"
            fn helper() {}
            fn use_it() {
                helper();
            }
        "#
            .find("helper();")
            .unwrap(),
        )
        .unwrap(),
    );

    let project_parse = parse_valid(
        r#"
            fn helper() {}
            fn external_api() {}
        "#,
    );
    let project_symbols = lower_file(&project_parse).workspace_symbols();
    let completions = hir.project_completion_symbols_at(offset, &project_symbols);

    assert_eq!(
        completions
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["external_api"]
    );
}

#[test]
fn semantic_diagnostics_report_unresolved_names() {
    let parse = parse_valid(
        r#"
            fn sample() {
                missing_name;
                this;
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, SemanticDiagnosticKind::UnresolvedName);
    assert_eq!(diagnostics[0].message, "unresolved name `missing_name`");
    assert!(diagnostics[0].related_range.is_none());
}

#[test]
fn semantic_diagnostics_report_duplicate_definitions_in_same_scope() {
    let parse = parse_valid(
        r#"
            let value = 1;
            let value = 2;

            fn sample(arg, arg) {
                let local = 1;
                let local = 2;
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::DuplicateDefinition)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 3);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `value`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `arg`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate definition of `local`")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
}

#[test]
fn semantic_diagnostics_report_unresolved_imports_and_exports() {
    let parse = parse_valid(
        r#"
            import missing_module as secure;
            export missing_value as exposed;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let import_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedImport)
        .collect::<Vec<_>>();
    let export_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedExport)
        .collect::<Vec<_>>();
    let unresolved_name_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedName)
        .collect::<Vec<_>>();

    assert_eq!(import_diagnostics.len(), 1);
    assert_eq!(
        import_diagnostics[0].message,
        "unresolved import module `missing_module`"
    );
    assert!(import_diagnostics[0].related_range.is_some());

    assert_eq!(export_diagnostics.len(), 1);
    assert_eq!(
        export_diagnostics[0].message,
        "unresolved export target `missing_value`"
    );
    assert!(export_diagnostics[0].related_range.is_some());

    assert!(unresolved_name_diagnostics.is_empty());
    assert_eq!(hir.imports.len(), 1);
    assert_eq!(hir.exports.len(), 1);
}

#[test]
fn semantic_diagnostics_reject_explicit_function_exports() {
    let parse = parse_valid(
        r#"
            fn helper() {}
            export helper as public_helper;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let invalid_export_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InvalidExportTarget)
        .collect::<Vec<_>>();

    assert_eq!(invalid_export_diagnostics.len(), 1);
    assert_eq!(
        invalid_export_diagnostics[0].message,
        "export target `helper` must refer to a global variable or constant"
    );
    assert!(invalid_export_diagnostics[0].related_range.is_some());
}

#[test]
fn semantic_diagnostics_reject_non_string_import_expressions() {
    let parse = parse_valid(
        r#"
            fn helper() {}
            let module_name = 1;
            const valid_module = "crypto";
            const prefix = "crypt";
            const suffix = "o";
            const block_module = { "crypto" };
            const conditional_module = if true { "crypto" } else { "hash" };

            import helper as bad_helper;
            import module_name as bad_value;
            import valid_module as ok_module;
            import prefix + suffix as ok_concat;
            import block_module as ok_block;
            import conditional_module as ok_conditional;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let invalid_import_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InvalidImportModuleType)
        .collect::<Vec<_>>();

    assert_eq!(invalid_import_diagnostics.len(), 2);
    assert!(invalid_import_diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "import module expression `helper` must evaluate to string, found function"
    }));
    assert!(invalid_import_diagnostics.iter().any(|diagnostic| {
        diagnostic.message
            == "import module expression `module_name` must evaluate to string, found int"
    }));
    assert!(
        invalid_import_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("prefix + suffix")
            || diagnostic.message.contains("block_module")
            || diagnostic.message.contains("conditional_module")
    }));
}

#[test]
fn semantic_diagnostics_reject_function_access_to_external_scope() {
    let parse = parse_valid(
        r#"
            let value = 42;

            fn helper() {
                value
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    let unresolved = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnresolvedName)
        .collect::<Vec<_>>();

    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].message, "unresolved name `value`");
}

#[test]
fn semantic_diagnostics_allow_global_import_aliases_inside_functions() {
    let parse = parse_valid(
        r#"
            import "hello" as hey;

            fn helper(value) {
                hey::process(value);
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir.diagnostics();

    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == SemanticDiagnosticKind::UnresolvedName
            && diagnostic.message == "unresolved name `hey`"
    }));

    let hey_reference = hir
        .references
        .iter()
        .find(|reference| reference.name == "hey")
        .expect("expected `hey` reference");
    let target = hey_reference
        .target
        .expect("expected resolved import alias");
    assert_eq!(hir.symbol(target).kind, SymbolKind::ImportAlias);
}

#[test]
fn semantic_diagnostics_report_unused_symbols() {
    let parse = parse_valid(
        r#"
            import "crypto" as secure;
            const KEPT = 1;

            fn sample(arg, _ignored) {
                let local = 1;
                let kept = arg + KEPT;
                kept;
            }

            sample(KEPT);
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::UnusedSymbol)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unused symbol `secure`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unused symbol `local`")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("_ignored"))
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("KEPT"))
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("arg"))
    );
}

#[test]
fn semantic_diagnostics_report_inconsistent_function_doc_types() {
    let parse = parse_valid(
        r#"
            /// @type int
            /// @param first int
            /// @param first string
            /// @param missing bool
            /// @return int
            /// @return string
            fn sample(first) {
                first
            }
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InconsistentDocType)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 4);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate `@param` tag for `first`")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate `@return` tags")
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "doc tag `@param missing` does not match any parameter of `sample`"
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "function `sample` has a non-function type annotation"
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.related_range.is_some())
    );
}

#[test]
fn semantic_diagnostics_report_function_doc_tags_on_non_functions() {
    let parse = parse_valid(
        r#"
            /// @param value int
            /// @return int
            let count = 1;
        "#,
    );
    let hir = lower_file(&parse);
    let diagnostics = hir
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == SemanticDiagnosticKind::InconsistentDocType)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "function doc tags cannot be attached to `count`"
    );
    assert!(diagnostics[0].related_range.is_some());
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
