use crate::syntax::{SyntaxError, SyntaxToken, TextRange, TextSize, TokenKind};

#[derive(Debug, Clone)]
pub struct Lexed {
    tokens: Vec<SyntaxToken>,
    errors: Vec<SyntaxError>,
}

impl Lexed {
    pub fn new(tokens: Vec<SyntaxToken>, errors: Vec<SyntaxError>) -> Self {
        Self { tokens, errors }
    }

    pub fn tokens(&self) -> &[SyntaxToken] {
        &self.tokens
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    pub fn into_parts(self) -> (Vec<SyntaxToken>, Vec<SyntaxError>) {
        (self.tokens, self.errors)
    }

    pub fn shifted(mut self, offset: TextSize) -> Self {
        if offset == TextSize::from(0) {
            return self;
        }

        self.tokens = self
            .tokens
            .into_iter()
            .map(|token| {
                SyntaxToken::new(
                    token.kind(),
                    TextRange::new(token.range().start() + offset, token.range().end() + offset),
                )
            })
            .collect();
        self.errors = self
            .errors
            .into_iter()
            .map(|error| {
                SyntaxError::new(
                    error.message().to_owned(),
                    TextRange::new(error.range().start() + offset, error.range().end() + offset),
                )
            })
            .collect();
        self
    }
}

pub fn lex_text(text: &str) -> Lexed {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut offset = 0usize;

    while offset < text.len() {
        let start = offset;
        let ch = next_char(text, offset);

        let kind = match ch {
            '#' if start == 0 && matches_char(text, offset + 1, '!') => {
                offset += 2;
                offset = consume_while(text, offset, |next| next != '\n');
                TokenKind::Shebang
            }
            c if c.is_whitespace() => {
                offset = consume_while(text, offset, |next| next.is_whitespace());
                TokenKind::Whitespace
            }
            '/' if matches_char(text, offset + 1, '/') => {
                let kind =
                    if matches_char(text, offset + 2, '/') || matches_char(text, offset + 2, '!') {
                        TokenKind::DocLineComment
                    } else {
                        TokenKind::LineComment
                    };
                offset += 2;
                offset = consume_while(text, offset, |next| next != '\n');
                kind
            }
            '/' if matches_char(text, offset + 1, '*') => {
                let kind =
                    if matches_char(text, offset + 2, '*') || matches_char(text, offset + 2, '!') {
                        TokenKind::DocBlockComment
                    } else {
                        TokenKind::BlockComment
                    };
                offset = lex_block_comment(text, offset, &mut errors);
                kind
            }
            '#' if matches_char(text, offset + 1, '{') => {
                offset += 2;
                TokenKind::HashBraceOpen
            }
            '#' if is_raw_string_start(text, offset) => {
                offset = lex_raw_string(text, offset, &mut errors);
                TokenKind::RawString
            }
            '"' => {
                offset = lex_quoted_string(text, offset, '"', true, &mut errors);
                TokenKind::String
            }
            '\'' => {
                offset = lex_quoted_string(text, offset, '\'', true, &mut errors);
                TokenKind::Char
            }
            '`' => {
                offset = lex_backtick_string(text, offset, &mut errors);
                TokenKind::BacktickString
            }
            c if is_ident_start(c) => {
                offset = consume_while(text, offset, is_ident_continue);
                match &text[start..offset] {
                    "_" => TokenKind::Underscore,
                    "let" => TokenKind::LetKw,
                    "const" => TokenKind::ConstKw,
                    "if" => TokenKind::IfKw,
                    "else" => TokenKind::ElseKw,
                    "switch" => TokenKind::SwitchKw,
                    "do" => TokenKind::DoKw,
                    "while" => TokenKind::WhileKw,
                    "until" => TokenKind::UntilKw,
                    "loop" => TokenKind::LoopKw,
                    "for" => TokenKind::ForKw,
                    "in" => TokenKind::InKw,
                    "continue" => TokenKind::ContinueKw,
                    "break" => TokenKind::BreakKw,
                    "return" => TokenKind::ReturnKw,
                    "throw" => TokenKind::ThrowKw,
                    "try" => TokenKind::TryKw,
                    "catch" => TokenKind::CatchKw,
                    "import" => TokenKind::ImportKw,
                    "export" => TokenKind::ExportKw,
                    "as" => TokenKind::AsKw,
                    "global" => TokenKind::GlobalKw,
                    "private" => TokenKind::PrivateKw,
                    "fn" => TokenKind::FnKw,
                    "this" => TokenKind::ThisKw,
                    "true" => TokenKind::TrueKw,
                    "false" => TokenKind::FalseKw,
                    "Fn" => TokenKind::FnPtrKw,
                    "call" => TokenKind::CallKw,
                    "curry" => TokenKind::CurryKw,
                    "is_shared" => TokenKind::IsSharedKw,
                    "is_def_fn" => TokenKind::IsDefFnKw,
                    "is_def_var" => TokenKind::IsDefVarKw,
                    "type_of" => TokenKind::TypeOfKw,
                    "print" => TokenKind::PrintKw,
                    "debug" => TokenKind::DebugKw,
                    "eval" => TokenKind::EvalKw,
                    _ => TokenKind::Ident,
                }
            }
            c if c.is_ascii_digit() => {
                offset = lex_number(text, offset);
                classify_number(&text[start..offset])
            }
            '(' => {
                offset += 1;
                TokenKind::OpenParen
            }
            ')' => {
                offset += 1;
                TokenKind::CloseParen
            }
            '[' => {
                offset += 1;
                TokenKind::OpenBracket
            }
            ']' => {
                offset += 1;
                TokenKind::CloseBracket
            }
            '{' => {
                offset += 1;
                TokenKind::OpenBrace
            }
            '}' => {
                offset += 1;
                TokenKind::CloseBrace
            }
            ',' => {
                offset += 1;
                TokenKind::Comma
            }
            ':' if matches_char(text, offset + 1, ':') => {
                offset += 2;
                TokenKind::ColonColon
            }
            ':' => {
                offset += 1;
                TokenKind::Colon
            }
            ';' => {
                offset += 1;
                TokenKind::Semicolon
            }
            '=' if matches_char(text, offset + 1, '>') => {
                offset += 2;
                TokenKind::FatArrow
            }
            '=' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::EqEq
            }
            '=' => {
                offset += 1;
                TokenKind::Eq
            }
            '+' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::PlusEq
            }
            '+' => {
                offset += 1;
                TokenKind::Plus
            }
            '-' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::MinusEq
            }
            '-' => {
                offset += 1;
                TokenKind::Minus
            }
            '*' if starts_with(text, offset, "**=") => {
                offset += 3;
                TokenKind::StarStarEq
            }
            '*' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::StarEq
            }
            '*' if matches_char(text, offset + 1, '*') => {
                offset += 2;
                TokenKind::StarStar
            }
            '*' => {
                offset += 1;
                TokenKind::Star
            }
            '/' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::SlashEq
            }
            '/' => {
                offset += 1;
                TokenKind::Slash
            }
            '%' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::PercentEq
            }
            '%' => {
                offset += 1;
                TokenKind::Percent
            }
            '<' if starts_with(text, offset, "<<=") => {
                offset += 3;
                TokenKind::ShlEq
            }
            '<' if matches_char(text, offset + 1, '<') => {
                offset += 2;
                TokenKind::Shl
            }
            '<' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::LtEq
            }
            '<' => {
                offset += 1;
                TokenKind::Lt
            }
            '>' if starts_with(text, offset, ">>=") => {
                offset += 3;
                TokenKind::ShrEq
            }
            '>' if matches_char(text, offset + 1, '>') => {
                offset += 2;
                TokenKind::Shr
            }
            '>' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::GtEq
            }
            '>' => {
                offset += 1;
                TokenKind::Gt
            }
            '&' if matches_char(text, offset + 1, '&') => {
                offset += 2;
                TokenKind::AmpAmp
            }
            '&' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::AmpEq
            }
            '&' => {
                offset += 1;
                TokenKind::Amp
            }
            '|' if matches_char(text, offset + 1, '|') => {
                offset += 2;
                TokenKind::PipePipe
            }
            '|' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::PipeEq
            }
            '|' => {
                offset += 1;
                TokenKind::Pipe
            }
            '^' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::CaretEq
            }
            '^' => {
                offset += 1;
                TokenKind::Caret
            }
            '!' if matches_char(text, offset + 1, '=') => {
                offset += 2;
                TokenKind::BangEq
            }
            '!' => {
                offset += 1;
                TokenKind::Bang
            }
            '?' if starts_with(text, offset, "??=") => {
                offset += 3;
                TokenKind::QuestionQuestionEq
            }
            '?' if starts_with(text, offset, "??") => {
                offset += 2;
                TokenKind::QuestionQuestion
            }
            '?' if starts_with(text, offset, "?.") => {
                offset += 2;
                TokenKind::QuestionDot
            }
            '?' if starts_with(text, offset, "?[") => {
                offset += 2;
                TokenKind::QuestionOpenBracket
            }
            '.' if starts_with(text, offset, "..=") => {
                offset += 3;
                TokenKind::RangeEq
            }
            '.' if starts_with(text, offset, "..") => {
                offset += 2;
                TokenKind::Range
            }
            '.' => {
                offset += 1;
                TokenKind::Dot
            }
            _ => {
                offset += ch.len_utf8();
                TokenKind::Unknown
            }
        };

        tokens.push(SyntaxToken::new(kind, text_range(start, offset)));
    }

    Lexed::new(tokens, errors)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

