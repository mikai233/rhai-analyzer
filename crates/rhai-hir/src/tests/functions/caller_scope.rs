use crate::tests::parse_valid;
use crate::{SymbolKind, lower_file};
use rhai_syntax::TextSize;

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
fn caller_scope_fn_pointer_dispatch_resolves_local_function_target() {
    let parse = parse_valid(
        r#"
            fn echo(value) {
                value
            }

            call!(Fn("echo"), 1);
        "#,
    );
    let hir = lower_file(&parse);

    let echo = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.kind == SymbolKind::Function && symbol.name == "echo")
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected echo symbol");

    assert_eq!(hir.calls.len(), 2);
    let outer = hir
        .calls
        .iter()
        .max_by_key(|call| call.range.len())
        .expect("expected outer caller-scope call");
    assert_eq!(outer.resolved_callee, Some(echo));
    assert_eq!(outer.parameter_bindings.len(), 2);
    assert_eq!(outer.parameter_bindings[0], None);
}
