use crate::parse_text;

#[test]
fn compact_snapshot_for_broken_program() {
    let parse = parse_text(
        r#"fn broken(x y {
let values = [1 2];
import "mod" as ;
}"#,
    );

    let expected = r#"Root
  RootItemList
    ItemFn
      FnKw "fn"
      Ident "broken"
      ParamList
        OpenParen "("
        Ident "x"
        Error
        Ident "y"
        Error
      Block
        OpenBrace "{"
        BlockItemList
          StmtLet
            LetKw "let"
            Ident "values"
            Eq "="
            ExprArray
              ArrayItemList
                OpenBracket "["
                ExprLiteral
                  Int "1"
                Error
                ExprLiteral
                  Int "2"
                CloseBracket "]"
            Semicolon ";"
          StmtImport
            ImportKw "import"
            ExprLiteral
              String "\"mod\""
            AliasClause
              AsKw "as"
              Error
            Semicolon ";"
        CloseBrace "}"
"#;

    assert_eq!(parse.debug_tree_compact(), expected);
}
#[test]
fn compact_snapshot_for_recovery_matrix() {
    let parse = parse_text(
        r##"fn broken(x, { return x; }
let sum = 1 + ;
let map = #{ a: 1 b: 2 };
let invoke = run(1 2);"##,
    );

    let expected = r##"Root
  RootItemList
    ItemFn
      FnKw "fn"
      Ident "broken"
      ParamList
        OpenParen "("
        Ident "x"
        Comma ","
        Error
        Error
      Block
        OpenBrace "{"
        BlockItemList
          StmtReturn
            ReturnKw "return"
            ExprName
              Ident "x"
            Semicolon ";"
        CloseBrace "}"
    StmtLet
      LetKw "let"
      Ident "sum"
      Eq "="
      ExprBinary
        ExprLiteral
          Int "1"
        Plus "+"
        Error
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "map"
      Eq "="
      ExprObject
        HashBraceOpen "#{"
        ObjectFieldList
          ObjectField
            Ident "a"
            Colon ":"
            ExprLiteral
              Int "1"
          Error
          ObjectField
            Ident "b"
            Colon ":"
            ExprLiteral
              Int "2"
        CloseBrace "}"
      Semicolon ";"
    StmtLet
      LetKw "let"
      Ident "invoke"
      Eq "="
      ExprCall
        ExprName
          Ident "run"
        ArgList
          OpenParen "("
          ExprLiteral
            Int "1"
          Error
          ExprLiteral
            Int "2"
          CloseParen ")"
      Semicolon ";"
"##;

    assert_eq!(parse.debug_tree_compact(), expected);
}
