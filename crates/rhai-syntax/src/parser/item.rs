use crate::parser::{BuildElement, BuildNode, Parser, node_element, token_element};
use crate::syntax::{SyntaxKind, TokenKind, empty_range};

impl<'a> Parser<'a> {
    pub(crate) fn parse_fn_item(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::ItemFn, children, start)
    }

    pub(crate) fn parse_fn_name_parts(&mut self) -> Vec<BuildElement> {
        let mut children = Vec::new();

        match self.peek_kind() {
            Some(TokenKind::Ident) => {
                children.push(token_element(
                    self.bump()
                        .expect("function name or typed receiver token should be present"),
                    self.source,
                ));

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
                    self.source,
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

    pub(crate) fn parse_catch_clause(&mut self) -> BuildNode {
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

        self.finish_node(SyntaxKind::CatchClause, children, start)
    }

    pub(crate) fn parse_alias_clause(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`as` token should be present")];
        children.push(
            self.expect_alias_name("expected alias after `as`", "alias token should be present"),
        );

        self.finish_node(SyntaxKind::AliasClause, children, start)
    }

    pub(crate) fn parse_param_list(&mut self) -> BuildNode {
        let start = self.current_offset();
        let mut children = Vec::new();

        if self.at(TokenKind::OpenParen) {
            children.push(self.bump_element("`(` token should be present"));
        } else {
            children.push(self.missing_error("expected `(` after function name"));
            return self.finish_node(SyntaxKind::ParamList, children, start);
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

        self.finish_node(SyntaxKind::ParamList, children, start)
    }

    pub(crate) fn parse_closure_param_list(&mut self) -> BuildNode {
        let start = self.current_offset();
        let open = self
            .bump()
            .expect("leading closure delimiter token should be present");
        let mut children = vec![token_element(open, self.source)];

        if open.kind() == TokenKind::PipePipe {
            return self.finish_node(SyntaxKind::ClosureParamList, children, start);
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

        self.finish_node(SyntaxKind::ClosureParamList, children, start)
    }
}
