use crate::tests::signature_help::builtins::{load_analysis, signature_help_at};

#[test]
fn signature_help_returns_builtin_string_method_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let value = "hello";
                value.contains("ell");
            }
        "#,
    );

    let offset =
        u32::try_from(text.find("\"ell\"").expect("expected contains arg") + 2).expect("offset");
    let help = signature_help_at(&analysis, file_id, offset);
    assert!(!help.signatures.is_empty());
    assert!(
        help.signatures
            .iter()
            .any(|signature| signature.label == "fn contains(string) -> bool")
    );
}

#[test]
fn signature_help_returns_builtin_array_and_map_method_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.contains(item);

                let user = #{ name: "Ada" };
                user.contains(key);
            }
        "#,
    );

    let array_offset =
        u32::try_from(text.find("item").expect("expected array argument")).expect("offset");
    let array_help = signature_help_at(&analysis, file_id, array_offset);
    assert_eq!(array_help.signatures[0].label, "fn contains(any) -> bool");

    let map_offset =
        u32::try_from(text.rfind("key").expect("expected map argument")).expect("offset");
    let map_help = signature_help_at(&analysis, file_id, map_offset);
    assert_eq!(map_help.signatures[0].label, "fn contains(string) -> bool");
}

#[test]
fn signature_help_returns_builtin_primitive_method_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let count = 1;
                count.max(limit);

                let ratio = 3.5;
                ratio.max(limit);
            }
        "#,
    );

    let int_offset =
        u32::try_from(text.find("limit").expect("expected int max arg")).expect("offset");
    let int_help = signature_help_at(&analysis, file_id, int_offset);
    assert_eq!(int_help.signatures[0].label, "fn max(int) -> int");

    let float_offset =
        u32::try_from(text.rfind("limit").expect("expected float max arg")).expect("offset");
    let float_help = signature_help_at(&analysis, file_id, float_offset);
    assert_eq!(
        float_help.signatures[0].label,
        "fn max(int | float) -> float"
    );
}

#[test]
fn signature_help_prefers_map_tag_field_function_over_builtin_dynamic_tag_method() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let user = #{ tag: || "field-fn", name: "Ada" };
                user.tag();
            }
        "#,
    );

    let offset = u32::try_from(text.find(".tag(").expect("expected tag call") + ".tag(".len())
        .expect("offset");
    let help = signature_help_at(&analysis, file_id, offset);
    assert_eq!(help.signatures[0].label, "fn tag() -> string");
}
