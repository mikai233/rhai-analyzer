use crate::parser::Parser;
use crate::syntax::{
    SyntaxError, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, TokenKind, empty_range,
    node_element, token_element,
};

impl<'a> Parser<'a> {
    pub(crate) fn parse_required_block(&mut self, message: &'static str) -> SyntaxNode {
        if self.at(TokenKind::OpenBrace) {
            self.parse_block_expr()
        } else {
            self.record_error(message, empty_range(self.current_offset()));
            SyntaxNode::with_range(
                SyntaxKind::Error,
                empty_range(self.current_offset()),
                Vec::new(),
            )
        }
    }

    pub(crate) fn bump_element(&mut self, expect_message: &'static str) -> crate::SyntaxElement {
        token_element(self.bump().expect(expect_message))
    }

    pub(crate) fn eat(
        &mut self,
        kind: TokenKind,
        expect_message: &'static str,
    ) -> Option<crate::SyntaxElement> {
        self.at(kind).then(|| self.bump_element(expect_message))
    }

    pub(crate) fn expect_token(
        &mut self,
        kind: TokenKind,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        self.eat(kind, expect_message)
            .unwrap_or_else(|| self.missing_error(error_message))
    }

    pub(crate) fn expect_ident(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        if self.at(TokenKind::Ident) {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn at_binding_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Ident | TokenKind::Underscore)
        )
    }

    pub(crate) fn expect_binding(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        if self.at_binding_start() {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn expect_alias_name(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        if matches!(
            self.peek_kind(),
            Some(TokenKind::Ident | TokenKind::GlobalKw)
        ) {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn expect_name_like(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        if self.at_name_like() {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn expect_object_field_name(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        if self.at_object_field_start() {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn missing_error(&mut self, message: &'static str) -> crate::SyntaxElement {
        self.record_error(message, empty_range(self.current_offset()));
        node_element(SyntaxNode::with_range(
            SyntaxKind::Error,
            empty_range(self.current_offset()),
            Vec::new(),
        ))
    }

    pub(crate) fn empty_error_node(&mut self, message: &'static str) -> SyntaxNode {
        let range = empty_range(self.current_offset());
        self.record_error(message, range);
        SyntaxNode::with_range(SyntaxKind::Error, range, Vec::new())
    }

    pub(crate) fn unexpected_token_error(&mut self, message: &'static str) -> SyntaxNode {
        let token = self.bump().expect("unexpected token should be present");
        self.record_error(message, token.range());
        SyntaxNode::new(
            SyntaxKind::Error,
            vec![token_element(token)],
            token.range().start(),
        )
    }

    pub(crate) fn record_error(&mut self, message: &'static str, range: TextRange) {
        self.errors.push(SyntaxError::new(message, range));
    }

    pub(crate) fn peek(&self) -> Option<SyntaxToken> {
        self.tokens.get(self.cursor).copied()
    }

    pub(crate) fn bump(&mut self) -> Option<SyntaxToken> {
        let token = self.peek()?;
        self.cursor += 1;
        Some(token)
    }

    pub(crate) fn peek_n(&self, n: usize) -> Option<SyntaxToken> {
        self.tokens.get(self.cursor + n).copied()
    }

    pub(crate) fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|token| token.kind())
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind() == kind)
    }

    pub(crate) fn at_fn_item_start(&self) -> bool {
        matches!(
            (self.peek_kind(), self.peek_n(1).map(|token| token.kind())),
            (Some(TokenKind::FnKw), _) | (Some(TokenKind::PrivateKw), Some(TokenKind::FnKw))
        )
    }

    pub(crate) fn current_offset(&self) -> TextSize {
        self.peek()
            .map_or(self.text_len, |token| token.range().start())
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    pub(crate) fn is_stmt_terminator(&self) -> bool {
        self.is_eof() || self.at(TokenKind::Semicolon) || self.at(TokenKind::CloseBrace)
    }

    pub(crate) fn at_expr_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(
                TokenKind::Ident
                    | TokenKind::ThisKw
                    | TokenKind::GlobalKw
                    | TokenKind::FnPtrKw
                    | TokenKind::CallKw
                    | TokenKind::CurryKw
                    | TokenKind::IsSharedKw
                    | TokenKind::IsDefFnKw
                    | TokenKind::IsDefVarKw
                    | TokenKind::TypeOfKw
                    | TokenKind::PrintKw
                    | TokenKind::DebugKw
                    | TokenKind::EvalKw
                    | TokenKind::Int
                    | TokenKind::Float
                    | TokenKind::String
                    | TokenKind::RawString
                    | TokenKind::BacktickString
                    | TokenKind::Char
                    | TokenKind::TrueKw
                    | TokenKind::FalseKw
                    | TokenKind::IfKw
                    | TokenKind::SwitchKw
                    | TokenKind::WhileKw
                    | TokenKind::LoopKw
                    | TokenKind::ForKw
                    | TokenKind::DoKw
                    | TokenKind::OpenBracket
                    | TokenKind::HashBraceOpen
                    | TokenKind::OpenParen
                    | TokenKind::OpenBrace
                    | TokenKind::Pipe
                    | TokenKind::PipePipe
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Bang
            )
        )
    }

    pub(crate) fn at_param_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Ident | TokenKind::Underscore | TokenKind::ThisKw)
        )
    }

    pub(crate) fn at_object_field_start(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Ident | TokenKind::String))
    }

    pub(crate) fn at_name_like(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(
                TokenKind::Ident
                    | TokenKind::ThisKw
                    | TokenKind::GlobalKw
                    | TokenKind::FnPtrKw
                    | TokenKind::CallKw
                    | TokenKind::CurryKw
                    | TokenKind::IsSharedKw
                    | TokenKind::IsDefFnKw
                    | TokenKind::IsDefVarKw
                    | TokenKind::TypeOfKw
                    | TokenKind::PrintKw
                    | TokenKind::DebugKw
                    | TokenKind::EvalKw
            )
        )
    }

    pub(crate) fn recover_to_any(&mut self, kinds: &[TokenKind]) {
        while !self.is_eof() && !kinds.iter().copied().any(|kind| self.at(kind)) {
            self.bump();
        }
    }
}

