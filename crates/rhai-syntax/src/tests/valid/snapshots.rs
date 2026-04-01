use crate::parse_text;

#[test]
fn compact_snapshot_for_valid_program() {
    let parse = parse_text(
        r#"private fn add(x, y,) { return x + y; }
const ANSWER = add(20, 22);
let kind = switch ANSWER { 42 => `yes`, _ => `no` };"#,
    );

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expected = r#"Root
  RootItemList
    ItemFn
      PrivateKw "private"
      FnKw "fn"
      Ident "add"
      ParamList
        OpenParen "("
        Ident "x"
        Comma ","
        Ident "y"
        Comma ","
        CloseParen ")"
      Block
        OpenBrace "{"
        BlockItemList
          StmtReturn
            ReturnKw "return"
            ExprBinary
              ExprName
                Ident "x"
              Plus "+"
              ExprName
                Ident "y"
            Semicolon ";"
        CloseBrace "}"
    StmtConst
      ConstKw "const"
      Ident "ANSWER"
      Eq "="
      ExprCall
        ExprName
          Ident "add"
        ArgList
          OpenParen "("
          ExprLiteral
            Int "20"
          Comma ","
          ExprLiteral
            Int "22"
          CloseParen ")"
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "kind"
      Eq "="
      ExprSwitch
        SwitchKw "switch"
        ExprName
          Ident "ANSWER"
        OpenBrace "{"
        SwitchArmList
          SwitchArm
            SwitchPatternList
              ExprLiteral
                Int "42"
            FatArrow "=>"
            ExprLiteral
              BacktickString "`yes`"
          Comma ","
          SwitchArm
            SwitchPatternList
              Underscore "_"
            FatArrow "=>"
            ExprLiteral
              BacktickString "`no`"
        CloseBrace "}"
      Semicolon ";"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}
#[test]
fn compact_snapshot_for_interpolated_string() {
    let parse = parse_text(r#"let msg = `value=${x + 1}`;"#);

    assert!(parse.errors().is_empty(), "{}", parse.debug_tree());

    let expected = r#"Root
  RootItemList
    StmtLet
      LetKw "let"
      Ident "msg"
      Eq "="
      ExprInterpolatedString
        Backtick "`"
        StringPartList
          StringSegment
            StringText "value="
          StringInterpolation
            InterpolationStart "${"
            InterpolationBody
              InterpolationItemList
                StmtExpr
                  ExprBinary
                    ExprName
                      Ident "x"
                    Plus "+"
                    ExprLiteral
                      Int "1"
            CloseBrace "}"
        Backtick "`"
      Semicolon ";"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}
