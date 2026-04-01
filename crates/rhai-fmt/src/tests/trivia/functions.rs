use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_comments_between_function_signature_and_body() {
    let source = r#"
fn run(value)
// explain body
{
value
}
"#;

    let expected = r#"fn run(value)
// explain body
{
    value
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_comments_inside_function_signatures() {
    let source = r#"
private /* keep private */ fn /* keep fn */ "Custom-Type" /* keep dot */ . /* keep name */ refresh /* keep params */ (value){
value
}
"#;

    let expected = r#"private /* keep private */ fn /* keep fn */ "Custom-Type" /* keep dot */ . /* keep name */ refresh /* keep params */ (value) {
    value
}
"#;

    assert_formats_to(source, expected);
}
