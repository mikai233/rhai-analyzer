use crate::CompletionItemSource;
use crate::tests::completion::members::{completions_at, load_analysis, member_completion};

#[test]
fn completions_include_builtin_string_members() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let text = "hello";
                text.
                helper();
            }

            fn helper() {}
        "#,
    );

    let offset =
        u32::try_from(text.find("text.").expect("expected string member access") + "text.".len())
            .expect("offset");

    let completions = completions_at(&analysis, file_id, offset);
    member_completion(&completions, "contains");
    member_completion(&completions, "is_shared");
    member_completion(&completions, "len");
}

#[test]
fn completions_include_builtin_array_members_for_incomplete_trailing_dot_access() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let a = [1, 2, 3];
                a.
                next();
            }

            fn next() {}
        "#,
    );

    let offset = u32::try_from(text.find("a.").expect("expected array member access") + "a.".len())
        .expect("offset");

    let completions = completions_at(&analysis, file_id, offset);
    assert!(
        completions
            .iter()
            .any(|item| item.label == "len" && item.source == CompletionItemSource::Member),
        "expected array len member completion, got {completions:?}"
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "push" && item.source == CompletionItemSource::Member),
        "expected array push member completion, got {completions:?}"
    );
}

#[test]
fn completions_merge_object_fields_with_builtin_map_members() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let user = #{ name: "Ada" };
                user.
                helper();
            }

            fn helper() {}
        "#,
    );

    let offset =
        u32::try_from(text.find("user.").expect("expected object member access") + "user.".len())
            .expect("offset");

    let completions = completions_at(&analysis, file_id, offset);
    member_completion(&completions, "name");
    member_completion(&completions, "keys");
    member_completion(&completions, "values");
}

#[test]
fn completions_include_builtin_primitive_members() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let count = 1;
                count.
                next();

                let ratio = 3.14;
                ratio.
                next();

                let initial = 'a';
                initial.
                next();
            }

            fn next() {}
        "#,
    );

    let int_offset =
        u32::try_from(text.find("count.").expect("expected int member access") + "count.".len())
            .expect("offset");
    let int_completions = completions_at(&analysis, file_id, int_offset);
    member_completion(&int_completions, "is_odd");
    member_completion(&int_completions, "to_float");

    let float_offset =
        u32::try_from(text.find("ratio.").expect("expected float member access") + "ratio.".len())
            .expect("offset");
    let float_completions = completions_at(&analysis, file_id, float_offset);
    member_completion(&float_completions, "floor");
    member_completion(&float_completions, "to_int");

    let char_offset = u32::try_from(
        text.find("initial.").expect("expected char member access") + "initial.".len(),
    )
    .expect("offset");
    let char_completions = completions_at(&analysis, file_id, char_offset);
    member_completion(&char_completions, "to_upper");
    member_completion(&char_completions, "to_int");
}

#[test]
fn completions_include_builtin_members_for_literal_receivers() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                "hello".
                next();
            }

            fn next() {}
        "#,
    );
    let string_offset = u32::try_from(
        text.find("\"hello\".")
            .expect("expected string member access")
            + "\"hello\".".len(),
    )
    .expect("offset");
    let string_completions = completions_at(&analysis, file_id, string_offset);
    member_completion(&string_completions, "contains");
    member_completion(&string_completions, "len");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                [1, 2, 3].
                next();
            }

            fn next() {}
        "#,
    );
    let array_offset = u32::try_from(
        text.find("[1, 2, 3].")
            .expect("expected array literal member access")
            + "[1, 2, 3].".len(),
    )
    .expect("offset");
    let array_completions = completions_at(&analysis, file_id, array_offset);
    member_completion(&array_completions, "push");
    member_completion(&array_completions, "len");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                #{ name: "Ada" }.
                next();
            }

            fn next() {}
        "#,
    );
    let map_offset = u32::try_from(
        text.find("#{ name: \"Ada\" }.")
            .expect("expected object literal member access")
            + "#{ name: \"Ada\" }.".len(),
    )
    .expect("offset");
    let map_completions = completions_at(&analysis, file_id, map_offset);
    member_completion(&map_completions, "name");
    member_completion(&map_completions, "keys");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                blob(4, 0).
                next();
            }

            fn next() {}
        "#,
    );
    let blob_offset = u32::try_from(
        text.find("blob(4, 0).")
            .expect("expected blob member access")
            + "blob(4, 0).".len(),
    )
    .expect("offset");
    let blob_completions = completions_at(&analysis, file_id, blob_offset);
    member_completion(&blob_completions, "len");
    member_completion(&blob_completions, "push");
}

#[test]
fn completions_include_builtin_members_for_expression_receivers() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                ("hello").
                next();
            }

            fn next() {}
        "#,
    );
    let grouped_offset = u32::try_from(
        text.find("(\"hello\").")
            .expect("expected grouped string member access")
            + "(\"hello\").".len(),
    )
    .expect("offset");
    let grouped_completions = completions_at(&analysis, file_id, grouped_offset);
    member_completion(&grouped_completions, "contains");
    member_completion(&grouped_completions, "len");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn make_text() {
                "hello"
            }

            fn run() {
                make_text().
                next();
            }

            fn next() {}
        "#,
    );
    let call_offset = u32::try_from(
        text.find("make_text().")
            .expect("expected call receiver member access")
            + "make_text().".len(),
    )
    .expect("offset");
    let call_completions = completions_at(&analysis, file_id, call_offset);
    member_completion(&call_completions, "contains");
    member_completion(&call_completions, "len");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run(flag) {
                (if flag { "left" } else { "right" }).
                next();
            }

            fn next() {}
        "#,
    );
    let conditional_offset = u32::try_from(
        text.find("(if flag { \"left\" } else { \"right\" }).")
            .expect("expected conditional receiver member access")
            + "(if flag { \"left\" } else { \"right\" }).".len(),
    )
    .expect("offset");
    let conditional_completions = completions_at(&analysis, file_id, conditional_offset);
    member_completion(&conditional_completions, "contains");
    member_completion(&conditional_completions, "len");
}

#[test]
fn completions_include_builtin_members_for_chained_and_indexed_receivers() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                "hello".to_upper().
                next();
            }

            fn next() {}
        "#,
    );
    let chained_offset = u32::try_from(
        text.find("\"hello\".to_upper().")
            .expect("expected chained string member access")
            + "\"hello\".to_upper().".len(),
    )
    .expect("offset");
    let chained_completions = completions_at(&analysis, file_id, chained_offset);
    member_completion(&chained_completions, "contains");
    member_completion(&chained_completions, "len");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                [1, 2, 3][0].
                next();
            }

            fn next() {}
        "#,
    );
    let indexed_offset = u32::try_from(
        text.find("[1, 2, 3][0].")
            .expect("expected indexed int member access")
            + "[1, 2, 3][0].".len(),
    )
    .expect("offset");
    let indexed_completions = completions_at(&analysis, file_id, indexed_offset);
    member_completion(&indexed_completions, "is_odd");
    member_completion(&indexed_completions, "to_float");

    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                #{ name: "Ada" }.name.
                next();
            }

            fn next() {}
        "#,
    );
    let field_offset = u32::try_from(
        text.find("#{ name: \"Ada\" }.name.")
            .expect("expected object field string member access")
            + "#{ name: \"Ada\" }.name.".len(),
    )
    .expect("offset");
    let field_completions = completions_at(&analysis, file_id, field_offset);
    member_completion(&field_completions, "contains");
    member_completion(&field_completions, "len");
}
