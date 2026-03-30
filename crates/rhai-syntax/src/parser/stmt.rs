use crate::parser::Parser;
use crate::syntax::{SyntaxKind, SyntaxNode, TokenKind, empty_range, node_element};

impl<'a> Parser<'a> {
    fn finish_statement(
        &mut self,
        children: &mut Vec<crate::SyntaxElement>,
        can_omit_semicolon_before_next: bool,
    ) {
        if let Some(semicolon) = self.eat(TokenKind::Semicolon, "`;` token should be present") {
            children.push(semicolon);
            return;
        }

        if !self.is_eof() && !self.at(TokenKind::CloseBrace) && !can_omit_semicolon_before_next {
            children.push(self.missing_error("expected `;` to terminate statement"));
        }
    }

    fn expr_stmt_can_omit_semicolon_before_next(expr: &SyntaxNode) -> bool {
        matches!(
            expr.kind(),
            SyntaxKind::ExprIf
                | SyntaxKind::ExprWhile
                | SyntaxKind::ExprLoop
                | SyntaxKind::ExprFor
                | SyntaxKind::ExprSwitch
                | SyntaxKind::Block
        )
    }

    pub(crate) fn parse_stmt(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_let_stmt(&mut self) -> SyntaxNode {
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

        self.finish_statement(&mut children, false);

        SyntaxNode::new(SyntaxKind::StmtLet, children, start)
    }

    pub(crate) fn parse_const_stmt(&mut self) -> SyntaxNode {
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

        self.finish_statement(&mut children, false);

        SyntaxNode::new(SyntaxKind::StmtConst, children, start)
    }

    pub(crate) fn parse_import_stmt(&mut self) -> SyntaxNode {
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

        self.finish_statement(&mut children, false);

        SyntaxNode::new(SyntaxKind::StmtImport, children, start)
    }

    pub(crate) fn parse_export_stmt(&mut self) -> SyntaxNode {
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

        if !parsed_declaration {
            self.finish_statement(&mut children, false);
        }

        SyntaxNode::new(SyntaxKind::StmtExport, children, start)
    }

    pub(crate) fn parse_value_stmt(&mut self, kind: SyntaxKind) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("statement keyword token should be present")];

        if !self.is_stmt_terminator() {
            children.push(node_element(self.parse_expr(0)));
        }

        self.finish_statement(&mut children, false);

        SyntaxNode::new(kind, children, start)
    }

    pub(crate) fn parse_continue_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let mut children = vec![self.bump_element("`continue` token should be present")];

        self.finish_statement(&mut children, false);

        SyntaxNode::new(SyntaxKind::StmtContinue, children, start)
    }

    pub(crate) fn parse_try_stmt(&mut self) -> SyntaxNode {
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

    pub(crate) fn parse_expr_stmt(&mut self) -> SyntaxNode {
        let start = self.current_offset();
        let expr = self.parse_expr(0);
        let can_omit_semicolon_before_next = Self::expr_stmt_can_omit_semicolon_before_next(&expr);
        let mut children = vec![node_element(expr)];

        self.finish_statement(&mut children, can_omit_semicolon_before_next);

        SyntaxNode::new(SyntaxKind::StmtExpr, children, start)
    }
}
