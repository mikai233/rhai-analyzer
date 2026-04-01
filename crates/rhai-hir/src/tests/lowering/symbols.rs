use crate::tests::{parse_valid, slice_range};
use crate::{
    BinaryOperator, BodyKind, ExprKind, LiteralKind, ReferenceKind, ScopeKind, UnaryOperator,
    lower_file,
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
