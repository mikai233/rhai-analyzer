use crate::tests::inference_flow::narrowing::{assert_variable_type, load_snapshot};
use rhai_hir::TypeRef;

#[test]
fn snapshot_narrows_union_values_after_type_of_function_guards() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type int | string
            let value = source;

            let picked = if type_of(value) == "string" {
                value
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_union_values_after_type_of_method_guards() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type int | string
            let value = source;

            let picked = if value.type_of() != "string" {
                value
            } else {
                0
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::Int);
}

#[test]
fn snapshot_narrows_union_values_in_switch_type_of_string_arms() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type int | string
            let value = source;

            let picked = switch type_of(value) {
                "string" => value,
                _ => "fallback",
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_union_values_in_switch_type_of_wildcard_arms() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type int | string
            let value = source;

            let picked = switch type_of(value) {
                "string" => "fallback",
                _ => {
                    let narrowed = value;
                    narrowed
                },
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "narrowed", TypeRef::Int);
    assert_variable_type(
        &snapshot,
        file_id,
        "picked",
        TypeRef::Union(vec![TypeRef::String, TypeRef::Int]),
    );
}