fn classify_number(text: &str) -> TokenKind {
    if text.contains(['.', 'e', 'E']) {
        TokenKind::Float
    } else {
        TokenKind::Int
    }
}

fn lex_number(text: &str, mut offset: usize) -> usize {
    offset = consume_while(text, offset, |next| next.is_ascii_digit());

    if offset < text.len()
        && matches_char(text, offset, '.')
        && matches_next_char(text, offset + 1, |next| next.is_ascii_digit())
    {
        offset += 1;
        offset = consume_while(text, offset, |next| next.is_ascii_digit());
    }

    if offset < text.len() && matches_next_char(text, offset, |next| next == 'e' || next == 'E') {
        let checkpoint = offset;
        offset += 1;

        if offset < text.len() && matches_next_char(text, offset, |next| next == '+' || next == '-')
        {
            offset += 1;
        }

        let exponent_end = consume_while(text, offset, |next| next.is_ascii_digit());
        if exponent_end > offset {
            offset = exponent_end;
        } else {
            offset = checkpoint;
        }
    }

    offset
}

fn lex_quoted_string(
    text: &str,
    mut offset: usize,
    terminator: char,
    allow_escapes: bool,
    errors: &mut Vec<SyntaxError>,
) -> usize {
    let start = offset;
    offset += terminator.len_utf8();
    let mut terminated = false;

    while offset < text.len() {
        let next = next_char(text, offset);
        offset += next.len_utf8();

        if allow_escapes && next == '\\' && offset < text.len() {
            let escaped = next_char(text, offset);
            offset += escaped.len_utf8();
            continue;
        }

        if next == terminator {
            terminated = true;
            break;
        }
    }

    if !terminated {
        let label = if terminator == '\'' {
            "unterminated character literal"
        } else {
            "unterminated string literal"
        };
        errors.push(SyntaxError::new(label, text_range(start, text.len())));
    }

    offset
}

