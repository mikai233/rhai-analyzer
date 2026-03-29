use std::sync::Arc;

use crate::lexer::lex_text;
use crate::syntax::{
    Parse, SyntaxError, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, TokenKind,
    empty_range, node_element, text_size_of, token_element,
};

pub fn parse_text(text: &str) -> Parse {
    let lexed = lex_text(text);
    let (tokens, mut errors) = lexed.into_parts();
    let text_len = text_size_of(text);
    let significant_tokens: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| !token.kind().is_trivia())
        .collect();

    let mut parser = Parser::new(significant_tokens, text_len, text);
    let root = parser.parse_root();
    errors.extend(parser.finish_errors());

    Parse::new(Arc::<str>::from(text), tokens, root, errors)
}

struct Parser<'a> {
    tokens: Vec<SyntaxToken>,
    cursor: usize,
    text_len: TextSize,
    errors: Vec<SyntaxError>,
    source: &'a str,
    statement_depth: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<SyntaxToken>, text_len: TextSize, source: &'a str) -> Self {
        Self {
            tokens,
            cursor: 0,
            text_len,
            errors: Vec::new(),
            source,
            statement_depth: 0,
        }
    }

    fn finish_errors(self) -> Vec<SyntaxError> {
        self.errors
    }

    fn parse_root(&mut self) -> SyntaxNode {
        let mut children = Vec::new();

        while !self.is_eof() {
            children.push(node_element(self.parse_stmt()));
        }

        SyntaxNode::with_range(
            SyntaxKind::Root,
            TextRange::new(TextSize::from(0), self.text_len),
            children,
        )
    }

    fn parse_stmt(&mut self) -> SyntaxNode {
        if self.at_fn_item_start() {
            return self.parse_fn_item();
        }

        match self.peek_kind() {
            Some(TokenKind::LetKw) => self.parse_let_stmt(),
            Some(TokenKind::ConstKw) => self.parse_const_stmt(),
            Some(TokenKind::ImportKw) => self.parse_import_stmt(),
            Some(TokenKind::ExportKw) => self.parse_export_stmt(),
            Some(TokenKind::BreakKw) => self.parse_value_stmt(SyntaxKind::StmtBreak),
            Some(TokenKind::ContinueKw) => self.parse_continue_stmt(),
            Some(TokenKind::ReturnKw) => self.parse_value_stmt(SyntaxKind::StmtReturn),
            Some(TokenKind::ThrowKw) => self.parse_value_stmt(SyntaxKind::StmtThrow),
            Some(TokenKind::TryKw) => self.parse_try_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();
        children.push(self.bump_element("`let` token should be present"));

        children.push(self.expect_ident(
            "expected identifier after `let`",
            "identifier token should be present",
        ));

        if self.at(TokenKind::Eq) {
            children.push(self.bump_element("`=` token should be present"));
            children.push(node_element(self.parse_expr(0)));
        }

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtLet, children, start)
    }

    fn parse_const_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`const` token should be present")];

        children.push(self.expect_ident(
            "expected constant name after `const`",
            "constant name token should be present",
        ));

        if self.at(TokenKind::Eq) {
            children.push(self.bump_element("`=` token should be present"));
        } else {
            children.push(self.missing_error("expected `=` in `const` statement"));
        }

        if self.is_stmt_terminator() {
            children.push(self.missing_error("expected constant value"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtConst, children, start)
    }

    fn parse_import_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`import` token should be present")];

        if self.is_stmt_terminator() {
            children.push(self.missing_error("expected module path after `import`"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        if self.at(TokenKind::AsKw) {
            children.push(node_element(self.parse_alias_clause()));
        }

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtImport, children, start)
    }

    fn parse_export_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`export` token should be present")];

        if self.statement_depth > 0 {
            self.record_error(
                "the `export` statement can only be used at global level",
                empty_range(start),
            );
        }

        let mut parsed_declaration = false;
        let mut parsed_plain_target = false;
        match self.peek_kind() {
            Some(TokenKind::LetKw) => {
                children.push(node_element(self.parse_let_stmt()));
                parsed_declaration = true;
            }
            Some(TokenKind::ConstKw) => {
                children.push(node_element(self.parse_const_stmt()));
                parsed_declaration = true;
            }
            Some(TokenKind::Ident) => {
                children.push(node_element(self.parse_name_expr()));
                parsed_plain_target = true;
            }
            Some(_) if self.at_expr_start() => {
                let target = self.parse_expr(0);
                self.record_error(
                    "expected exported variable name or `let`/`const` declaration after `export`",
                    target.range(),
                );
                children.push(node_element(target));
            }
            _ => {
                children.push(self.missing_error("expected export target after `export`"));
            }
        }

        if parsed_plain_target && self.at(TokenKind::AsKw) {
            children.push(node_element(self.parse_alias_clause()));
        } else if parsed_declaration && self.at(TokenKind::AsKw) {
            let alias = self.parse_alias_clause();
            self.record_error(
                "exported `let`/`const` declarations cannot be renamed with `as`",
                alias.range(),
            );
            children.push(node_element(alias));
        }

        if !parsed_declaration
            && let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present")
        {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtExport, children, start)
    }

    fn parse_fn_item(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        if self.statement_depth > 0 {
            self.record_error(
                "functions can only be defined at global level",
                empty_range(start),
            );
        }

        if let Some(private_kw) =
            self.eat(TokenKind::PrivateKw, "`private` token should be present")
        {
            children.push(private_kw);
        }

        children.push(self.bump_element("`fn` token should be present"));
        children.extend(self.parse_fn_name_parts());

        children.push(node_element(self.parse_param_list()));
        children.push(node_element(
            self.parse_required_block("expected function body after parameter list"),
        ));

        SyntaxNode::new(SyntaxKind::ItemFn, children, start)
    }

    fn parse_fn_name_parts(&mut self) -> Vec<crate::SyntaxElement> {
        let mut children = Vec::new();

        match self.peek_kind() {
            Some(TokenKind::Ident) => {
                children
                    .push(token_element(self.bump().expect(
                        "function name or typed receiver token should be present",
                    )));

                if self.at(TokenKind::Dot) {
                    children.push(self.bump_element("`.` token should be present"));
                    children.push(self.expect_ident(
                        "expected method name after `.` in typed method definition",
                        "typed method name token should be present",
                    ));
                }
            }
            Some(TokenKind::String) => {
                children.push(token_element(
                    self.bump()
                        .expect("typed receiver string token should be present"),
                ));

                if self.at(TokenKind::Dot) {
                    children.push(self.bump_element("`.` token should be present"));
                    children.push(self.expect_ident(
                        "expected method name after `.` in typed method definition",
                        "typed method name token should be present",
                    ));
                } else {
                    children.push(self.missing_error("expected `.` after typed method receiver"));
                }
            }
            _ => {
                children.push(self.expect_ident(
                    "expected function name after `fn`",
                    "function name token should be present",
                ));
            }
        }

        children
    }

    fn parse_value_stmt(&mut self, kind: SyntaxKind) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("statement keyword token should be present")];

        if !self.is_stmt_terminator() {
            children.push(node_element(self.parse_expr(0)));
        }

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(kind, children, start)
    }

    fn parse_continue_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`continue` token should be present")];

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtContinue, children, start)
    }

    fn parse_try_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`try` token should be present")];
        children.push(node_element(
            self.parse_required_block("expected block after `try`"),
        ));

        if self.at(TokenKind::CatchKw) {
            children.push(node_element(self.parse_catch_clause()));
        } else {
            children.push(self.missing_error("expected `catch` clause after `try`"));
        }

        SyntaxNode::new(SyntaxKind::StmtTry, children, start)
    }

    fn parse_expr_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![node_element(self.parse_expr(0))];

        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
        }

        SyntaxNode::new(SyntaxKind::StmtExpr, children, start)
    }

    fn parse_expr(&mut self, min_binding_power: u8) -> SyntaxNode {
        let mut lhs = self.parse_prefix_expr();

        loop {
            let Some(op) = self.peek() else {
                break;
            };
            let Some((left_bp, right_bp, expr_kind)) = infix_binding_power(op.kind()) else {
                break;
            };
            if left_bp < min_binding_power {
                break;
            }

            let start = lhs.range().start();
            let op_token = self.bump_element("operator token should be present");
            let rhs = if self.at_expr_start() {
                self.parse_expr(right_bp)
            } else {
                let range = empty_range(self.current_offset());
                self.record_error("expected expression after operator", range);
                SyntaxNode::with_range(SyntaxKind::Error, range, Vec::new())
            };

            lhs = SyntaxNode::new(
                expr_kind,
                vec![node_element(lhs), op_token, node_element(rhs)],
                start,
            );
        }

        lhs
    }

    fn parse_prefix_expr(&mut self) -> SyntaxNode {
        match self.peek_kind() {
            Some(TokenKind::Plus | TokenKind::Minus | TokenKind::Bang) => {
                let start = self.current_offset();
                let operator = self.bump_element("unary operator token should be present");
                let operand = node_element(self.parse_prefix_expr());
                SyntaxNode::new(SyntaxKind::ExprUnary, vec![operator, operand], start)
            }
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_postfix_expr(&mut self) -> SyntaxNode {
        let mut expr = self.parse_primary_expr();

        loop {
            expr = match self.peek_kind() {
                Some(TokenKind::Bang)
                    if self
                        .peek_n(1)
                        .is_some_and(|token| token.kind() == TokenKind::OpenParen) =>
                {
                    self.parse_call_expr(expr, true)
                }
                Some(TokenKind::OpenParen) => self.parse_call_expr(expr, false),
                Some(TokenKind::ColonColon) => self.parse_path_expr(expr),
                Some(TokenKind::OpenBracket | TokenKind::QuestionOpenBracket) => {
                    self.parse_index_expr(expr)
                }
                Some(TokenKind::Dot | TokenKind::QuestionDot) => self.parse_field_expr(expr),
                _ => break,
            };
        }

        expr
    }

    fn parse_primary_expr(&mut self) -> SyntaxNode {
        let Some(token) = self.peek() else {
            return self.empty_error_node("expected expression");
        };

        match token.kind() {
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
            | TokenKind::EvalKw => SyntaxNode::new(
                SyntaxKind::ExprName,
                vec![token_element(
                    self.bump().expect("identifier token should be present"),
                )],
                token.range().start(),
            ),
            TokenKind::Int
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::RawString
            | TokenKind::Char
            | TokenKind::TrueKw
            | TokenKind::FalseKw => SyntaxNode::new(
                SyntaxKind::ExprLiteral,
                vec![token_element(
                    self.bump().expect("literal token should be present"),
                )],
                token.range().start(),
            ),
            TokenKind::IfKw => self.parse_if_expr(),
            TokenKind::SwitchKw => self.parse_switch_expr(),
            TokenKind::WhileKw => self.parse_while_expr(),
            TokenKind::LoopKw => self.parse_loop_expr(),
            TokenKind::ForKw => self.parse_for_expr(),
            TokenKind::DoKw => self.parse_do_expr(),
            TokenKind::Pipe | TokenKind::PipePipe => self.parse_closure_expr(),
            TokenKind::BacktickString => self.parse_backtick_string_expr(),
            TokenKind::OpenBracket => self.parse_array_expr(),
            TokenKind::HashBraceOpen => self.parse_object_expr(),
            TokenKind::OpenParen => self.parse_paren_expr(),
            TokenKind::OpenBrace => self.parse_block_expr(),
            _ => self.unexpected_token_error("expected expression"),
        }
    }

    fn parse_name_expr(&mut self) -> SyntaxNode {
        let token = self.bump().expect("identifier token should be present");
        SyntaxNode::new(
            SyntaxKind::ExprName,
            vec![token_element(token)],
            token.range().start(),
        )
    }

    fn parse_switch_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`switch` token should be present")];
        children.push(node_element(self.parse_expr(0)));

        children.push(self.expect_token(
            TokenKind::OpenBrace,
            "expected `{` after `switch` expression",
            "`{` token should be present",
        ));
        if matches!(children.last(), Some(crate::SyntaxElement::Node(node)) if node.kind() == SyntaxKind::Error)
        {
            return SyntaxNode::new(SyntaxKind::ExprSwitch, children, start);
        }

        while !self.is_eof() && !self.at(TokenKind::CloseBrace) {
            children.push(node_element(self.parse_switch_arm()));

            if self.at(TokenKind::Comma) {
                children.push(self.bump_element("`,` token should be present"));
            } else if !self.at(TokenKind::CloseBrace) {
                children.push(self.missing_error("expected `,` between `switch` arms"));
                self.recover_to_any(&[TokenKind::Comma, TokenKind::CloseBrace]);
                if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                    children.push(comma);
                }
            }
        }

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close `switch` expression",
            "`}` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ExprSwitch, children, start)
    }

    fn parse_if_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`if` token should be present")];
        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `if` condition"),
        ));

        if self.at(TokenKind::ElseKw) {
            children.push(node_element(self.parse_else_branch()));
        }

        SyntaxNode::new(SyntaxKind::ExprIf, children, start)
    }

    fn parse_else_branch(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`else` token should be present")];

        if self.at(TokenKind::IfKw) {
            children.push(node_element(self.parse_if_expr()));
        } else {
            children.push(node_element(
                self.parse_required_block("expected block or `if` after `else`"),
            ));
        }

        SyntaxNode::new(SyntaxKind::ElseBranch, children, start)
    }

    fn parse_while_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`while` token should be present")];
        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `while` condition"),
        ));

        SyntaxNode::new(SyntaxKind::ExprWhile, children, start)
    }

    fn parse_loop_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`loop` token should be present")];
        children.push(node_element(
            self.parse_required_block("expected block after `loop`"),
        ));

        SyntaxNode::new(SyntaxKind::ExprLoop, children, start)
    }

    fn parse_for_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`for` token should be present")];
        children.push(node_element(self.parse_for_bindings()));

        if self.at(TokenKind::InKw) {
            children.push(self.bump_element("`in` token should be present"));
        } else {
            children.push(self.missing_error("expected `in` in `for` expression"));
        }

        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `for` expression"),
        ));

        SyntaxNode::new(SyntaxKind::ExprFor, children, start)
    }

    fn parse_do_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`do` token should be present")];
        children.push(node_element(
            self.parse_required_block("expected block after `do`"),
        ));

        let condition_start = self.current_offset();
        let mut condition_children = Vec::new();
        if self.at(TokenKind::WhileKw) || self.at(TokenKind::UntilKw) {
            condition_children.push(token_element(
                self.bump()
                    .expect("`while` or `until` token should be present"),
            ));
            condition_children.push(node_element(self.parse_expr(0)));
        } else {
            condition_children
                .push(self.missing_error("expected `while` or `until` after `do` block"));
        }

        children.push(node_element(SyntaxNode::new(
            SyntaxKind::DoCondition,
            condition_children,
            condition_start,
        )));

        SyntaxNode::new(SyntaxKind::ExprDo, children, start)
    }

    fn parse_closure_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![node_element(self.parse_closure_param_list())];

        if self.at(TokenKind::OpenBrace) {
            children.push(node_element(self.parse_block_expr()));
        } else if self.at_expr_start() {
            children.push(node_element(self.parse_expr(0)));
        } else {
            children.push(self.missing_error("expected closure body"));
        }

        SyntaxNode::new(SyntaxKind::ExprClosure, children, start)
    }

    fn parse_backtick_string_expr(&mut self) -> SyntaxNode {
        let token = self
            .bump()
            .expect("back-tick string token should be present");
        let text = token.text(self.source);

        if !text.contains("${") {
            return SyntaxNode::new(
                SyntaxKind::ExprLiteral,
                vec![token_element(token)],
                token.range().start(),
            );
        }

        let mut children = vec![token_element(SyntaxToken::new(
            TokenKind::Backtick,
            TextRange::new(
                token.range().start(),
                token.range().start() + TextSize::from(1),
            ),
        ))];

        let ranges = self.collect_interpolated_string_parts(token, text);
        for part in ranges {
            match part {
                InterpolatedPart::Text(range) => {
                    if range.is_empty() {
                        continue;
                    }
                    let fragment = SyntaxToken::new(TokenKind::StringText, range);
                    children.push(node_element(SyntaxNode::new(
                        SyntaxKind::StringSegment,
                        vec![token_element(fragment)],
                        range.start(),
                    )));
                }
                InterpolatedPart::Interpolation {
                    start,
                    body_range,
                    end,
                } => {
                    let mut interpolation_children = vec![token_element(SyntaxToken::new(
                        TokenKind::InterpolationStart,
                        start,
                    ))];
                    let body = self.parse_interpolation_body(body_range);
                    interpolation_children.push(node_element(body));
                    interpolation_children
                        .push(token_element(SyntaxToken::new(TokenKind::CloseBrace, end)));
                    children.push(node_element(SyntaxNode::new(
                        SyntaxKind::StringInterpolation,
                        interpolation_children,
                        start.start(),
                    )));
                }
            }
        }

        children.push(token_element(SyntaxToken::new(
            TokenKind::Backtick,
            TextRange::new(token.range().end() - TextSize::from(1), token.range().end()),
        )));

        SyntaxNode::new(
            SyntaxKind::ExprInterpolatedString,
            children,
            token.range().start(),
        )
    }

    fn collect_interpolated_string_parts(
        &mut self,
        token: SyntaxToken,
        text: &str,
    ) -> Vec<InterpolatedPart> {
        let mut parts = Vec::new();
        let absolute_start = u32::from(token.range().start()) as usize;
        let mut cursor = 1usize;
        let closing = text.len().saturating_sub(1);
        let mut segment_start = cursor;

        while cursor < closing {
            if text[cursor..].starts_with("``") {
                cursor += 2;
                continue;
            }

            if text[cursor..].starts_with("${") {
                if segment_start < cursor {
                    parts.push(InterpolatedPart::Text(make_absolute_range(
                        absolute_start + segment_start,
                        absolute_start + cursor,
                    )));
                }

                let interpolation_start = cursor;
                cursor += 2;
                let body_start = cursor;
                let body_end = find_interpolation_end(text, &mut cursor);

                if let Some(body_end) = body_end {
                    parts.push(InterpolatedPart::Interpolation {
                        start: make_absolute_range(
                            absolute_start + interpolation_start,
                            absolute_start + interpolation_start + 2,
                        ),
                        body_range: make_absolute_range(
                            absolute_start + body_start,
                            absolute_start + body_end,
                        ),
                        end: make_absolute_range(
                            absolute_start + body_end,
                            absolute_start + body_end + 1,
                        ),
                    });
                    cursor += 1;
                    segment_start = cursor;
                    continue;
                }

                self.record_error(
                    "unterminated string interpolation",
                    make_absolute_range(
                        absolute_start + interpolation_start,
                        absolute_start + closing,
                    ),
                );
                break;
            }

            cursor += next_char_at(text, cursor).len_utf8();
        }

        if segment_start < closing {
            parts.push(InterpolatedPart::Text(make_absolute_range(
                absolute_start + segment_start,
                absolute_start + closing,
            )));
        }

        parts
    }

    fn parse_interpolation_body(&mut self, body_range: TextRange) -> SyntaxNode {
        let start = u32::from(body_range.start()) as usize;
        let end = u32::from(body_range.end()) as usize;
        let body_text = &self.source[start..end];

        let lexed = lex_text(body_text);
        let (tokens, errors) = lexed.into_parts();
        let shifted_offset = body_range.start();

        self.errors.extend(
            errors
                .into_iter()
                .map(|error| shift_error(error, shifted_offset)),
        );

        let significant_tokens: Vec<_> = tokens
            .iter()
            .copied()
            .filter(|token| !token.kind().is_trivia())
            .collect();

        let mut parser = Parser::new(significant_tokens, text_size_of(body_text), body_text);
        let root = parser.parse_root();
        self.errors.extend(
            parser
                .finish_errors()
                .into_iter()
                .map(|error| shift_error(error, shifted_offset)),
        );

        let shifted_children = root
            .children()
            .iter()
            .cloned()
            .map(|element| shift_element(element, shifted_offset))
            .collect();

        SyntaxNode::with_range(SyntaxKind::InterpolationBody, body_range, shifted_children)
    }

    fn parse_for_bindings(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        if self.at(TokenKind::OpenParen) {
            children.push(self.bump_element("`(` token should be present"));
            children.push(self.expect_binding(
                "expected first binding in `for`",
                "binding token should be present",
            ));
            children.push(self.expect_token(
                TokenKind::Comma,
                "expected `,` in `for` bindings",
                "`,` token should be present",
            ));
            children.push(self.expect_binding(
                "expected second binding in `for`",
                "binding token should be present",
            ));
            children.push(self.expect_token(
                TokenKind::CloseParen,
                "expected `)` after `for` bindings",
                "`)` token should be present",
            ));
        } else if self.at_binding_start() {
            children.push(self.bump_element("binding token should be present"));
        } else {
            children.push(self.missing_error("expected binding in `for` expression"));
        }

        SyntaxNode::new(SyntaxKind::ForBindings, children, start)
    }

    fn parse_catch_clause(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`catch` token should be present")];

        if self.at(TokenKind::OpenParen) {
            children.push(self.bump_element("`(` token should be present"));
            children.push(self.expect_binding(
                "expected binding in `catch` clause",
                "catch binding token should be present",
            ));
            children.push(self.expect_token(
                TokenKind::CloseParen,
                "expected `)` after `catch` binding",
                "`)` token should be present",
            ));
        }

        children.push(node_element(
            self.parse_required_block("expected block after `catch`"),
        ));

        SyntaxNode::new(SyntaxKind::CatchClause, children, start)
    }

    fn parse_alias_clause(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`as` token should be present")];
        children.push(
            self.expect_alias_name("expected alias after `as`", "alias token should be present"),
        );

        SyntaxNode::new(SyntaxKind::AliasClause, children, start)
    }

    fn parse_switch_arm(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![node_element(self.parse_switch_pattern_list())];

        children.push(self.expect_token(
            TokenKind::FatArrow,
            "expected `=>` in `switch` arm",
            "`=>` token should be present",
        ));

        children.push(node_element(self.parse_switch_arm_body()));

        SyntaxNode::new(SyntaxKind::SwitchArm, children, start)
    }

    fn parse_switch_pattern_list(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        loop {
            if self.at(TokenKind::Underscore) {
                children.push(self.bump_element("`_` token should be present"));
            } else {
                children.push(node_element(self.parse_expr(0)));
            }

            if let Some(pipe) = self.eat(TokenKind::Pipe, "`|` token should be present") {
                children.push(pipe);
            } else {
                break;
            }
        }

        SyntaxNode::new(SyntaxKind::SwitchPatternList, children, start)
    }

    fn parse_switch_arm_body(&mut self) -> SyntaxNode {
        if self.at(TokenKind::OpenBrace) {
            self.parse_block_expr()
        } else {
            self.parse_expr(0)
        }
    }

    fn parse_param_list(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        if self.at(TokenKind::OpenParen) {
            children.push(self.bump_element("`(` token should be present"));
        } else {
            children.push(self.missing_error("expected `(` after function name"));
            return SyntaxNode::new(SyntaxKind::ParamList, children, start);
        }

        while !self.is_eof() && !self.at(TokenKind::CloseParen) {
            if self.at_param_start() {
                children.push(self.bump_element("parameter token should be present"));
            } else {
                children.push(self.missing_error("expected parameter name"));
                self.recover_to_any(&[
                    TokenKind::Comma,
                    TokenKind::CloseParen,
                    TokenKind::OpenBrace,
                    TokenKind::Semicolon,
                ]);
            }

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                children.push(comma);
            } else if self.at_param_start() {
                children.push(self.missing_error("expected `,` between parameters"));
            } else {
                break;
            }
        }

        children.push(self.expect_token(
            TokenKind::CloseParen,
            "expected `)` after parameters",
            "`)` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ParamList, children, start)
    }

    fn parse_closure_param_list(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let open = self
            .bump()
            .expect("leading closure delimiter token should be present");
        let mut children = vec![token_element(open)];

        if open.kind() == TokenKind::PipePipe {
            return SyntaxNode::new(SyntaxKind::ClosureParamList, children, start);
        }

        while !self.is_eof() && !self.at(TokenKind::Pipe) {
            if self.at_param_start() {
                children.push(self.bump_element("closure parameter token should be present"));
            } else {
                children.push(self.missing_error("expected closure parameter"));
                self.recover_to_any(&[
                    TokenKind::Comma,
                    TokenKind::Pipe,
                    TokenKind::OpenBrace,
                    TokenKind::Semicolon,
                ]);
            }

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                children.push(comma);
            } else if self.at_param_start() {
                children.push(self.missing_error("expected `,` between closure parameters"));
            } else {
                break;
            }
        }

        children.push(self.expect_token(
            TokenKind::Pipe,
            "expected closing `|` for closure parameters",
            "closing `|` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ClosureParamList, children, start)
    }

    fn parse_call_expr(&mut self, callee: SyntaxNode, caller_scope: bool) -> SyntaxNode {
        let start = callee.range().start();
        let mut children = vec![node_element(callee)];
        if caller_scope {
            self.validate_caller_scope_callee(&children[0]);
            children.push(self.bump_element("`!` token should be present"));
        }
        children.push(self.bump_element("`(` token should be present"));

        let mut arg_children = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::CloseParen) {
            arg_children.push(node_element(self.parse_expr(0)));

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                arg_children.push(comma);
            } else if self.at_expr_start() {
                arg_children.push(self.missing_error("expected `,` between arguments"));
            } else {
                break;
            }
        }

        children.push(node_element(SyntaxNode::new(
            SyntaxKind::ArgList,
            arg_children,
            self.current_offset(),
        )));

        children.push(self.expect_token(
            TokenKind::CloseParen,
            "expected `)` to close argument list",
            "`)` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ExprCall, children, start)
    }

    fn validate_caller_scope_callee(&mut self, callee: &crate::SyntaxElement) {
        let Some(callee) = callee.as_node() else {
            return;
        };

        match callee.kind() {
            SyntaxKind::ExprName => {}
            SyntaxKind::ExprField => {
                self.record_error(
                    "caller-scope function calls cannot use method-call style",
                    callee.range(),
                );
            }
            SyntaxKind::ExprPath => {
                self.record_error(
                    "caller-scope function calls cannot use namespace-qualified paths",
                    callee.range(),
                );
            }
            _ => {
                self.record_error(
                    "caller-scope function calls require a bare function name",
                    callee.range(),
                );
            }
        }
    }

    fn parse_path_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];

        while self.at(TokenKind::ColonColon) {
            children.push(self.bump_element("`::` token should be present"));
            children.push(self.expect_name_like(
                "expected path segment after `::`",
                "path segment token should be present",
            ));
            if matches!(children.last(), Some(crate::SyntaxElement::Node(node)) if node.kind() == SyntaxKind::Error)
            {
                break;
            }
        }

        SyntaxNode::new(SyntaxKind::ExprPath, children, start)
    }

    fn parse_index_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];
        children.push(token_element(
            self.bump().expect("index opener token should be present"),
        ));

        if self.at(TokenKind::CloseBracket) {
            children.push(self.missing_error("expected index expression"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        if self.at(TokenKind::CloseBracket) {
            children.push(token_element(
                self.bump().expect("`]` token should be present"),
            ));
        } else {
            children.push(self.missing_error("expected `]` to close index expression"));
        }

        SyntaxNode::new(SyntaxKind::ExprIndex, children, start)
    }

    fn parse_field_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];
        children.push(token_element(
            self.bump().expect("field access token should be present"),
        ));

        if self.at_name_like() {
            children.push(token_element(
                self.bump()
                    .expect("field identifier token should be present"),
            ));
        } else {
            children.push(self.missing_error("expected property name after field access"));
        }

        SyntaxNode::new(SyntaxKind::ExprField, children, start)
    }

    fn parse_array_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`[` token should be present")];

        let mut item_children = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::CloseBracket) {
            item_children.push(node_element(self.parse_expr(0)));

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                item_children.push(comma);
            } else if self.at_expr_start() {
                item_children.push(self.missing_error("expected `,` between array items"));
            } else {
                break;
            }
        }

        children.push(node_element(SyntaxNode::new(
            SyntaxKind::ArrayItemList,
            item_children,
            self.current_offset(),
        )));

        children.push(self.expect_token(
            TokenKind::CloseBracket,
            "expected `]` to close array literal",
            "`]` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ExprArray, children, start)
    }

    fn parse_object_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`#{` token should be present")];

        while !self.is_eof() && !self.at(TokenKind::CloseBrace) {
            children.push(node_element(self.parse_object_field()));

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                children.push(comma);
            } else if self.at_object_field_start() {
                children.push(self.missing_error("expected `,` between object fields"));
            } else {
                break;
            }
        }

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close object map literal",
            "`}` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ExprObject, children, start)
    }

    fn parse_object_field(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        children.push(self.expect_object_field_name(
            "expected property name",
            "object field name token should be present",
        ));
        children.push(self.expect_token(
            TokenKind::Colon,
            "expected `:` after property name",
            "`:` token should be present",
        ));

        if self.at(TokenKind::Comma) || self.at(TokenKind::CloseBrace) {
            children.push(self.missing_error("expected property value"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        SyntaxNode::new(SyntaxKind::ObjectField, children, start)
    }

    fn parse_paren_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`(` token should be present")];

        if self.at(TokenKind::CloseParen) {
            children.push(self.missing_error("expected expression inside parentheses"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        children.push(self.expect_token(
            TokenKind::CloseParen,
            "expected `)` to close parenthesized expression",
            "`)` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::ExprParen, children, start)
    }

    fn parse_block_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`{` token should be present")];

        self.statement_depth += 1;
        while !self.is_eof() && !self.at(TokenKind::CloseBrace) {
            children.push(node_element(self.parse_stmt()));
        }
        self.statement_depth = self.statement_depth.saturating_sub(1);

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close block",
            "`}` token should be present",
        ));

        SyntaxNode::new(SyntaxKind::Block, children, start)
    }

    fn parse_required_block(&mut self, message: &'static str) -> SyntaxNode {
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

    fn bump_element(&mut self, expect_message: &'static str) -> crate::SyntaxElement {
        token_element(self.bump().expect(expect_message))
    }

    fn eat(
        &mut self,
        kind: TokenKind,
        expect_message: &'static str,
    ) -> Option<crate::SyntaxElement> {
        self.at(kind).then(|| self.bump_element(expect_message))
    }

    fn expect_token(
        &mut self,
        kind: TokenKind,
        error_message: &'static str,
        expect_message: &'static str,
    ) -> crate::SyntaxElement {
        self.eat(kind, expect_message)
            .unwrap_or_else(|| self.missing_error(error_message))
    }

    fn expect_ident(
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

    fn at_binding_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Ident | TokenKind::Underscore)
        )
    }

    fn expect_binding(
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

    fn expect_alias_name(
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

    fn expect_name_like(
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

    fn expect_object_field_name(
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

    fn missing_error(&mut self, message: &'static str) -> crate::SyntaxElement {
        self.record_error(message, empty_range(self.current_offset()));
        node_element(SyntaxNode::with_range(
            SyntaxKind::Error,
            empty_range(self.current_offset()),
            Vec::new(),
        ))
    }

    fn empty_error_node(&mut self, message: &'static str) -> SyntaxNode {
        let range = empty_range(self.current_offset());
        self.record_error(message, range);
        SyntaxNode::with_range(SyntaxKind::Error, range, Vec::new())
    }

    fn unexpected_token_error(&mut self, message: &'static str) -> SyntaxNode {
        let token = self.bump().expect("unexpected token should be present");
        self.record_error(message, token.range());
        SyntaxNode::new(
            SyntaxKind::Error,
            vec![token_element(token)],
            token.range().start(),
        )
    }

    fn record_error(&mut self, message: &'static str, range: TextRange) {
        self.errors.push(SyntaxError::new(message, range));
    }

    fn peek(&self) -> Option<SyntaxToken> {
        self.tokens.get(self.cursor).copied()
    }

    fn bump(&mut self) -> Option<SyntaxToken> {
        let token = self.peek()?;
        self.cursor += 1;
        Some(token)
    }

    fn peek_n(&self, n: usize) -> Option<SyntaxToken> {
        self.tokens.get(self.cursor + n).copied()
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|token| token.kind())
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind() == kind)
    }

    fn at_fn_item_start(&self) -> bool {
        matches!(
            (self.peek_kind(), self.peek_n(1).map(|token| token.kind())),
            (Some(TokenKind::FnKw), _) | (Some(TokenKind::PrivateKw), Some(TokenKind::FnKw))
        )
    }

    fn current_offset(&self) -> TextSize {
        self.peek()
            .map_or(self.text_len, |token| token.range().start())
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn is_stmt_terminator(&self) -> bool {
        self.is_eof() || self.at(TokenKind::Semicolon) || self.at(TokenKind::CloseBrace)
    }

    fn at_expr_start(&self) -> bool {
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

    fn at_param_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Ident | TokenKind::Underscore | TokenKind::ThisKw)
        )
    }

    fn at_object_field_start(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Ident | TokenKind::String))
    }

    fn at_name_like(&self) -> bool {
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

    fn recover_to_any(&mut self, kinds: &[TokenKind]) {
        while !self.is_eof() && !kinds.iter().copied().any(|kind| self.at(kind)) {
            self.bump();
        }
    }
}

enum InterpolatedPart {
    Text(TextRange),
    Interpolation {
        start: TextRange,
        body_range: TextRange,
        end: TextRange,
    },
}

fn shift_error(error: SyntaxError, offset: TextSize) -> SyntaxError {
    SyntaxError::new(
        error.message().to_owned(),
        shift_range(error.range(), offset),
    )
}

fn shift_element(element: crate::SyntaxElement, offset: TextSize) -> crate::SyntaxElement {
    match element {
        crate::SyntaxElement::Node(node) => node_element(shift_node(*node, offset)),
        crate::SyntaxElement::Token(token) => token_element(SyntaxToken::new(
            token.kind(),
            shift_range(token.range(), offset),
        )),
    }
}

fn shift_node(node: SyntaxNode, offset: TextSize) -> SyntaxNode {
    let children = node
        .children()
        .iter()
        .cloned()
        .map(|element| shift_element(element, offset))
        .collect();

    SyntaxNode::with_range(node.kind(), shift_range(node.range(), offset), children)
}

fn shift_range(range: TextRange, offset: TextSize) -> TextRange {
    TextRange::new(range.start() + offset, range.end() + offset)
}

fn make_absolute_range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::from(u32::try_from(start).unwrap_or(u32::MAX)),
        TextSize::from(u32::try_from(end).unwrap_or(u32::MAX)),
    )
}

fn find_interpolation_end(text: &str, cursor: &mut usize) -> Option<usize> {
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

fn skip_block_comment(text: &str, cursor: &mut usize) {
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

fn skip_quoted_string(text: &str, cursor: &mut usize, terminator: char, allow_escapes: bool) {
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

fn skip_backtick_string(text: &str, cursor: &mut usize) {
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

fn is_raw_string_at(text: &str, mut cursor: usize) -> bool {
    while cursor < text.len() && text[cursor..].starts_with('#') {
        cursor += 1;
    }
    cursor < text.len() && text[cursor..].starts_with('"')
}

fn skip_raw_string(text: &str, cursor: &mut usize) {
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

fn next_char_at(text: &str, offset: usize) -> char {
    text[offset..]
        .chars()
        .next()
        .expect("valid parser cursor offset")
}

fn infix_binding_power(kind: TokenKind) -> Option<(u8, u8, SyntaxKind)> {
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
