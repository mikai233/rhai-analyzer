use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, stmt_support};
use crate::formatter::trivia::comments::GapSeparatorOptions;

impl Formatter<'_> {
    pub(crate) fn format_stmt(&self, stmt: Stmt, indent: usize) -> Doc {
        if self.is_skipped(stmt.syntax()) {
            return Doc::text(self.raw(stmt.syntax()));
        }

        if matches!(stmt_support(&stmt).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(stmt.syntax()));
        }

        if matches!(stmt_support(&stmt).level, FormatSupportLevel::Structural)
            && self.stmt_requires_raw_fallback(stmt.clone())
        {
            return Doc::text(self.raw(stmt.syntax()));
        }

        match stmt {
            Stmt::Let(let_stmt) => self.format_let_stmt(let_stmt, indent),
            Stmt::Const(const_stmt) => self.format_const_stmt(const_stmt, indent),
            Stmt::Import(import_stmt) => self.format_import_stmt(import_stmt, indent),
            Stmt::Export(export_stmt) => self.format_export_stmt(export_stmt, indent),
            Stmt::Break(break_stmt) => {
                let head = Doc::text("break");
                let mut parts = vec![head.clone()];
                let value = break_stmt.value();
                if let Some(value) = value.clone() {
                    let Some(keyword_token) = self.token(break_stmt.syntax(), TokenKind::BreakKw)
                    else {
                        parts[0] = self.format_statement_tail_doc(
                            head,
                            "",
                            self.format_expr_doc(value.clone(), indent),
                            indent,
                        );
                        parts.push(self.statement_semicolon_doc(Some(value), &break_stmt.syntax()));
                        return Doc::concat(parts);
                    };
                    parts[0] = self.format_value_statement_doc(
                        &break_stmt.syntax(),
                        head,
                        self.format_expr_doc(value.clone(), indent),
                        keyword_token,
                        value.clone(),
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value, &break_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Continue(continue_stmt) => {
                let keyword_token = self.token(continue_stmt.syntax(), TokenKind::ContinueKw);
                Doc::concat(vec![
                    Doc::text("continue"),
                    keyword_token
                        .map(|keyword_token| {
                            self.keyword_semicolon_doc(keyword_token, &continue_stmt.syntax())
                        })
                        .unwrap_or_else(Doc::nil),
                ])
            }
            Stmt::Return(return_stmt) => {
                let head = Doc::text("return");
                let mut parts = vec![head.clone()];
                let value = return_stmt.value();
                if let Some(value) = value.clone() {
                    let Some(keyword_token) = self.token(return_stmt.syntax(), TokenKind::ReturnKw)
                    else {
                        parts[0] = self.format_statement_tail_doc(
                            head,
                            "",
                            self.format_expr_doc(value.clone(), indent),
                            indent,
                        );
                        parts
                            .push(self.statement_semicolon_doc(Some(value), &return_stmt.syntax()));
                        return Doc::concat(parts);
                    };
                    parts[0] = self.format_value_statement_doc(
                        &return_stmt.syntax(),
                        head,
                        self.format_expr_doc(value.clone(), indent),
                        keyword_token,
                        value.clone(),
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value, &return_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Throw(throw_stmt) => {
                let head = Doc::text("throw");
                let mut parts = vec![head.clone()];
                let value = throw_stmt.value();
                if let Some(value) = value.clone() {
                    let Some(keyword_token) = self.token(throw_stmt.syntax(), TokenKind::ThrowKw)
                    else {
                        parts[0] = self.format_statement_tail_doc(
                            head,
                            "",
                            self.format_expr_doc(value.clone(), indent),
                            indent,
                        );
                        parts.push(self.statement_semicolon_doc(Some(value), &throw_stmt.syntax()));
                        return Doc::concat(parts);
                    };
                    parts[0] = self.format_value_statement_doc(
                        &throw_stmt.syntax(),
                        head,
                        self.format_expr_doc(value.clone(), indent),
                        keyword_token,
                        value.clone(),
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value, &throw_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Try(try_stmt) => {
                let try_kw = self.token(try_stmt.syntax(), TokenKind::TryKw);
                let body_expr = try_stmt.body();
                let body = body_expr
                    .clone()
                    .map(|body| self.format_block_doc(body, indent))
                    .unwrap_or_else(|| Doc::text("{}"));
                let try_kw_end = self
                    .token_range(try_stmt.syntax(), TokenKind::TryKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(try_stmt.syntax().text_range().start()) as usize);
                let body_start = body_expr
                    .as_ref()
                    .map(|body| u32::from(body.syntax().text_range().start()) as usize)
                    .unwrap_or_else(|| u32::from(try_stmt.syntax().text_range().end()) as usize);
                let body_end = body_expr
                    .as_ref()
                    .map(|body| u32::from(body.syntax().text_range().end()) as usize)
                    .unwrap_or(body_start);
                let body_separator = match (try_kw.clone(), body_expr.clone()) {
                    (Some(try_kw), Some(body_expr)) => self.head_body_separator_for_boundary(
                        try_stmt.syntax(),
                        TriviaBoundary::TokenNode(try_kw, body_expr.syntax()),
                    ),
                    _ => self.head_body_separator_doc(try_kw_end, body_start),
                };
                let mut parts = vec![Doc::text("try"), body_separator, body];
                if let Some(catch_clause) = try_stmt.catch_clause() {
                    let catch_start = self
                        .token_range(catch_clause.syntax(), TokenKind::CatchKw)
                        .map(range_start)
                        .unwrap_or_else(|| {
                            u32::from(catch_clause.syntax().text_range().start()) as usize
                        });
                    parts.push(match body_expr.clone() {
                        Some(body_expr) => self.inline_or_boundary_separator_doc(
                            try_stmt.syntax(),
                            TriviaBoundary::NodeNode(body_expr.syntax(), catch_clause.syntax()),
                            GapSeparatorOptions {
                                inline_text: " ",
                                minimum_newlines: 1,
                                has_previous: true,
                                has_next: true,
                                include_terminal_newline: true,
                            },
                        ),
                        None => self.inline_or_gap_separator_doc(
                            body_end,
                            catch_start,
                            GapSeparatorOptions {
                                inline_text: " ",
                                minimum_newlines: 1,
                                has_previous: true,
                                has_next: true,
                                include_terminal_newline: true,
                            },
                        ),
                    });
                    parts.push(self.format_catch_clause_doc(catch_clause, indent));
                }
                Doc::concat(parts)
            }
            Stmt::Expr(expr_stmt) => self.format_expr_stmt(expr_stmt, indent),
        }
    }

    pub(crate) fn format_let_stmt(&self, let_stmt: LetStmt, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("let ")];
        if let Some(name) = let_stmt.name_token() {
            head_parts.push(Doc::text(name.text()));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        let initializer = let_stmt.initializer();
        if let Some(initializer) = initializer {
            parts[0] = self.format_assignment_statement_doc(
                &let_stmt.syntax(),
                head,
                self.format_expr_doc(initializer, indent),
                let_stmt.name_token(),
                let_stmt.initializer(),
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(let_stmt.initializer(), &let_stmt.syntax()));
        Doc::concat(parts)
    }

    pub(crate) fn format_const_stmt(&self, const_stmt: ConstStmt, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("const ")];
        if let Some(name) = const_stmt.name_token() {
            head_parts.push(Doc::text(name.text()));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        let value = const_stmt.value();
        if let Some(value) = value {
            parts[0] = self.format_assignment_statement_doc(
                &const_stmt.syntax(),
                head,
                self.format_expr_doc(value, indent),
                const_stmt.name_token(),
                const_stmt.value(),
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(const_stmt.value(), &const_stmt.syntax()));
        Doc::concat(parts)
    }

    pub(crate) fn format_import_stmt(&self, import_stmt: ImportStmt, indent: usize) -> Doc {
        if self.import_stmt_requires_raw_fallback(&import_stmt) {
            return Doc::text(self.raw(import_stmt.syntax()));
        }

        let module = import_stmt.module();
        let Some(module) = module else {
            return Doc::text("import;");
        };

        let mut tail_parts = vec![self.format_expr_doc(module.clone(), indent)];
        if let Some(alias) = import_stmt.alias() {
            let gap = self
                .boundary_trivia(
                    import_stmt.syntax(),
                    TriviaBoundary::NodeNode(module.syntax(), alias.syntax()),
                )
                .unwrap_or_default();
            tail_parts.push(self.space_or_tight_gap_from_gap(&gap));
            tail_parts.push(self.format_alias_clause_doc(alias));
        }
        let tail = Doc::concat(tail_parts);
        let Some(import_kw) = import_stmt
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::ImportKw))
        else {
            return Doc::concat(vec![
                Doc::text("import"),
                Doc::text(" "),
                tail,
                Doc::text(";"),
            ]);
        };

        let head_gap = self
            .boundary_trivia(
                import_stmt.syntax(),
                TriviaBoundary::TokenNode(import_kw.clone(), module.syntax()),
            )
            .unwrap_or_default();

        if !head_gap.has_comments() && self.statement_tail_renders_single_line(&tail, indent) {
            return Doc::group(Doc::concat(vec![
                Doc::text("import"),
                Doc::indent(1, Doc::concat(vec![Doc::soft_line(), tail])),
                Doc::text(";"),
            ]));
        }

        Doc::concat(vec![
            Doc::text("import"),
            self.space_or_tight_gap_from_gap(&head_gap),
            tail,
            Doc::text(";"),
        ])
    }

    pub(crate) fn format_export_stmt(&self, export_stmt: rhai_syntax::ExportStmt, indent: usize) -> Doc {
        if self.export_stmt_requires_raw_fallback(&export_stmt) {
            return Doc::text(self.raw(export_stmt.syntax()));
        }

        if let Some(declaration) = export_stmt.declaration() {
            let declaration_doc = self.format_stmt(declaration.clone(), indent);
            let Some(export_kw) = export_stmt
                .syntax()
                .direct_significant_tokens()
                .find(|token| token.kind().token_kind() == Some(TokenKind::ExportKw))
            else {
                return Doc::concat(vec![Doc::text("export "), declaration_doc]);
            };

            let head_gap = self
                .boundary_trivia(
                    export_stmt.syntax(),
                    TriviaBoundary::TokenNode(export_kw.clone(), declaration.syntax()),
                )
                .unwrap_or_default();

            if !head_gap.has_comments()
                && self.statement_tail_renders_single_line(&declaration_doc, indent)
            {
                return Doc::group(Doc::concat(vec![
                    Doc::text("export"),
                    Doc::indent(1, Doc::concat(vec![Doc::soft_line(), declaration_doc])),
                ]));
            }

            return Doc::concat(vec![
                Doc::text("export"),
                self.space_or_tight_gap_from_gap(&head_gap),
                declaration_doc,
            ]);
        } else if let Some(target) = export_stmt.target() {
            let mut tail_parts = vec![self.format_expr_doc(target.clone(), indent)];
            if let Some(alias) = export_stmt.alias() {
                let gap = self
                    .boundary_trivia(
                        export_stmt.syntax(),
                        TriviaBoundary::NodeNode(target.syntax(), alias.syntax()),
                    )
                    .unwrap_or_default();
                tail_parts.push(self.space_or_tight_gap_from_gap(&gap));
                tail_parts.push(self.format_alias_clause_doc(alias));
            }
            let tail = Doc::concat(tail_parts);
            let Some(export_kw) = export_stmt
                .syntax()
                .direct_significant_tokens()
                .find(|token| token.kind().token_kind() == Some(TokenKind::ExportKw))
            else {
                return Doc::concat(vec![Doc::text("export "), tail, Doc::text(";")]);
            };

            let head_gap = self
                .boundary_trivia(
                    export_stmt.syntax(),
                    TriviaBoundary::TokenNode(export_kw.clone(), target.syntax()),
                )
                .unwrap_or_default();

            if !head_gap.has_comments() && self.statement_tail_renders_single_line(&tail, indent) {
                return Doc::group(Doc::concat(vec![
                    Doc::text("export"),
                    Doc::indent(1, Doc::concat(vec![Doc::soft_line(), tail])),
                    Doc::text(";"),
                ]));
            }

            return Doc::concat(vec![
                Doc::text("export"),
                self.space_or_tight_gap_from_gap(&head_gap),
                tail,
                Doc::text(";"),
            ]);
        }

        Doc::text("export;")
    }

    pub(crate) fn format_expr_stmt(&self, expr_stmt: ExprStmt, indent: usize) -> Doc {
        let mut parts = Vec::new();
        if let Some(expr) = expr_stmt.expr() {
            parts.push(self.format_expr_doc(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            parts.push(self.statement_semicolon_doc(expr_stmt.expr(), &expr_stmt.syntax()));
        }
        Doc::concat(parts)
    }

    pub(crate) fn format_block_doc(&self, block: BlockExpr, indent: usize) -> Doc {
        let items = block
            .item_list()
            .map(|items| items.items().collect::<Vec<_>>())
            .unwrap_or_default();
        let open_brace = self.token(block.syntax(), TokenKind::OpenBrace);
        let close_brace = self.token(block.syntax(), TokenKind::CloseBrace);
        let open_brace_end = open_brace
            .as_ref()
            .map(|range| u32::from(range.text_range().end()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().text_range().start()) as usize);
        let close_brace_start = close_brace
            .as_ref()
            .map(|range| u32::from(range.text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().text_range().end()) as usize);
        let item_elements = items
            .iter()
            .map(|item| item.syntax().into())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(open_brace_end, close_brace_start, &item_elements);
        let leading_gap = owned.leading.clone();
        if items.is_empty() {
            if !leading_gap.has_comments() {
                return Doc::text("{}");
            }

            return Doc::concat(vec![
                Doc::text("{"),
                Doc::indent(
                    1,
                    Doc::concat(vec![
                        Doc::hard_line(),
                        self.format_empty_sequence_body_doc(&leading_gap),
                    ]),
                ),
                Doc::hard_line(),
                Doc::text("}"),
            ]);
        }

        let mut body_parts = self.format_sequence_body_doc(
            items
                .iter()
                .map(|item| self.format_item(item.clone(), indent + 1))
                .collect(),
            &owned,
            |_| 1,
        );
        self.append_sequence_trailing_doc(&mut body_parts, &owned.trailing, !items.is_empty(), 1);

        Doc::concat(vec![
            Doc::text("{"),
            Doc::indent(
                1,
                Doc::concat(vec![Doc::hard_line(), Doc::concat(body_parts)]),
            ),
            Doc::hard_line(),
            Doc::text("}"),
        ])
    }

    pub(crate) fn format_block_item_list_body_doc(
        &self,
        item_list: BlockItemList,
        indent: usize,
    ) -> Doc {
        let items = item_list.items().collect::<Vec<_>>();
        let item_elements = items
            .iter()
            .map(|item| item.syntax().into())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(
            u32::from(item_list.syntax().text_range().start()) as usize,
            u32::from(item_list.syntax().text_range().end()) as usize,
            &item_elements,
        );
        let body_parts = self.format_sequence_body_doc(
            items
                .iter()
                .map(|item| self.format_item(item.clone(), indent))
                .collect(),
            &owned,
            |_| 1,
        );

        Doc::concat(body_parts)
    }

    pub(crate) fn format_statement_tail_doc(
        &self,
        head: Doc,
        suffix_head: &str,
        tail: Doc,
        indent: usize,
    ) -> Doc {
        if self.statement_tail_renders_single_line(&tail, indent) {
            return Doc::group(Doc::concat(vec![
                head,
                Doc::indent(
                    1,
                    Doc::concat(vec![Doc::soft_line(), Doc::text(suffix_head), tail]),
                ),
            ]));
        }

        if suffix_head.is_empty() {
            Doc::concat(vec![head, Doc::text(" "), tail])
        } else {
            Doc::concat(vec![head, Doc::text(format!(" {suffix_head}")), tail])
        }
    }

    pub(crate) fn try_stmt_requires_raw_fallback(&self, try_stmt: rhai_syntax::TryStmt) -> bool {
        let Some(body) = try_stmt.body() else {
            return self.node_has_unowned_comments(try_stmt.syntax());
        };
        let Some(try_kw) = self.token(try_stmt.syntax(), TokenKind::TryKw) else {
            return self.node_has_unowned_comments(try_stmt.syntax());
        };

        let mut allowed_boundaries = vec![TriviaBoundary::TokenNode(try_kw, body.syntax())];
        if let Some(catch_clause) = try_stmt.catch_clause() {
            allowed_boundaries.push(TriviaBoundary::NodeNode(
                body.syntax(),
                catch_clause.syntax(),
            ));

            if self.catch_clause_requires_raw_fallback(catch_clause) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside_boundaries(try_stmt.syntax(), &allowed_boundaries)
    }

    pub(crate) fn catch_clause_requires_raw_fallback(&self, catch_clause: rhai_syntax::CatchClause) -> bool {
        let Some(body) = catch_clause.body() else {
            return self.node_has_unowned_comments(catch_clause.syntax());
        };
        let Some(catch_head_token) = self
            .token(catch_clause.syntax(), TokenKind::CloseParen)
            .or_else(|| self.token(catch_clause.syntax(), TokenKind::CatchKw))
        else {
            return self.node_has_unowned_comments(catch_clause.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            catch_clause.syntax(),
            &[TriviaBoundary::TokenNode(catch_head_token, body.syntax())],
        )
    }

    pub(crate) fn import_stmt_requires_raw_fallback(&self, import_stmt: &ImportStmt) -> bool {
        let mut allowed_boundaries = Vec::new();
        if let (Some(module), Some(alias)) = (import_stmt.module(), import_stmt.alias()) {
            allowed_boundaries.push(TriviaBoundary::NodeNode(module.syntax(), alias.syntax()));
            if self.alias_clause_requires_raw_fallback(alias) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside_boundaries(import_stmt.syntax(), &allowed_boundaries)
    }

    pub(crate) fn export_stmt_requires_raw_fallback(&self, export_stmt: &rhai_syntax::ExportStmt) -> bool {
        let mut allowed_boundaries = Vec::new();
        if let Some(alias) = export_stmt.alias() {
            if self.alias_clause_requires_raw_fallback(alias.clone()) {
                return true;
            }

            if let Some(target) = export_stmt.target() {
                allowed_boundaries.push(TriviaBoundary::NodeNode(target.syntax(), alias.syntax()));
            }
        }

        self.node_has_unowned_comments_outside_boundaries(export_stmt.syntax(), &allowed_boundaries)
    }

    pub(crate) fn alias_clause_requires_raw_fallback(&self, alias: rhai_syntax::AliasClause) -> bool {
        let Some(as_token) = self.token(alias.syntax(), TokenKind::AsKw) else {
            return self.node_has_unowned_comments(alias.syntax());
        };
        let Some(alias_name) = alias.alias_token() else {
            return self.node_has_unowned_comments(alias.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            alias.syntax(),
            &[TriviaBoundary::TokenToken(as_token, alias_name)],
        )
    }

    pub(crate) fn let_stmt_requires_raw_fallback(&self, let_stmt: LetStmt) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            &let_stmt.syntax(),
            let_stmt.name_token(),
            let_stmt.initializer(),
            let_stmt.initializer().is_some(),
        )
    }

    pub(crate) fn const_stmt_requires_raw_fallback(&self, const_stmt: ConstStmt) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            &const_stmt.syntax(),
            const_stmt.name_token(),
            const_stmt.value(),
            const_stmt.value().is_some(),
        )
    }

    pub(crate) fn assignment_stmt_requires_raw_fallback(
        &self,
        stmt: &SyntaxNode,
        head_token: Option<SyntaxToken>,
        value_expr: Option<Expr>,
        has_value: bool,
    ) -> bool {
        let Some(eq_token) = self.token(stmt.clone(), TokenKind::Eq) else {
            return self.node_has_unowned_comments(stmt.clone());
        };

        let Some(head_token) = head_token else {
            return self.node_has_unowned_comments(stmt.clone());
        };

        let mut allowed_boundaries = vec![TriviaBoundary::TokenToken(head_token, eq_token.clone())];
        if let Some(value_expr) = value_expr {
            allowed_boundaries.push(TriviaBoundary::TokenNode(
                eq_token.clone(),
                value_expr.syntax(),
            ));
            if let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) {
                allowed_boundaries.push(TriviaBoundary::NodeToken(
                    value_expr.syntax(),
                    semicolon_token,
                ));
            }
        } else if has_value {
            return self.node_has_unowned_comments(stmt.clone());
        }

        self.node_has_unowned_comments_outside_boundaries(stmt.clone(), &allowed_boundaries)
    }

    pub(crate) fn value_stmt_requires_raw_fallback(
        &self,
        stmt: &SyntaxNode,
        keyword_token: Option<SyntaxToken>,
        value_expr: Option<Expr>,
        has_value: bool,
    ) -> bool {
        let mut allowed_boundaries = Vec::new();
        if let (Some(keyword_token), Some(value_expr)) = (keyword_token, value_expr) {
            allowed_boundaries.push(TriviaBoundary::TokenNode(
                keyword_token,
                value_expr.syntax(),
            ));
            if let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) {
                allowed_boundaries.push(TriviaBoundary::NodeToken(
                    value_expr.syntax(),
                    semicolon_token,
                ));
            }
        } else if has_value {
            return self.node_has_unowned_comments(stmt.clone());
        }

        self.node_has_unowned_comments_outside_boundaries(stmt.clone(), &allowed_boundaries)
    }

    pub(crate) fn expr_stmt_requires_raw_fallback(&self, expr_stmt: ExprStmt) -> bool {
        let Some(expr) = expr_stmt.expr() else {
            return self.node_has_unowned_comments(expr_stmt.syntax());
        };

        let mut allowed_boundaries = Vec::new();
        if let Some(semicolon_token) = self.token(expr_stmt.syntax(), TokenKind::Semicolon) {
            allowed_boundaries.push(TriviaBoundary::NodeToken(expr.syntax(), semicolon_token));
        }

        self.node_has_unowned_comments_outside_boundaries(expr_stmt.syntax(), &allowed_boundaries)
    }

    pub(crate) fn continue_stmt_requires_raw_fallback(&self, stmt: &SyntaxNode) -> bool {
        let Some(keyword_token) = self.token(stmt.clone(), TokenKind::ContinueKw) else {
            return self.node_has_unowned_comments(stmt.clone());
        };
        let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) else {
            return self.node_has_unowned_comments(stmt.clone());
        };

        self.node_has_unowned_comments_outside_boundaries(
            stmt.clone(),
            &[TriviaBoundary::TokenToken(keyword_token, semicolon_token)],
        )
    }

    pub(crate) fn format_assignment_statement_doc(
        &self,
        stmt: &SyntaxNode,
        head: Doc,
        value: Doc,
        head_token: Option<SyntaxToken>,
        value_expr: Option<Expr>,
        indent: usize,
    ) -> Doc {
        let Some(head_token) = head_token else {
            return self.format_statement_tail_doc(head, "= ", value, indent);
        };
        let Some(value_expr) = value_expr else {
            return self.format_statement_tail_doc(head, "= ", value, indent);
        };
        let Some(eq_token) = self.token(stmt.clone(), TokenKind::Eq) else {
            return self.format_statement_tail_doc(head, "= ", value, indent);
        };
        let before_eq_gap = self
            .boundary_trivia(
                stmt.clone(),
                TriviaBoundary::TokenToken(head_token, eq_token.clone()),
            )
            .unwrap_or_default();
        let after_eq_gap = self
            .boundary_trivia(
                stmt.clone(),
                TriviaBoundary::TokenNode(eq_token.clone(), value_expr.syntax()),
            )
            .unwrap_or_default();

        if before_eq_gap.has_comments() || after_eq_gap.has_comments() {
            return Doc::concat(vec![
                head,
                self.space_or_tight_gap_from_gap(&before_eq_gap),
                Doc::text("="),
                self.space_or_tight_gap_from_gap(&after_eq_gap),
                value,
            ]);
        }

        self.format_statement_tail_doc(head, "= ", value, indent)
    }

    pub(crate) fn format_value_statement_doc(
        &self,
        stmt: &SyntaxNode,
        head: Doc,
        value: Doc,
        keyword_token: SyntaxToken,
        value_expr: Expr,
        indent: usize,
    ) -> Doc {
        let gap = self
            .boundary_trivia(
                stmt.clone(),
                TriviaBoundary::TokenNode(keyword_token, value_expr.syntax()),
            )
            .unwrap_or_default();
        if gap.has_comments() {
            return Doc::concat(vec![head, self.space_or_tight_gap_from_gap(&gap), value]);
        }

        self.format_statement_tail_doc(head, "", value, indent)
    }

    pub(crate) fn statement_semicolon_doc(&self, value_expr: Option<Expr>, stmt: &SyntaxNode) -> Doc {
        let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) else {
            return Doc::nil();
        };
        let Some(value_expr) = value_expr else {
            return Doc::text(";");
        };

        Doc::concat(vec![
            self.tight_comment_gap_for_boundary(
                stmt.clone(),
                TriviaBoundary::NodeToken(value_expr.syntax(), semicolon_token),
                false,
            ),
            Doc::text(";"),
        ])
    }

    pub(crate) fn keyword_semicolon_doc(&self, keyword_token: SyntaxToken, stmt: &SyntaxNode) -> Doc {
        let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) else {
            return Doc::nil();
        };

        Doc::concat(vec![
            self.tight_comment_gap_for_boundary(
                stmt.clone(),
                TriviaBoundary::TokenToken(keyword_token, semicolon_token),
                false,
            ),
            Doc::text(";"),
        ])
    }

    pub(crate) fn space_or_tight_statement_gap_doc(&self, gap: Option<GapTrivia>) -> Doc {
        gap.as_ref()
            .map(|gap| self.space_or_tight_gap_from_gap(gap))
            .unwrap_or_else(|| Doc::text(" "))
    }

    pub(crate) fn format_alias_clause_doc(&self, alias: rhai_syntax::AliasClause) -> Doc {
        let Some(alias_name) = alias.alias_token() else {
            return Doc::text(self.raw(alias.syntax()));
        };
        let Some(as_token) = alias
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::AsKw))
        else {
            return Doc::text(self.raw(alias.syntax()));
        };

        Doc::concat(vec![
            Doc::text("as"),
            self.space_or_tight_statement_gap_doc(self.boundary_trivia(
                alias.syntax(),
                TriviaBoundary::TokenToken(as_token, alias_name.clone()),
            )),
            Doc::text(alias_name.text()),
        ])
    }

    pub(crate) fn format_catch_clause_doc(
        &self,
        catch_clause: rhai_syntax::CatchClause,
        indent: usize,
    ) -> Doc {
        let mut catch_head = String::from("catch");
        if let Some(binding) = catch_clause.binding_token() {
            catch_head.push_str(" (");
            catch_head.push_str(binding.text());
            catch_head.push(')');
        }
        let catch_body = catch_clause
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let catch_kw = self.token(catch_clause.syntax(), TokenKind::CatchKw);
        let close_paren = self.token(catch_clause.syntax(), TokenKind::CloseParen);
        let catch_body_start = catch_clause
            .body()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(catch_clause.syntax().text_range().end()) as usize);
        let catch_head_end = self
            .token_range(catch_clause.syntax(), TokenKind::CloseParen)
            .map(range_end)
            .or_else(|| {
                self.token_range(catch_clause.syntax(), TokenKind::CatchKw)
                    .map(range_end)
            })
            .unwrap_or(catch_body_start);
        let body_separator = match catch_clause.body() {
            Some(body) => {
                let boundary = close_paren
                    .map(|close_paren| TriviaBoundary::TokenNode(close_paren, body.syntax()))
                    .or_else(|| {
                        catch_kw
                            .clone()
                            .map(|catch_kw| TriviaBoundary::TokenNode(catch_kw, body.syntax()))
                    });
                boundary.map_or_else(
                    || self.head_body_separator_doc(catch_head_end, catch_body_start),
                    |boundary| {
                        self.head_body_separator_for_boundary(catch_clause.syntax(), boundary)
                    },
                )
            }
            None => self.head_body_separator_doc(catch_head_end, catch_body_start),
        };

        Doc::concat(vec![Doc::text(catch_head), body_separator, catch_body])
    }
}

fn range_start(range: rhai_syntax::TextRange) -> usize {
    u32::from(range.start()) as usize
}

fn range_end(range: rhai_syntax::TextRange) -> usize {
    u32::from(range.end()) as usize
}