fn lex_backtick_string(text: &str, mut offset: usize, errors: &mut Vec<SyntaxError>) -> usize {
    let start = offset;
    offset += 1;
    let mut terminated = false;

    while offset < text.len() {
        if starts_with(text, offset, "``") {
            offset += 2;
            continue;
        }

        if starts_with(text, offset, "${") {
            offset += 2;
            offset = lex_interpolation_block(text, offset, errors);
            continue;
        }

        let next = next_char(text, offset);
        offset += next.len_utf8();

        if next == '`' {
            terminated = true;
            break;
        }
    }

    if !terminated {
        errors.push(SyntaxError::new(
            "unterminated back-tick string literal",
            text_range(start, text.len()),
        ));
    }

    offset
}

fn lex_interpolation_block(text: &str, mut offset: usize, errors: &mut Vec<SyntaxError>) -> usize {
    let start = offset.saturating_sub(2);
    let mut depth = 1usize;

    while offset < text.len() && depth > 0 {
        if starts_with(text, offset, "//") {
            offset += 2;
            offset = consume_while(text, offset, |next| next != '\n');
            continue;
        }

        if starts_with(text, offset, "/*") {
            offset = lex_block_comment(text, offset, errors);
            continue;
        }

        if is_raw_string_start(text, offset) {
            offset = lex_raw_string(text, offset, errors);
            continue;
        }

        let next = next_char(text, offset);

        match next {
            '"' => {
                offset = lex_quoted_string(text, offset, '"', true, errors);
            }
            '\'' => {
                offset = lex_quoted_string(text, offset, '\'', true, errors);
            }
            '`' => {
                offset = lex_backtick_string(text, offset, errors);
            }
            '{' => {
                depth += 1;
                offset += 1;
            }
            '}' => {
                depth -= 1;
                offset += 1;
            }
            _ => {
                offset += next.len_utf8();
            }
        }
    }

    if depth > 0 {
        errors.push(SyntaxError::new(
            "unterminated string interpolation",
            text_range(start, text.len()),
        ));
    }

    offset
}

