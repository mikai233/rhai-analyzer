use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_comments_inside_delimited_containers() {
    let source = r#"
fn run(
left,
// keep right
right,
){
let values=[
1, // first
// keep second
2,
];
let user=#{
name:"Ada",
// keep age
age:42,
};
process(
left, // keep left
// keep arg
right,
);
}
"#;

    let expected = r#"fn run(
    left,
    // keep right
    right,
) {
    let values = [
        1, // first
        // keep second
        2,
    ];
    let user = #{
        name: "Ada",
        // keep age
        age: 42,
    };
    process(
        left, // keep left
        // keep arg
        right,
    );
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_dangling_comments_after_last_delimited_item() {
    let source = r#"
fn run(){
process(
value,
// keep trailing arg comment
);
}
"#;

    let expected = r#"fn run() {
    process(
        value,
        // keep trailing arg comment
    );
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_dangling_comments_in_empty_delimited_containers() {
    let source = r#"
fn run(){
let values=[
// nothing yet
];
let user=#{
// nothing here
};
process(
// no args
);
}
"#;

    let expected = r#"fn run() {
    let values = [
        // nothing yet
    ];
    let user = #{
        // nothing here
    };
    process(
        // no args
    );
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_comments_inside_switch_arms() {
    let source = r#"
fn run(value){
let label=switch value{
// zero
0=>1, // trailing zero

// one
1=>{
// nested
value
},
// fallback
_=>2
};
label
}
"#;

    let expected = r#"fn run(value) {
    let label = switch value {
        // zero
        0 => 1, // trailing zero

        // one
        1 => {
            // nested
            value
        },
        // fallback
        _ => 2
    };
    label
}
"#;

    assert_formats_to(source, expected);
}
