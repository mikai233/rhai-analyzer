use crate::tests::parse_valid;
use crate::{ExternalSignatureIndex, FunctionTypeRef, SymbolKind, TypeRef, lower_file};
use rhai_syntax::TextSize;

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
