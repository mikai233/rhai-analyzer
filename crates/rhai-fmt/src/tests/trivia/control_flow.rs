use crate::tests::assert_formats_to;

#[test]
fn formatter_preserves_comments_between_while_or_loop_heads_and_bodies() {
    let source = r#"
fn run(value){
while value > 0
// loop while positive
{
value
}

loop
// keep polling
{
break;
}
}
"#;

    let expected = r#"fn run(value) {
    while value > 0
    // loop while positive
    {
        value
    }

    loop
    // keep polling
    {
        break;
    }
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_comments_between_if_for_do_heads_and_bodies() {
    let source = r#"
fn run(items, ready){
if ready
// enter branch
{
items.len()
} else
// nested branch
{
0
}

for item in items
// visit item
{
item
}

do
// once first
{
step()
}
// stop when ready
while ready
}
"#;

    let expected = r#"fn run(items, ready) {
    if ready
    // enter branch
    {
        items.len()
    } else
    // nested branch
    {
        0
    }

    for item in items
    // visit item
    {
        item
    }

    do
    // once first
    {
        step()
    }
    // stop when ready
    while ready
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_comments_between_try_catch_heads_and_bodies() {
    let source = r#"
fn run(){
try
// attempt work
{
work()
}
// recover below
catch (err)
// inspect error
{
log(err);
}
}
"#;

    let expected = r#"fn run() {
    try
    // attempt work
    {
        work()
    }
    // recover below
    catch (err)
    // inspect error
    {
        log(err);
    }
}
"#;

    assert_formats_to(source, expected);
}
