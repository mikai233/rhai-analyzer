use crate::{SyntaxErrorCode, parse_text};

#[test]
fn recovers_when_for_is_missing_in_keyword() {
    let parse = parse_text("for value values { break; }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedInForExpression
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprFor"), "{tree}");
    assert!(tree.contains("ForBindings"), "{tree}");
}
#[test]
fn recovers_when_switch_arm_is_missing_arrow() {
    let parse = parse_text("switch value { 1 `one`, _ => `other` }");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedSwitchArmArrow
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("ExprSwitch"), "{tree}");
    assert!(tree.contains("SwitchArmList"), "{tree}");
    assert!(tree.contains("SwitchArm"), "{tree}");
}
#[test]
fn recovers_when_const_is_missing_value() {
    let parse = parse_text("const ANSWER = ;");

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedConstantValue
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtConst"), "{tree}");
    assert!(tree.contains("Error"), "{tree}");
}
#[test]
fn recovers_when_alias_is_missing_after_as() {
    let parse = parse_text(r#"import "crypto" as ;"#);

    assert_eq!(parse.errors().len(), 1, "{}", parse.debug_tree());
    assert_eq!(
        parse.errors()[0].code(),
        &SyntaxErrorCode::ExpectedAliasAfterAs
    );

    let tree = parse.debug_tree();
    assert!(tree.contains("StmtImport"), "{tree}");
    assert!(tree.contains("AliasClause"), "{tree}");
    assert!(tree.contains("Error"), "{tree}");
}
