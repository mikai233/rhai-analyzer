use crate::parser::{BuildElement, BuildNode, Parser, node_element, token_element};
use crate::syntax::{
    SyntaxError, SyntaxKind, SyntaxToken, TextRange, TextSize, TokenKind, empty_range,
};

impl<'a> Parser<'a> {
    pub(crate) fn parse_required_block(&mut self, message: &'static str) -> BuildNode {
        if self.at(TokenKind::OpenBrace) {
            self.parse_block_expr()
        } else {
            self.record_error(message, empty_range(self.current_offset()));
            BuildNode::with_range(
                SyntaxKind::Error,
                empty_range(self.current_offset()),
                Vec::new(),
            )
        }
    }

    pub(crate) fn bump_element(&mut self, expect_message: &'static str) -> BuildElement {
        token_element(self.bump().expect(expect_message), self.source)
    }

    pub(crate) fn eat(
        &mut self,
        kind: TokenKind,
        expect_message: &'static str,
    ) -> Option<BuildElement> {
        self.at(kind).then(|| self.bump_element(expect_message))
    }

    pub(crate) fn expect_token(
        &mut self,
        kind: TokenKind,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> BuildElement {
        self.eat(kind, expect_message)
            .unwrap_or_else(|| self.missing_error(error_message))
    }

    pub(crate) fn expect_ident(
        &mut self,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> BuildElement {
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
    ) -> BuildElement {
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
    ) -> BuildElement {
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
    ) -> BuildElement {
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
    ) -> BuildElement {
        if self.at_object_field_start() {
            self.bump_element(expect_message)
        } else {
            self.missing_error(error_message)
        }
    }

    pub(crate) fn missing_error(&mut self, message: &'static str) -> BuildElement {
        self.record_error(message, empty_range(self.current_offset()));
        node_element(BuildNode::with_range(
            SyntaxKind::Error,
            empty_range(self.current_offset()),
            Vec::new(),
        ))
    }

    pub(crate) fn empty_error_node(&mut self, message: &'static str) -> BuildNode {
        let range = empty_range(self.current_offset());
        self.record_error(message, range);
        BuildNode::with_range(SyntaxKind::Error, range, Vec::new())
    }

    pub(crate) fn unexpected_token_error(&mut self, message: &'static str) -> BuildNode {
        let token = self.bump().expect("unexpected token should be present");
        self.record_error(message, token.range());
        BuildNode::new(
            SyntaxKind::Error,
            vec![token_element(token, self.source)],
            token.range().start(),
        )
    }

    pub(crate) fn record_error(&mut self, message: &'static str, range: TextRange) {
        self.errors.push(SyntaxError::new(message, range));
    }

    pub(crate) fn peek(&self) -> Option<SyntaxToken> {
        self.next_significant_index(self.cursor)
            .and_then(|index| self.tokens.get(index).copied())
    }

    pub(crate) fn bump(&mut self) -> Option<SyntaxToken> {
        let index = self.next_significant_index(self.cursor)?;
        let token = self.tokens[index];
        self.cursor = index + 1;
        Some(token)
    }

    pub(crate) fn peek_n(&self, n: usize) -> Option<SyntaxToken> {
        let mut index = self.cursor;
        let mut remaining = n;
        loop {
            let next = self.next_significant_index(index)?;
            if remaining == 0 {
                return self.tokens.get(next).copied();
            }
            remaining -= 1;
            index = next + 1;
        }
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
        self.tokens
            .get(self.cursor)
            .map_or(self.text_len, |token| token.range().start())
    }

    pub(crate) fn next_significant_offset(&self) -> TextSize {
        self.peek()
            .map_or(self.text_len, |token| token.range().start())
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.next_significant_index(self.cursor).is_none()
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
                    | TokenKind::Backtick
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

    fn next_significant_index(&self, mut index: usize) -> Option<usize> {
        while let Some(token) = self.tokens.get(index) {
            if !token.kind().is_trivia() {
                return Some(index);
            }
            index += 1;
        }
        None
    }
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
