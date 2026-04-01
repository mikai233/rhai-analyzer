use crate::tests::inference_flow::narrowing::{assert_variable_type, load_snapshot};
use rhai_hir::TypeRef;

#[test]
fn snapshot_narrows_nullable_values_in_truthy_if_branches() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;

            let picked = if value {
                value
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_nullable_values_in_negated_else_branches() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;

            let picked = if !value {
                "fallback"
            } else {
                value
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_nullable_values_after_not_equal_unit_checks() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if value != none {
                value
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_nullable_values_after_equal_unit_else_branches() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if value == none {
                "fallback"
            } else {
                value
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_nullable_values_to_unit_in_equal_unit_branches() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };

            let picked = if !(value != none) {
                value
            } else {
                none
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::Unit);
}

#[test]
fn snapshot_narrows_nullable_values_through_conjunctive_null_checks() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            /// @type string?
            let value = source;
            let none = loop { break; };
            let ready = true;

            let picked = if value != none && ready {
                value
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_index_reads_after_not_equal_unit_checks() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            let none = loop { break; };
            let items = if flag {
                ["Ada"]
            } else {
                [none]
            };

            let picked = if items[0] != none {
                items[0]
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}
