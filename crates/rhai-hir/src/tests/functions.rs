use crate::tests::parse_valid;
use crate::{
    FunctionTypeRef, ReferenceKind, SemanticDiagnosticKind, SymbolKind, TypeRef, lower_file,
};
use rhai_syntax::TextSize;

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