fn lex_block_comment(text: &str, mut offset: usize, errors: &mut Vec<SyntaxError>) -> usize {
    let start = offset;
    offset += 2;
    let mut depth = 1usize;

    while offset < text.len() && depth > 0 {
        if starts_with(text, offset, "/*") {
            offset += 2;
            depth += 1;
        } else if starts_with(text, offset, "*/") {
            offset += 2;
            depth -= 1;
        } else {
            offset += next_char(text, offset).len_utf8();
        }
    }

    if depth > 0 {
        errors.push(SyntaxError::new(
            "unterminated block comment",
            text_range(start, text.len()),
        ));
    }

    offset
}

fn is_raw_string_start(text: &str, offset: usize) -> bool {
    let mut cursor = offset;
    while cursor < text.len() && matches_char(text, cursor, '#') {
        cursor += 1;
    }
    matches_char(text, cursor, '"')
}

fn lex_raw_string(text: &str, mut offset: usize, errors: &mut Vec<SyntaxError>) -> usize {
    let start = offset;
    let mut hash_count = 0usize;
    while offset < text.len() && matches_char(text, offset, '#') {
        hash_count += 1;
        offset += 1;
    }

    offset += 1;
    let mut terminated = false;

    while offset < text.len() {
        let next = next_char(text, offset);
        offset += next.len_utf8();

        if next != '"' {
            continue;
        }

        let mut cursor = offset;
        let mut matched = true;
        for _ in 0..hash_count {
            if !matches_char(text, cursor, '#') {
                matched = false;
                break;
            }
            cursor += 1;
        }

        if matched {
            offset = cursor;
            terminated = true;
            break;
        }
    }

    if !terminated {
        errors.push(SyntaxError::new(
            "unterminated raw string literal",
            text_range(start, text.len()),
        ));
    }

    offset
}

fn consume_while(text: &str, mut offset: usize, predicate: impl Fn(char) -> bool) -> usize {
    while offset < text.len() {
        let ch = next_char(text, offset);
        if !predicate(ch) {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}

fn next_char(text: &str, offset: usize) -> char {
    text[offset..]
        .chars()
        .next()
        .expect("valid offset while lexing")
}

fn matches_char(text: &str, offset: usize, expected: char) -> bool {
    text.get(offset..)
        .is_some_and(|rest| rest.starts_with(expected))
}

fn matches_next_char(text: &str, offset: usize, predicate: impl Fn(char) -> bool) -> bool {
    text.get(offset..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(predicate)
}

fn starts_with(text: &str, offset: usize, expected: &str) -> bool {
    text.get(offset..)
        .is_some_and(|rest| rest.starts_with(expected))
}

fn text_range(start: usize, end: usize) -> TextRange {
    TextRange::new(text_offset(start), text_offset(end))
}

fn text_offset(offset: usize) -> TextSize {
    TextSize::from(u32::try_from(offset).unwrap_or(u32::MAX))
}
