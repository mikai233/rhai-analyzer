use crate::{SyntaxErrorCode, SyntaxKind, TokenKind, lex_text};

#[test]
fn lexes_basic_tokens() {
    let lexed = lex_text("let answer = add(1, 2)");
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::LetKw,
            TokenKind::Ident,
            TokenKind::Eq,
            TokenKind::Ident,
            TokenKind::OpenParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::CloseParen,
        ]
    );
    assert!(lexed.errors().is_empty());
}
#[test]
fn syntax_kind_round_trips_through_rowan_kind() {
    let kinds = [
        SyntaxKind::Root,
        SyntaxKind::StmtLet,
        SyntaxKind::ExprBinary,
        SyntaxKind::ArgList,
        SyntaxKind::CatchClause,
        SyntaxKind::Error,
    ];

    for kind in kinds {
        let raw = kind.to_rowan();
        assert_eq!(SyntaxKind::from_rowan(raw), Some(kind));
    }

    assert_eq!(SyntaxKind::from_rowan(rowan::SyntaxKind(u16::MAX)), None);
}
#[test]
fn lexes_extended_rhai_token_set() {
    let source = r#"
        const answer = #{ value: 42, text: `hello ${name}` };
        if answer.value >= 40 && "value" in answer {
            answer.value += 1;
            answer?.items?[0] ??= 0;
        }
        /* nested /* block */ comment */
    "#;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert!(kinds.contains(&TokenKind::ConstKw));
    assert!(kinds.contains(&TokenKind::HashBraceOpen));
    assert!(kinds.contains(&TokenKind::Colon));
    assert!(kinds.contains(&TokenKind::Backtick));
    assert!(kinds.contains(&TokenKind::StringText));
    assert!(kinds.contains(&TokenKind::InterpolationStart));
    assert!(kinds.contains(&TokenKind::IfKw));
    assert!(kinds.contains(&TokenKind::GtEq));
    assert!(kinds.contains(&TokenKind::AmpAmp));
    assert!(kinds.contains(&TokenKind::InKw));
    assert!(kinds.contains(&TokenKind::PlusEq));
    assert!(kinds.contains(&TokenKind::QuestionDot));
    assert!(kinds.contains(&TokenKind::QuestionOpenBracket));
    assert!(kinds.contains(&TokenKind::QuestionQuestionEq));
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}
#[test]
fn lexes_doc_comments_and_shebang() {
    let source = "#!/usr/bin/env rhai\n/// docs\n//! module docs\n/** block docs */\n/*! inner docs */\nlet value = 1;";
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed.tokens().iter().map(|token| token.kind()).collect();

    assert_eq!(kinds[0], TokenKind::Shebang);
    assert!(kinds.contains(&TokenKind::DocLineComment));
    assert!(kinds.contains(&TokenKind::DocBlockComment));
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}
#[test]
fn lexes_raw_strings_number_edges_and_remaining_operators() {
    let source = r###"
        let raw = #"alpha"#;
        let deeper = ##"beta"##;
        let plain = 42;
        let float = 3.14;
        let exp = 6.02e23;
        let fallback = 10e + 2;
        value <<= 1;
        value >>= 2;
        value &= 3;
        value |= 4;
        value ^= 5;
        value **= 6;
        if left == right != maybe { result = 1..=3; other = 4..9; }
        @
    "###;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert!(kinds.contains(&TokenKind::RawString));
    assert!(kinds.contains(&TokenKind::Float));
    assert!(kinds.contains(&TokenKind::ShlEq));
    assert!(kinds.contains(&TokenKind::ShrEq));
    assert!(kinds.contains(&TokenKind::AmpEq));
    assert!(kinds.contains(&TokenKind::PipeEq));
    assert!(kinds.contains(&TokenKind::CaretEq));
    assert!(kinds.contains(&TokenKind::StarStarEq));
    assert!(kinds.contains(&TokenKind::EqEq));
    assert!(kinds.contains(&TokenKind::BangEq));
    assert!(kinds.contains(&TokenKind::RangeEq));
    assert!(kinds.contains(&TokenKind::Range));
    assert!(kinds.contains(&TokenKind::Unknown));

    assert!(
        kinds.windows(4).any(|window| {
            window
                == [
                    TokenKind::Int,
                    TokenKind::Ident,
                    TokenKind::Plus,
                    TokenKind::Int,
                ]
        }),
        "{kinds:?}"
    );
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}
#[test]
fn lexes_interpolation_with_nested_lexical_forms() {
    let source =
        r###"let value = `head ${ foo(1, #"raw"#, `inner ${bar}`) /* note */ + "tail" } tail`;"###;
    let lexed = lex_text(source);
    let kinds: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::LetKw,
            TokenKind::Ident,
            TokenKind::Eq,
            TokenKind::Backtick,
            TokenKind::StringText,
            TokenKind::InterpolationStart,
            TokenKind::Ident,
            TokenKind::OpenParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::RawString,
            TokenKind::Comma,
            TokenKind::Backtick,
            TokenKind::StringText,
            TokenKind::InterpolationStart,
            TokenKind::Ident,
            TokenKind::CloseBrace,
            TokenKind::Backtick,
            TokenKind::CloseParen,
            TokenKind::Plus,
            TokenKind::String,
            TokenKind::CloseBrace,
            TokenKind::StringText,
            TokenKind::Backtick,
            TokenKind::Semicolon,
        ]
    );
    assert!(lexed.errors().is_empty(), "{:?}", lexed.errors());
}
#[test]
fn reports_unterminated_string_like_literals() {
    let string_lexed = lex_text("\"unterminated");
    assert_eq!(string_lexed.errors().len(), 1);
    assert_eq!(
        string_lexed.errors()[0].code(),
        &SyntaxErrorCode::UnterminatedStringLiteral
    );

    let char_lexed = lex_text("'x");
    assert_eq!(char_lexed.errors().len(), 1);
    assert_eq!(
        char_lexed.errors()[0].code(),
        &SyntaxErrorCode::UnterminatedCharacterLiteral
    );

    let raw_lexed = lex_text("#\"raw");
    assert_eq!(raw_lexed.errors().len(), 1);
    assert_eq!(
        raw_lexed.errors()[0].code(),
        &SyntaxErrorCode::UnterminatedRawStringLiteral
    );

    let backtick_lexed = lex_text("`value");
    assert_eq!(backtick_lexed.errors().len(), 1);
    assert_eq!(
        backtick_lexed.errors()[0].code(),
        &SyntaxErrorCode::UnterminatedBacktickStringLiteral
    );
}
#[test]
fn reports_unterminated_interpolation_and_block_comments() {
    let interpolation_lexed = lex_text("`value = ${foo(1)`");
    let interpolation_codes: Vec<_> = interpolation_lexed
        .errors()
        .iter()
        .map(|error| error.code())
        .collect();
    assert!(
        interpolation_codes.contains(&&SyntaxErrorCode::UnterminatedStringInterpolation),
        "{interpolation_codes:?}"
    );
    assert!(
        interpolation_codes.contains(&&SyntaxErrorCode::UnterminatedBacktickStringLiteral),
        "{interpolation_codes:?}"
    );
    assert!(interpolation_codes.len() >= 2, "{interpolation_codes:?}");

    let block_lexed = lex_text("/* outer /* inner */");
    assert_eq!(block_lexed.errors().len(), 1);
    assert_eq!(
        block_lexed.errors()[0].code(),
        &SyntaxErrorCode::UnterminatedBlockComment
    );
}
