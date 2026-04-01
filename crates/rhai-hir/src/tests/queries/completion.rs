use crate::tests::parse_valid;
use crate::{MemberCompletionSource, SymbolKind, TypeRef, lower_file};
use rhai_syntax::TextSize;

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
            /// @field name string Primary display name
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
    assert_eq!(
        documented_fields[0].docs.as_deref(),
        Some("Primary display name")
    );
    assert_eq!(documented_fields[1].name, "age");
    assert_eq!(documented_fields[1].annotation, TypeRef::Int);
    assert_eq!(documented_fields[1].docs, None);

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
    let name_member = user_members
        .iter()
        .find(|member| member.name == "name")
        .expect("expected documented name member");
    assert_eq!(name_member.docs.as_deref(), Some("Primary display name"));

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
