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
