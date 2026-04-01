use crate::tests::inference_flow::narrowing::{assert_variable_type, load_snapshot};
use rhai_hir::TypeRef;

#[test]
fn snapshot_narrows_member_reads_after_type_of_method_guards() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            let user = if flag {
                #{ name: "Ada" }
            } else {
                #{ name: 42 }
            };

            let picked = if user.name.type_of() == "string" {
                user.name
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_index_reads_after_type_of_function_guards() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            let items = if flag {
                ["Ada"]
            } else {
                [42]
            };

            let picked = if type_of(items[0]) == "string" {
                items[0]
            } else {
                "fallback"
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}

#[test]
fn snapshot_narrows_member_reads_in_switch_type_of_arms() {
    let (snapshot, file_id) = load_snapshot(
        r#"
            let user = if flag {
                #{ name: "Ada" }
            } else {
                #{ name: 42 }
            };

            let picked = switch type_of(user.name) {
                "string" => user.name,
                _ => "fallback",
            };
        "#,
    );

    assert_variable_type(&snapshot, file_id, "picked", TypeRef::String);
}
