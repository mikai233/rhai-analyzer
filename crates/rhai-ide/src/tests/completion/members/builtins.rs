use crate::CompletionItemSource;
use crate::tests::completion::members::{completions_at, load_analysis, member_completion};
use crate::{AnalysisHost, CompletionInsertFormat, FilePosition};
use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

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
fn completions_prefer_map_tag_field_over_builtin_dynamic_tag_property() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let user = #{ tag: "field-value", name: "Ada" };
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
    let tag = member_completion(&completions, "tag");
    let detail = tag.detail.as_deref().expect("expected tag detail");
    assert!(
        detail.contains("string"),
        "expected map tag completion to keep the field type, got {detail}"
    );
    assert!(
        !detail.contains("string | int"),
        "expected map tag completion not to advertise builtin tag property, got {detail}"
    );
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
    let tag_completion = member_completion(&int_completions, "tag");
    assert_eq!(tag_completion.detail.as_deref(), Some("int"));
    let docs = tag_completion.docs.as_deref().expect("expected tag docs");
    assert!(docs.contains("## Usage"));
    assert!(docs.contains("set_tag"));

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

#[test]
fn completions_keep_builtin_members_visible_for_prefixed_object_field_results() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            let student = #{ name: "mikai233" };
            let chars = student.name.c
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(std::path::Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("student.name.c")
            .expect("expected member completion target")
            + "student.name.c".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let chars = completions
        .iter()
        .filter(|item| item.label == "chars" && item.source == CompletionItemSource::Member)
        .collect::<Vec<_>>();
    member_completion(&completions, "contains");
    assert!(
        chars.len() >= 2,
        "expected overloaded chars completion items, got {completions:?}"
    );
    assert!(
        chars
            .iter()
            .all(|item| item.insert_format == CompletionInsertFormat::Snippet)
    );
    assert!(chars.iter().any(|item| {
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()) == Some("chars()$0")
    }));
}

#[test]
fn completions_prioritize_matching_member_overloads_by_argument_types() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                "hello".pa(5, "!")
            }
        "#,
    );

    let offset = u32::try_from(
        text.find("\"hello\".pa(5, \"!\")")
            .expect("expected member completion target")
            + "\"hello\".pa".len(),
    )
    .expect("offset");

    let completions = completions_at(&analysis, file_id, offset);
    let pad_overloads = completions
        .iter()
        .filter(|item| item.label == "pad" && item.source == CompletionItemSource::Member)
        .collect::<Vec<_>>();

    assert!(
        pad_overloads.len() >= 2,
        "expected multiple pad overload completions, got {completions:?}"
    );

    let string_index = pad_overloads
        .iter()
        .position(|item| item.detail.as_deref() == Some("fun(int, string) -> ()"))
        .expect("expected string pad overload");
    let char_index = pad_overloads
        .iter()
        .position(|item| item.detail.as_deref() == Some("fun(int, char) -> ()"))
        .expect("expected char pad overload");

    assert!(
        string_index < char_index,
        "expected string-matching pad overload to rank ahead of char overload: {pad_overloads:?}"
    );
}
