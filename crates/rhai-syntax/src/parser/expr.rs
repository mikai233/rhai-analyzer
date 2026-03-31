use crate::parser::{
    BuildElement, BuildNode, Parser, infix_binding_power, node_element, token_element,
};
use crate::syntax::{SyntaxKind, TextRange, TokenKind, empty_range};
use rowan::NodeOrToken;

impl<'a> Parser<'a> {
    pub(crate) fn parse_expr(&mut self, min_binding_power: u8) -> BuildNode {
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
                BuildNode::with_range(SyntaxKind::Error, range, Vec::new())
            };

            lhs = self.finish_node(
                expr_kind,
                vec![node_element(lhs), op_token, node_element(rhs)],
                start,
            );
        }

        lhs
    }

    pub(crate) fn parse_prefix_expr(&mut self) -> BuildNode {
        match self.peek_kind() {
            Some(TokenKind::Plus | TokenKind::Minus | TokenKind::Bang) => {
                let start = self.current_offset();
                let operator = self.bump_element("unary operator token should be present");
                let operand = node_element(self.parse_prefix_expr());
                self.finish_node(SyntaxKind::ExprUnary, vec![operator, operand], start)
            }
            _ => self.parse_postfix_expr(),
        }
    }

    pub(crate) fn parse_postfix_expr(&mut self) -> BuildNode {
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

    pub(crate) fn parse_primary_expr(&mut self) -> BuildNode {
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
            | TokenKind::EvalKw => {
                let bumped = self.bump().expect("identifier token should be present");
                self.finish_node(
                    SyntaxKind::ExprName,
                    vec![token_element(bumped, self.source)],
                    token.range().start(),
                )
            }
            TokenKind::Int
            | TokenKind::Float
            | TokenKind::String
            | TokenKind::RawString
            | TokenKind::Char
            | TokenKind::TrueKw
            | TokenKind::FalseKw => {
                let bumped = self.bump().expect("literal token should be present");
                self.finish_node(
                    SyntaxKind::ExprLiteral,
                    vec![token_element(bumped, self.source)],
                    token.range().start(),
                )
            }
            TokenKind::IfKw => self.parse_if_expr(),
            TokenKind::SwitchKw => self.parse_switch_expr(),
            TokenKind::WhileKw => self.parse_while_expr(),
            TokenKind::LoopKw => self.parse_loop_expr(),
            TokenKind::ForKw => self.parse_for_expr(),
            TokenKind::DoKw => self.parse_do_expr(),
            TokenKind::Pipe | TokenKind::PipePipe => self.parse_closure_expr(),
            TokenKind::BacktickString | TokenKind::Backtick => self.parse_backtick_string_expr(),
            TokenKind::OpenBracket => self.parse_array_expr(),
            TokenKind::HashBraceOpen => self.parse_object_expr(),
            TokenKind::OpenParen => self.parse_paren_expr(),
            TokenKind::OpenBrace => self.parse_block_expr(),
            _ => self.unexpected_token_error("expected expression"),
        }
    }

    pub(crate) fn parse_name_expr(&mut self) -> BuildNode {
        let token = self.bump().expect("identifier token should be present");
        self.finish_node(
            SyntaxKind::ExprName,
            vec![token_element(token, self.source)],
            token.range().start(),
        )
    }

    pub(crate) fn parse_switch_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`switch` token should be present")];
        children.push(node_element(self.parse_expr(0)));

        children.push(self.expect_token(
            TokenKind::OpenBrace,
            "expected `{` after `switch` expression",
            "`{` token should be present",
        ));
        if matches!(children.last(), Some(NodeOrToken::Node(node)) if node.kind() == SyntaxKind::Error)
        {
            return self.finish_node(SyntaxKind::ExprSwitch, children, start);
        }

        let arms_start = self.current_offset();
        let mut arm_children = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::CloseBrace) {
            arm_children.push(node_element(self.parse_switch_arm()));

            if self.at(TokenKind::Comma) {
                arm_children.push(self.bump_element("`,` token should be present"));
            } else if !self.at(TokenKind::CloseBrace) {
                arm_children.push(self.missing_error("expected `,` between `switch` arms"));
                self.recover_to_any(&[TokenKind::Comma, TokenKind::CloseBrace]);
                if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                    arm_children.push(comma);
                }
            }
        }
        let arms_end = self.next_significant_offset();
        children.push(node_element(self.finish_node_with_range(
            SyntaxKind::SwitchArmList,
            TextRange::new(arms_start, arms_end),
            arm_children,
        )));

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close `switch` expression",
            "`}` token should be present",
        ));

        self.finish_node(SyntaxKind::ExprSwitch, children, start)
    }

    pub(crate) fn parse_if_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`if` token should be present")];
        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `if` condition"),
        ));

        if self.at(TokenKind::ElseKw) {
            children.push(node_element(self.parse_else_branch()));
        }

        self.finish_node(SyntaxKind::ExprIf, children, start)
    }

    pub(crate) fn parse_else_branch(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`else` token should be present")];

        if self.at(TokenKind::IfKw) {
            children.push(node_element(self.parse_if_expr()));
        } else {
            children.push(node_element(
                self.parse_required_block("expected block or `if` after `else`"),
            ));
        }

        self.finish_node(SyntaxKind::ElseBranch, children, start)
    }

    pub(crate) fn parse_while_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`while` token should be present")];
        children.push(node_element(self.parse_expr(0)));
        children.push(node_element(
            self.parse_required_block("expected block after `while` condition"),
        ));

        self.finish_node(SyntaxKind::ExprWhile, children, start)
    }

    pub(crate) fn parse_loop_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`loop` token should be present")];
        children.push(node_element(
            self.parse_required_block("expected block after `loop`"),
        ));

        self.finish_node(SyntaxKind::ExprLoop, children, start)
    }

    pub(crate) fn parse_for_expr(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::ExprFor, children, start)
    }

    pub(crate) fn parse_do_expr(&mut self) -> BuildNode {
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
                self.source,
            ));
            condition_children.push(node_element(self.parse_expr(0)));
        } else {
            condition_children
                .push(self.missing_error("expected `while` or `until` after `do` block"));
        }

        children.push(node_element(self.finish_node(
            SyntaxKind::DoCondition,
            condition_children,
            condition_start,
        )));

        self.finish_node(SyntaxKind::ExprDo, children, start)
    }

    pub(crate) fn parse_closure_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![node_element(self.parse_closure_param_list())];

        if self.at(TokenKind::OpenBrace) {
            children.push(node_element(self.parse_block_expr()));
        } else if self.at_expr_start() {
            children.push(node_element(self.parse_expr(0)));
        } else {
            children.push(self.missing_error("expected closure body"));
        }

        self.finish_node(SyntaxKind::ExprClosure, children, start)
    }

    pub(crate) fn parse_backtick_string_expr(&mut self) -> BuildNode {
        let token = self
            .bump()
            .expect("back-tick string token should be present");

        if token.kind() == TokenKind::BacktickString {
            return self.finish_node(
                SyntaxKind::ExprLiteral,
                vec![token_element(token, self.source)],
                token.range().start(),
            );
        }

        let start = token.range().start();
        let mut children = vec![token_element(token, self.source)];
        let parts_start = self.current_offset();
        let mut part_children = Vec::new();

        while !self.is_eof() && !self.at(TokenKind::Backtick) {
            match self.peek_kind() {
                Some(TokenKind::StringText) => {
                    let text = self.bump().expect("string text token should be present");
                    part_children.push(node_element(self.finish_node(
                        SyntaxKind::StringSegment,
                        vec![token_element(text, self.source)],
                        text.range().start(),
                    )));
                }
                Some(TokenKind::InterpolationStart) => {
                    let interpolation_start = self
                        .bump()
                        .expect("interpolation start token should be present");
                    let mut interpolation_children =
                        vec![token_element(interpolation_start, self.source)];
                    let body_start = self.current_offset();
                    let item_list = self.parse_statement_list_with_range(
                        SyntaxKind::InterpolationItemList,
                        body_start,
                        Some(TokenKind::CloseBrace),
                        false,
                    );
                    interpolation_children.push(node_element(self.finish_node(
                        SyntaxKind::InterpolationBody,
                        vec![node_element(item_list)],
                        body_start,
                    )));
                    interpolation_children.push(self.expect_token(
                        TokenKind::CloseBrace,
                        "expected `}` to close string interpolation",
                        "`}` token should be present",
                    ));
                    part_children.push(node_element(self.finish_node(
                        SyntaxKind::StringInterpolation,
                        interpolation_children,
                        interpolation_start.range().start(),
                    )));
                }
                _ => {
                    part_children.push(self.missing_error(
                        "expected string text, interpolation, or closing back-tick",
                    ));
                    self.recover_to_any(&[TokenKind::Backtick, TokenKind::InterpolationStart]);
                }
            }
        }

        children.push(node_element(self.finish_node(
            SyntaxKind::StringPartList,
            part_children,
            parts_start,
        )));

        children.push(self.expect_token(
            TokenKind::Backtick,
            "expected closing back-tick",
            "closing back-tick token should be present",
        ));

        self.finish_node(SyntaxKind::ExprInterpolatedString, children, start)
    }

    pub(crate) fn parse_for_bindings(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::ForBindings, children, start)
    }

    pub(crate) fn parse_switch_arm(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![node_element(self.parse_switch_pattern_list())];

        children.push(self.expect_token(
            TokenKind::FatArrow,
            "expected `=>` in `switch` arm",
            "`=>` token should be present",
        ));

        children.push(node_element(self.parse_switch_arm_body()));

        self.finish_node(SyntaxKind::SwitchArm, children, start)
    }

    pub(crate) fn parse_switch_pattern_list(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::SwitchPatternList, children, start)
    }

    pub(crate) fn parse_switch_arm_body(&mut self) -> BuildNode {
        if self.at(TokenKind::OpenBrace) {
            self.parse_block_expr()
        } else {
            self.parse_expr(0)
        }
    }

    pub(crate) fn parse_call_expr(&mut self, callee: BuildNode, caller_scope: bool) -> BuildNode {
        let start = callee.range().start();
        let mut children = vec![node_element(callee)];
        if caller_scope {
            self.validate_caller_scope_callee(&children[0]);
            children.push(self.bump_element("`!` token should be present"));
        }
        let args_start = self.current_offset();
        let mut arg_children = vec![self.bump_element("`(` token should be present")];
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

        arg_children.push(self.expect_token(
            TokenKind::CloseParen,
            "expected `)` to close argument list",
            "`)` token should be present",
        ));

        children.push(node_element(self.finish_node(
            SyntaxKind::ArgList,
            arg_children,
            args_start,
        )));

        self.finish_node(SyntaxKind::ExprCall, children, start)
    }

    pub(crate) fn validate_caller_scope_callee(&mut self, callee: &BuildElement) {
        let NodeOrToken::Node(callee) = callee else {
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

    pub(crate) fn parse_path_expr(&mut self, base: BuildNode) -> BuildNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];

        while self.at(TokenKind::ColonColon) {
            children.push(self.bump_element("`::` token should be present"));
            children.push(self.expect_name_like(
                "expected path segment after `::`",
                "path segment token should be present",
            ));
            if matches!(children.last(), Some(NodeOrToken::Node(node)) if node.kind() == SyntaxKind::Error)
            {
                break;
            }
        }

        self.finish_node(SyntaxKind::ExprPath, children, start)
    }

    pub(crate) fn parse_index_expr(&mut self, base: BuildNode) -> BuildNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];
        children.push(token_element(
            self.bump().expect("index opener token should be present"),
            self.source,
        ));

        if self.at(TokenKind::CloseBracket) {
            children.push(self.missing_error("expected index expression"));
        } else {
            children.push(node_element(self.parse_expr(0)));
        }

        if self.at(TokenKind::CloseBracket) {
            children.push(token_element(
                self.bump().expect("`]` token should be present"),
                self.source,
            ));
        } else {
            children.push(self.missing_error("expected `]` to close index expression"));
        }

        self.finish_node(SyntaxKind::ExprIndex, children, start)
    }

    pub(crate) fn parse_field_expr(&mut self, base: BuildNode) -> BuildNode {
        let start = base.range().start();
        let mut children = vec![node_element(base)];
        children.push(token_element(
            self.bump().expect("field access token should be present"),
            self.source,
        ));

        if self.at_name_like() {
            children.push(token_element(
                self.bump()
                    .expect("field identifier token should be present"),
                self.source,
            ));
        } else {
            children.push(self.missing_error("expected property name after field access"));
        }

        self.finish_node(SyntaxKind::ExprField, children, start)
    }

    pub(crate) fn parse_array_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = Vec::new();
        let items_start = self.current_offset();
        let mut item_children = vec![self.bump_element("`[` token should be present")];
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

        item_children.push(self.expect_token(
            TokenKind::CloseBracket,
            "expected `]` to close array literal",
            "`]` token should be present",
        ));

        children.push(node_element(self.finish_node(
            SyntaxKind::ArrayItemList,
            item_children,
            items_start,
        )));

        self.finish_node(SyntaxKind::ExprArray, children, start)
    }

    pub(crate) fn parse_object_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`#{` token should be present")];

        let fields_start = self.current_offset();
        let mut field_children = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::CloseBrace) {
            field_children.push(node_element(self.parse_object_field()));

            if let Some(comma) = self.eat(TokenKind::Comma, "`,` token should be present") {
                field_children.push(comma);
            } else if self.at_object_field_start() {
                field_children.push(self.missing_error("expected `,` between object fields"));
            } else {
                break;
            }
        }
        let fields_end = self.next_significant_offset();
        children.push(node_element(self.finish_node_with_range(
            SyntaxKind::ObjectFieldList,
            TextRange::new(fields_start, fields_end),
            field_children,
        )));

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close object map literal",
            "`}` token should be present",
        ));

        self.finish_node(SyntaxKind::ExprObject, children, start)
    }

    pub(crate) fn parse_object_field(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::ObjectField, children, start)
    }

    pub(crate) fn parse_paren_expr(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::ExprParen, children, start)
    }

    pub(crate) fn parse_block_expr(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`{` token should be present")];

        let items_start = self.current_offset();
        children.push(node_element(self.parse_statement_list_with_range(
            SyntaxKind::BlockItemList,
            items_start,
            Some(TokenKind::CloseBrace),
            true,
        )));

        children.push(self.expect_token(
            TokenKind::CloseBrace,
            "expected `}` to close block",
            "`}` token should be present",
        ));

        self.finish_node(SyntaxKind::Block, children, start)
    }
}