pub(crate) enum InterpolatedPart {
    Text(TextRange),
    Interpolation {
        start: TextRange,
        body_range: TextRange,
        end: TextRange,
    },
}

pub(crate) fn shift_error(error: SyntaxError, offset: TextSize) -> SyntaxError {
    SyntaxError::new(
        error.message().to_owned(),
        shift_range(error.range(), offset),
    )
}

pub(crate) fn shift_element(
    element: crate::SyntaxElement,
    offset: TextSize,
) -> crate::SyntaxElement {
    match element {
        crate::SyntaxElement::Node(node) => node_element(shift_node(*node, offset)),
        crate::SyntaxElement::Token(token) => token_element(SyntaxToken::new(
            token.kind(),
            shift_range(token.range(), offset),
        )),
    }
}

pub(crate) fn shift_node(node: SyntaxNode, offset: TextSize) -> SyntaxNode {
    let children = node
        .children()
        .iter()
        .cloned()
        .map(|element| shift_element(element, offset))
        .collect();

    SyntaxNode::with_range(node.kind(), shift_range(node.range(), offset), children)
}

pub(crate) fn shift_range(range: TextRange, offset: TextSize) -> TextRange {
    TextRange::new(range.start() + offset, range.end() + offset)
}

pub(crate) fn make_absolute_range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::from(u32::try_from(start).unwrap_or(u32::MAX)),
        TextSize::from(u32::try_from(end).unwrap_or(u32::MAX)),
    )
}

pub(crate) fn find_interpolation_end(text: &str, cursor: &mut usize) -> Option<usize> {
    let mut depth = 1usize;

    while *cursor < text.len() {
        if text[*cursor..].starts_with("//") {
            *cursor += 2;
            while *cursor < text.len() && next_char_at(text, *cursor) != '\n' {
                *cursor += next_char_at(text, *cursor).len_utf8();
            }
            continue;
        }

        if text[*cursor..].starts_with("/*") {
            skip_block_comment(text, cursor);
            continue;
        }

        if is_raw_string_at(text, *cursor) {
            skip_raw_string(text, cursor);
            continue;
        }

        let next = next_char_at(text, *cursor);
        match next {
            '"' => skip_quoted_string(text, cursor, '"', true),
            '\'' => skip_quoted_string(text, cursor, '\'', true),
            '`' => skip_backtick_string(text, cursor),
            '{' => {
                depth += 1;
                *cursor += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(*cursor);
                }
                *cursor += 1;
            }
            _ => {
                *cursor += next.len_utf8();
            }
        }
    }

    None
}

pub(crate) fn skip_block_comment(text: &str, cursor: &mut usize) {
    *cursor += 2;
    let mut depth = 1usize;

    while *cursor < text.len() && depth > 0 {
        if text[*cursor..].starts_with("/*") {
            *cursor += 2;
            depth += 1;
        } else if text[*cursor..].starts_with("*/") {
            *cursor += 2;
            depth -= 1;
        } else {
            *cursor += next_char_at(text, *cursor).len_utf8();
        }
    }
}

