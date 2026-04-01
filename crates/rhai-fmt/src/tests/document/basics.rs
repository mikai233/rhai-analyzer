use crate::{FormatOptions, format_text};

use crate::tests::assert_formats_to;

#[test]
fn formatter_rewrites_basic_functions_blocks_and_binary_spacing() {
    let source = r#"
fn helper(value){
let result=value+1;
result
}
"#;

    let result = format_text(source, &FormatOptions::default());
    let expected = r#"fn helper(value) {
    let result = value + 1;
    result
}
"#;

    assert_eq!(result.text, expected);
    assert!(result.changed);
}
#[test]
fn formatter_rewrites_arrays_objects_calls_and_imports() {
    let source = r#"import   "tools"  as tools;
fn run(){let values=[1,2,3]; let user=#{name:"Ada",scores:[1,2]}; tools::add(values[0],user.name.len());}
"#;

    let expected = r#"import "tools" as tools;

fn run() {
    let values = [1, 2, 3];
    let user = #{name: "Ada", scores: [1, 2]};
    tools::add(values[0], user.name.len());
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_preserves_tail_expressions_and_multiline_containers() {
    let source = r#"
fn build(){
if true{#{name:"Ada",numbers:[1,2,3,4,5,6,7,8,9,10,11,12]}} else {#{name:"Bob"}}
}
"#;

    let expected = r#"fn build() {
    if true {
        #{
            name: "Ada",
            numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        }
    } else {
        #{name: "Bob"}
    }
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_rewrites_switch_loops_and_closures() {
    let source = r#"
fn flow(items){
let mapper=|value| value+1;
let label=switch items.len(){0=>`empty`,1|2=>`small`,_=>{`many`}};
for (item,index) in items { while index<10 { if item>0 { break item; } else { continue; } } }
loop { break; }
do { mapper(items[0]) } while items.len()<3;
}
"#;

    let expected = r#"fn flow(items) {
    let mapper = |value| value + 1;
    let label = switch items.len() {
        0 => `empty`,
        1 | 2 => `small`,
        _ => {
            `many`
        }
    };
    for (item, index) in items {
        while index < 10 {
            if item > 0 {
                break item;
            } else {
                continue;
            }
        }
    }
    loop {
        break;
    }
    do {
        mapper(items[0])
    } while items.len() < 3;
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_rewrites_safe_access_chains_and_multiline_interpolations() {
    let source = r#"
fn render(user,items){
let safe=user?.profile?[index+1].name;
let summary=`user=${user?.profile.name} total=${items.len()+1}`;
let detailed=`result=${let total=items.len()+1;
if total>10 { `many:${total}` } else { `few:${total}` }}`;
}
"#;

    let expected = r#"fn render(user, items) {
    let safe = user?.profile?[index + 1].name;
    let summary = `user=${user?.profile.name} total=${items.len() + 1}`;
    let detailed = `result=${
        let total = items.len() + 1;
        if total > 10 {
            `many:${total}`
        } else {
            `few:${total}`
        }
    }`;
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_formats_try_catch_bindings_with_parentheses() {
    let source = r#"
fn run(){
try { work(); } catch (err){ throw err; }
}
"#;

    let expected = r#"fn run() {
    try {
        work();
    } catch (err) {
        throw err;
    }
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_formats_private_typed_functions_and_caller_scope_calls() {
    let source = r#"
private fn int.do_update(left,right){helper!(left,right)}
fn "Custom-Type".refresh(){call!(worker,2);}
"#;

    let expected = r#"private fn int.do_update(left, right) {
    helper!(left, right)
}

fn "Custom-Type".refresh() {
    call!(worker, 2);
}
"#;

    assert_formats_to(source, expected);
}
#[test]
fn formatter_formats_do_until_loops() {
    let source = r#"
fn run(){
do { step() } until ready()
}
"#;

    let expected = r#"fn run() {
    do {
        step()
    } until ready()
}
"#;

    assert_formats_to(source, expected);
}
