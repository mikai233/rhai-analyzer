use crate::lexer::lex_text;
use crate::parser::{
    InterpolatedPart, Parser, find_interpolation_end, infix_binding_power, make_absolute_range,
    next_char_at, shift_element, shift_error,
};
use crate::syntax::{
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, TokenKind, empty_range, node_element,
    text_size_of, token_element,
};

impl<'a> Parser<'a> {
    pub(crate) fn parse_expr(&mut self, min_binding_power: u8) -> SyntaxNode {
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

    pub(crate) fn parse_prefix_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_postfix_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_primary_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_name_expr(&mut self) -> SyntaxNode {
        let token = self.bump().expect("identifier token should be present");
        SyntaxNode::new(
            SyntaxKind::ExprName,
            vec![token_element(token)],
            token.range().start(),
        )
    }

    pub(crate) fn parse_switch_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_if_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_else_branch(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_while_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`while` token should be present")];
        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `while` condition"),
        ));

        SyntaxNode::new(SyntaxKind::ExprWhile, children, start)
    }

    pub(crate) fn parse_loop_expr(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`loop` token should be present")];
        children.push(node_element(
            self.parse_required_block("expected block after `loop`"),
        ));

        SyntaxNode::new(SyntaxKind::ExprLoop, children, start)
    }

    pub(crate) fn parse_for_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_do_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_closure_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_backtick_string_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn collect_interpolated_string_parts(
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

    pub(crate) fn parse_interpolation_body(&mut self, body_range: TextRange) -> SyntaxNode {
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

    pub(crate) fn parse_for_bindings(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_switch_arm(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_switch_pattern_list(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_switch_arm_body(&mut self) -> SyntaxNode {
        if self.at(TokenKind::OpenBrace) {
            self.parse_block_expr()
        } else {
            self.parse_expr(0)
        }
    }

    pub(crate) fn parse_call_expr(&mut self, callee: SyntaxNode, caller_scope: bool) -> SyntaxNode {
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

    pub(crate) fn validate_caller_scope_callee(&mut self, callee: &crate::SyntaxElement) {
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

    pub(crate) fn parse_path_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
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

    pub(crate) fn parse_index_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
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

    pub(crate) fn parse_field_expr(&mut self, base: SyntaxNode) -> SyntaxNode {
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

    pub(crate) fn parse_array_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_object_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_object_field(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_paren_expr(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_block_expr(&mut self) -> SyntaxNode {
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
}
