use crate::tests::{parse_valid, slice_range};
use crate::{
    BodyKind, ExternalSignatureIndex, FunctionTypeRef, MemberCompletionSource, ScopeKind,
    SymbolKind, TypeRef, lower_file,
};
use rhai_syntax::TextSize;

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