pub(crate) fn skip_quoted_string(
    text: &str,
    cursor: &mut usize,
    terminator: char,
    allow_escapes: bool,
) {
    *cursor += terminator.len_utf8();

    while *cursor < text.len() {
        let next = next_char_at(text, *cursor);
        *cursor += next.len_utf8();

        if allow_escapes && next == '\\' && *cursor < text.len() {
            *cursor += next_char_at(text, *cursor).len_utf8();
            continue;
        }

        if next == terminator {
            break;
        }
    }
}

pub(crate) fn skip_backtick_string(text: &str, cursor: &mut usize) {
    *cursor += 1;

    while *cursor < text.len() {
        if text[*cursor..].starts_with("``") {
            *cursor += 2;
            continue;
        }

        if text[*cursor..].starts_with("${") {
            *cursor += 2;
            if let Some(end) = find_interpolation_end(text, cursor) {
                *cursor = end + 1;
            } else {
                break;
            }
            continue;
        }

        let next = next_char_at(text, *cursor);
        *cursor += next.len_utf8();
        if next == '`' {
            break;
        }
    }
}

pub(crate) fn is_raw_string_at(text: &str, mut cursor: usize) -> bool {
    while cursor < text.len() && text[cursor..].starts_with('#') {
        cursor += 1;
    }
    cursor < text.len() && text[cursor..].starts_with('"')
}

pub(crate) fn skip_raw_string(text: &str, cursor: &mut usize) {
    let mut hashes = 0usize;
    while *cursor < text.len() && text[*cursor..].starts_with('#') {
        hashes += 1;
        *cursor += 1;
    }

    *cursor += 1;

    while *cursor < text.len() {
        let next = next_char_at(text, *cursor);
        *cursor += next.len_utf8();
        if next != '"' {
            continue;
        }

        let mut lookahead = *cursor;
        let mut matched = true;
        for _ in 0..hashes {
            if lookahead >= text.len() || !text[lookahead..].starts_with('#') {
                matched = false;
                break;
            }
            lookahead += 1;
        }

        if matched {
            *cursor = lookahead;
            break;
        }
    }
}

pub(crate) fn next_char_at(text: &str, offset: usize) -> char {
    text[offset..]
        .chars()
        .next()
        .expect("valid parser cursor offset")
}

pub(crate) fn infix_binding_power(kind: TokenKind) -> Option<(u8, u8, SyntaxKind)> {
    match kind {
        TokenKind::Eq
        | TokenKind::PlusEq
        | TokenKind::MinusEq
        | TokenKind::StarEq
        | TokenKind::SlashEq
        | TokenKind::PercentEq
        | TokenKind::StarStarEq
        | TokenKind::ShlEq
        | TokenKind::ShrEq
        | TokenKind::AmpEq
        | TokenKind::PipeEq
        | TokenKind::CaretEq
        | TokenKind::QuestionQuestionEq => Some((10, 10, SyntaxKind::ExprAssign)),
        TokenKind::PipePipe | TokenKind::Pipe | TokenKind::Caret => {
            Some((30, 31, SyntaxKind::ExprBinary))
        }
        TokenKind::AmpAmp | TokenKind::Amp => Some((60, 61, SyntaxKind::ExprBinary)),
        TokenKind::EqEq | TokenKind::BangEq => Some((90, 91, SyntaxKind::ExprBinary)),
        TokenKind::InKw => Some((110, 111, SyntaxKind::ExprBinary)),
        TokenKind::Gt | TokenKind::GtEq | TokenKind::Lt | TokenKind::LtEq => {
            Some((130, 131, SyntaxKind::ExprBinary))
        }
        TokenKind::QuestionQuestion => Some((135, 136, SyntaxKind::ExprBinary)),
        TokenKind::Range | TokenKind::RangeEq => Some((140, 141, SyntaxKind::ExprBinary)),
        TokenKind::Plus | TokenKind::Minus => Some((150, 151, SyntaxKind::ExprBinary)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => {
            Some((180, 181, SyntaxKind::ExprBinary))
        }
        TokenKind::StarStar => Some((190, 190, SyntaxKind::ExprBinary)),
        TokenKind::Shl | TokenKind::Shr => Some((210, 211, SyntaxKind::ExprBinary)),
        _ => None,
    }
}
