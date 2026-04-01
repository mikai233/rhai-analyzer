use crate::tests::parse_valid;
use crate::{ExpectedTypeSource, ReferenceKind, SymbolKind, TypeRef, lower_file};
use rhai_syntax::TextSize;

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
fn expected_type_sites_capture_symbol_call_and_return_sources() {
    let parse = parse_valid(
        r#"
            /// @param callback fun(int) -> int
            fn apply(callback) {
                callback(1)
            }

            /// @type fun(int) -> int
            let mapper = |value| value + 1;
            let result = apply(mapper);

            /// @return fun(int) -> int
            fn make_mapper() {
                |item| item + 1
            }
        "#,
    );
    let hir = lower_file(&parse);

    let mapper_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "mapper" && symbol.kind == SymbolKind::Variable)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `mapper` symbol");
    let make_mapper_symbol = hir
        .symbols
        .iter()
        .enumerate()
        .find_map(|(index, symbol)| {
            (symbol.name == "make_mapper" && symbol.kind == SymbolKind::Function)
                .then_some(crate::SymbolId(index as u32))
        })
        .expect("expected `make_mapper` symbol");
    let apply_call = hir
        .calls
        .iter()
        .enumerate()
        .find_map(|(index, call)| {
            call.callee_reference
                .map(|reference| hir.reference(reference).name.as_str())
                .filter(|name| *name == "apply")
                .map(|_| crate::CallSiteId(index as u32))
        })
        .expect("expected `apply` call");

    assert!(hir.expected_type_sites.iter().any(|site| {
        hir.expr(site.expr).kind == crate::ExprKind::Closure
            && site.source == ExpectedTypeSource::Symbol(mapper_symbol)
    }));
    assert!(hir.expected_type_sites.iter().any(|site| {
        hir.expr(site.expr).kind == crate::ExprKind::Name
            && site.source
                == ExpectedTypeSource::CallArgument {
                    call: apply_call,
                    parameter_index: 0,
                }
    }));
    assert!(hir.expected_type_sites.iter().any(|site| {
        hir.expr(site.expr).kind == crate::ExprKind::Closure
            && site.source == ExpectedTypeSource::FunctionReturn(make_mapper_symbol)
    }));
}
