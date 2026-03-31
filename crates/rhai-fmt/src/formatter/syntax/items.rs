use rhai_syntax::{
    AstNode, BlockExpr, BlockItemList, ConstStmt, Expr, ExprStmt, FnItem, ImportStmt, Item,
    LetStmt, Root, RootItemList, Stmt, SyntaxNode, SyntaxNodeExt, SyntaxToken, TokenKind,
    TriviaBoundary,
};

use crate::ImportSortOrder;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, item_support, stmt_support};
use crate::formatter::trivia::comments::GapSeparatorOptions;

impl Formatter<'_> {
    pub(crate) fn format_root(&self, root: Root) -> Doc {
        let items = root
            .item_list()
            .map(|items| items.items().collect::<Vec<_>>())
            .unwrap_or_default();
        let entries = self.root_entries(&items);
        let mut parts = Vec::new();
        let mut cursor = u32::from(root.syntax().text_range().start()) as usize;

        for (index, entry) in entries.iter().enumerate() {
            let gap = self.comment_gap(cursor, entry.start, index > 0, true);
            parts.push(self.gap_separator_doc(
                &gap,
                if index > 0 {
                    root_item_separator_min_newlines(entries[index - 1].kind, entry.kind)
                } else {
                    1
                },
                index > 0,
                true,
            ));
            parts.push(entry.doc.clone());
            cursor = entry.end;
        }

        let root_end = u32::from(root.syntax().text_range().end()) as usize;
        let trailing_gap = self.comment_gap(cursor, root_end, !items.is_empty(), false);
        if !items.is_empty() && trailing_gap.has_comments() {
            parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if trailing_gap.has_vertical_comments() {
            parts.push(self.render_line_comments_doc(trailing_gap.vertical_comments()));
        }

        if parts.is_empty() {
            Doc::nil()
        } else {
            parts.push(Doc::hard_line());
            Doc::concat(parts)
        }
    }

    pub(crate) fn format_root_item_list_body_doc(&self, item_list: RootItemList) -> Doc {
        let items = item_list.items().collect::<Vec<_>>();
        let entries = self.root_entries(&items);
        let mut parts = Vec::new();
        let mut cursor = u32::from(item_list.syntax().text_range().start()) as usize;

        for (index, entry) in entries.iter().enumerate() {
            let gap = self.comment_gap(cursor, entry.start, index > 0, true);
            parts.push(self.gap_separator_doc(
                &gap,
                if index > 0 {
                    root_item_separator_min_newlines(entries[index - 1].kind, entry.kind)
                } else {
                    1
                },
                index > 0,
                true,
            ));
            parts.push(entry.doc.clone());
            cursor = entry.end;
        }

        Doc::concat(parts)
    }

    fn root_entries(&self, items: &[Item]) -> Vec<RootEntry> {
        let mut entries = Vec::new();
        let mut index = 0;

        while index < items.len() {
            let item = items[index].clone();
            if matches!(item, Item::Stmt(Stmt::Import(_)))
                && let Some((entry, next_index)) = self.sorted_import_entry(items, index)
            {
                entries.push(entry);
                index = next_index;
                continue;
            }

            entries.push(RootEntry {
                start: u32::from(item.syntax().text_range().start()) as usize,
                end: u32::from(item.syntax().text_range().end()) as usize,
                kind: top_level_item_kind(item.clone()),
                doc: self.format_item(item.clone(), 0),
            });
            index += 1;
        }

        entries
    }

    fn sorted_import_entry(
        &self,
        items: &[Item],
        start_index: usize,
    ) -> Option<(RootEntry, usize)> {
        if self.options.import_sort_order == ImportSortOrder::Preserve {
            return None;
        }

        let mut end_index = start_index;
        while end_index < items.len() && matches!(items[end_index], Item::Stmt(Stmt::Import(_))) {
            if end_index > start_index
                && self.import_boundary_starts_new_group(
                    items[end_index - 1].clone(),
                    items[end_index].clone(),
                )
            {
                break;
            }
            end_index += 1;
        }

        if end_index - start_index < 2 {
            return None;
        }

        let import_items = &items[start_index..end_index];
        if !self.can_reorder_import_run(import_items) {
            return None;
        }

        let mut rendered_imports = import_items
            .iter()
            .map(|item| self.render_fragment(&self.format_item(item.clone(), 0), 0))
            .collect::<Vec<_>>();
        rendered_imports.sort();

        let mut parts = Vec::new();
        for (index, import) in rendered_imports.into_iter().enumerate() {
            if index > 0 {
                parts.push(Doc::hard_line());
            }
            parts.push(Doc::text(import));
        }

        Some((
            RootEntry {
                start: u32::from(import_items[0].syntax().text_range().start()) as usize,
                end: u32::from(
                    import_items[import_items.len() - 1]
                        .syntax()
                        .text_range()
                        .end(),
                ) as usize,
                kind: TopLevelItemKind::Import,
                doc: Doc::concat(parts),
            },
            end_index,
        ))
    }

    fn can_reorder_import_run(&self, items: &[Item]) -> bool {
        items.windows(2).all(|pair| {
            let [left, right] = pair else {
                return true;
            };
            self.is_whitespace_only_between_nodes(left.syntax(), right.syntax())
        })
    }

    fn import_boundary_starts_new_group(&self, left: Item, right: Item) -> bool {
        self.has_blank_line_between_nodes(left.syntax(), right.syntax())
    }

    pub(crate) fn format_item(&self, item: Item, indent: usize) -> Doc {
        if matches!(item_support(&item).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(item.syntax()));
        }

        if matches!(item_support(&item).level, FormatSupportLevel::Structural)
            && self.item_requires_raw_fallback(item.clone())
        {
            return Doc::text(self.raw(item.syntax()));
        }

        match item {
            Item::Fn(function) => self.format_function(function, indent),
            Item::Stmt(stmt) => self.format_stmt(stmt, indent),
        }
    }

    fn format_function(&self, function: FnItem, indent: usize) -> Doc {
        let params = function.params();
        let params_doc = self.format_params_doc(params.clone(), indent);
        let signature = self.format_function_signature_doc(&function);
        let body = function
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));

        let params_end = params
            .map(|params| u32::from(params.syntax().text_range().end()) as usize)
            .unwrap_or_else(|| u32::from(function.syntax().text_range().start()) as usize);
        let body_start = function
            .body()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(function.syntax().text_range().end()) as usize);

        Doc::concat(vec![
            signature,
            self.function_params_separator_doc(&function),
            params_doc,
            self.head_body_separator_doc(params_end, body_start),
            body,
        ])
    }

    pub(crate) fn format_stmt(&self, stmt: Stmt, indent: usize) -> Doc {
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
                let value_range = value.as_ref().map(|expr| expr.syntax().text_range());
                if let Some(value) = value {
                    let value_start = u32::from(value.syntax().text_range().start()) as usize;
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(break_stmt.syntax(), TokenKind::BreakKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(break_stmt.syntax().text_range().start()) as usize
                            }),
                        value_start,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value_range, &break_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Continue(continue_stmt) => {
                let keyword_end = self
                    .token_range(continue_stmt.syntax(), TokenKind::ContinueKw)
                    .map(range_end)
                    .unwrap_or_else(|| {
                        u32::from(continue_stmt.syntax().text_range().start()) as usize
                    });
                Doc::concat(vec![
                    Doc::text("continue"),
                    self.keyword_semicolon_doc(keyword_end, &continue_stmt.syntax()),
                ])
            }
            Stmt::Return(return_stmt) => {
                let head = Doc::text("return");
                let mut parts = vec![head.clone()];
                let value = return_stmt.value();
                let value_range = value.as_ref().map(|expr| expr.syntax().text_range());
                if let Some(value) = value {
                    let value_start = u32::from(value.syntax().text_range().start()) as usize;
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(return_stmt.syntax(), TokenKind::ReturnKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(return_stmt.syntax().text_range().start()) as usize
                            }),
                        value_start,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value_range, &return_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Throw(throw_stmt) => {
                let head = Doc::text("throw");
                let mut parts = vec![head.clone()];
                let value = throw_stmt.value();
                let value_range = value.as_ref().map(|expr| expr.syntax().text_range());
                if let Some(value) = value {
                    let value_start = u32::from(value.syntax().text_range().start()) as usize;
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(throw_stmt.syntax(), TokenKind::ThrowKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(throw_stmt.syntax().text_range().start()) as usize
                            }),
                        value_start,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(value_range, &throw_stmt.syntax()));
                Doc::concat(parts)
            }
            Stmt::Try(try_stmt) => {
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
                let mut parts = vec![
                    Doc::text("try"),
                    self.head_body_separator_doc(try_kw_end, body_start),
                    body,
                ];
                if let Some(catch_clause) = try_stmt.catch_clause() {
                    let catch_start = self
                        .token_range(catch_clause.syntax(), TokenKind::CatchKw)
                        .map(range_start)
                        .unwrap_or_else(|| {
                            u32::from(catch_clause.syntax().text_range().start()) as usize
                        });
                    parts.push(self.inline_or_gap_separator_doc(
                        body_end,
                        catch_start,
                        GapSeparatorOptions {
                            inline_text: " ",
                            minimum_newlines: 1,
                            has_previous: true,
                            has_next: true,
                            include_terminal_newline: true,
                        },
                    ));
                    parts.push(self.format_catch_clause_doc(catch_clause, indent));
                }
                Doc::concat(parts)
            }
            Stmt::Expr(expr_stmt) => self.format_expr_stmt(expr_stmt, indent),
        }
    }

    fn format_let_stmt(&self, let_stmt: LetStmt, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("let ")];
        if let Some(name) = let_stmt.name_token() {
            head_parts.push(Doc::text(name.text()));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        let initializer = let_stmt.initializer();
        let initializer_range = initializer.as_ref().map(|expr| expr.syntax().text_range());
        if let Some(initializer) = initializer {
            let initializer_start = u32::from(initializer.syntax().text_range().start()) as usize;
            parts[0] = self.format_assignment_statement_doc(
                &let_stmt.syntax(),
                head,
                self.format_expr_doc(initializer, indent),
                range_end(name_or_stmt_end(let_stmt.name_token(), &let_stmt.syntax())),
                initializer_start,
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(initializer_range, &let_stmt.syntax()));
        Doc::concat(parts)
    }

    fn format_const_stmt(&self, const_stmt: ConstStmt, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("const ")];
        if let Some(name) = const_stmt.name_token() {
            head_parts.push(Doc::text(name.text()));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        let value = const_stmt.value();
        let value_range = value.as_ref().map(|expr| expr.syntax().text_range());
        if let Some(value) = value {
            let value_start = u32::from(value.syntax().text_range().start()) as usize;
            parts[0] = self.format_assignment_statement_doc(
                &const_stmt.syntax(),
                head,
                self.format_expr_doc(value, indent),
                range_end(name_or_stmt_end(
                    const_stmt.name_token(),
                    &const_stmt.syntax(),
                )),
                value_start,
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(value_range, &const_stmt.syntax()));
        Doc::concat(parts)
    }

    fn format_import_stmt(&self, import_stmt: ImportStmt, indent: usize) -> Doc {
        if self.import_stmt_requires_raw_fallback(&import_stmt) {
            return Doc::text(self.raw(import_stmt.syntax()));
        }

        let module = import_stmt.module();
        let Some(module) = module else {
            return Doc::text("import;");
        };

        let mut tail_parts = vec![self.format_expr_doc(module.clone(), indent)];
        if let Some(alias) = import_stmt.alias() {
            let (start, end) = self.range_after_node_before_node(module.syntax(), alias.syntax());
            tail_parts.push(self.space_or_tight_statement_gap_doc(start, end));
            tail_parts.push(self.format_alias_clause_doc(alias));
        }
        let tail = Doc::concat(tail_parts);
        let module_start = range_start(module.syntax().text_range());
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

        if !self.has_comments_after_token_before_node(&import_kw, module.syntax())
            && self.statement_tail_renders_single_line(&tail, indent)
        {
            return Doc::group(Doc::concat(vec![
                Doc::text("import"),
                Doc::indent(1, Doc::concat(vec![Doc::soft_line(), tail])),
                Doc::text(";"),
            ]));
        }

        Doc::concat(vec![
            Doc::text("import"),
            self.space_or_tight_statement_gap_doc(range_end(import_kw.text_range()), module_start),
            tail,
            Doc::text(";"),
        ])
    }

    fn format_export_stmt(&self, export_stmt: rhai_syntax::ExportStmt, indent: usize) -> Doc {
        if self.export_stmt_requires_raw_fallback(&export_stmt) {
            return Doc::text(self.raw(export_stmt.syntax()));
        }

        if let Some(declaration) = export_stmt.declaration() {
            let declaration_doc = self.format_stmt(declaration.clone(), indent);
            let declaration_start = range_start(declaration.syntax().text_range());
            let Some(export_kw) = export_stmt
                .syntax()
                .direct_significant_tokens()
                .find(|token| token.kind().token_kind() == Some(TokenKind::ExportKw))
            else {
                return Doc::concat(vec![Doc::text("export "), declaration_doc]);
            };

            if !self.has_comments_after_token_before_node(&export_kw, declaration.syntax())
                && self.statement_tail_renders_single_line(&declaration_doc, indent)
            {
                return Doc::group(Doc::concat(vec![
                    Doc::text("export"),
                    Doc::indent(1, Doc::concat(vec![Doc::soft_line(), declaration_doc])),
                ]));
            }

            return Doc::concat(vec![
                Doc::text("export"),
                self.space_or_tight_statement_gap_doc(
                    range_end(export_kw.text_range()),
                    declaration_start,
                ),
                declaration_doc,
            ]);
        } else if let Some(target) = export_stmt.target() {
            let mut tail_parts = vec![self.format_expr_doc(target.clone(), indent)];
            if let Some(alias) = export_stmt.alias() {
                let (start, end) =
                    self.range_after_node_before_node(target.syntax(), alias.syntax());
                tail_parts.push(self.space_or_tight_statement_gap_doc(start, end));
                tail_parts.push(self.format_alias_clause_doc(alias));
            }
            let tail = Doc::concat(tail_parts);
            let target_start = range_start(target.syntax().text_range());
            let Some(export_kw) = export_stmt
                .syntax()
                .direct_significant_tokens()
                .find(|token| token.kind().token_kind() == Some(TokenKind::ExportKw))
            else {
                return Doc::concat(vec![Doc::text("export "), tail, Doc::text(";")]);
            };

            if !self.has_comments_after_token_before_node(&export_kw, target.syntax())
                && self.statement_tail_renders_single_line(&tail, indent)
            {
                return Doc::group(Doc::concat(vec![
                    Doc::text("export"),
                    Doc::indent(1, Doc::concat(vec![Doc::soft_line(), tail])),
                    Doc::text(";"),
                ]));
            }

            return Doc::concat(vec![
                Doc::text("export"),
                self.space_or_tight_statement_gap_doc(
                    range_end(export_kw.text_range()),
                    target_start,
                ),
                tail,
                Doc::text(";"),
            ]);
        }

        Doc::text("export;")
    }

    fn format_expr_stmt(&self, expr_stmt: ExprStmt, indent: usize) -> Doc {
        let mut parts = Vec::new();
        if let Some(expr) = expr_stmt.expr() {
            parts.push(self.format_expr_doc(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            parts.push(self.statement_semicolon_doc(
                expr_stmt.expr().map(|expr| expr.syntax().text_range()),
                &expr_stmt.syntax(),
            ));
        }
        Doc::concat(parts)
    }

    pub(crate) fn format_block_doc(&self, block: BlockExpr, indent: usize) -> Doc {
        let items = block
            .item_list()
            .map(|items| items.items().collect::<Vec<_>>())
            .unwrap_or_default();
        let open_brace_end = self
            .token_range(block.syntax(), TokenKind::OpenBrace)
            .map(|range| u32::from(range.end()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().text_range().start()) as usize);
        let close_brace_start = self
            .token_range(block.syntax(), TokenKind::CloseBrace)
            .map(|range| u32::from(range.start()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().text_range().end()) as usize);

        let first_item_start = items
            .first()
            .map(|item| u32::from(item.syntax().text_range().start()) as usize)
            .unwrap_or(close_brace_start);
        let leading_gap =
            self.comment_gap(open_brace_end, first_item_start, false, !items.is_empty());
        if items.is_empty() && !leading_gap.has_vertical_comments() {
            return Doc::text("{}");
        }

        let mut body_parts = Vec::new();
        if leading_gap.has_vertical_comments() {
            body_parts.push(self.render_line_comments_doc(leading_gap.vertical_comments()));

            let suffix_newlines = if items.is_empty() {
                leading_gap.trailing_blank_lines_before_next
            } else {
                leading_gap.trailing_blank_lines_before_next + 1
            };
            if suffix_newlines > 0 {
                body_parts.push(Doc::concat(vec![Doc::hard_line(); suffix_newlines]));
            }
        }

        let mut cursor = first_item_start;
        for (index, item) in items.iter().enumerate() {
            let item_start = u32::from(item.syntax().text_range().start()) as usize;
            let has_leading_content = !body_parts.is_empty();
            let skip_separator = index == 0 && has_leading_content && item_start == cursor;

            if !skip_separator {
                let gap = self.comment_gap(
                    cursor,
                    item_start,
                    index > 0 || !body_parts.is_empty(),
                    true,
                );
                body_parts.push(self.gap_separator_doc(
                    &gap,
                    1,
                    index > 0 || !body_parts.is_empty(),
                    true,
                ));
            }
            body_parts.push(self.format_item(item.clone(), indent + 1));
            cursor = u32::from(item.syntax().text_range().end()) as usize;
        }
        let trailing_gap = self.comment_gap(cursor, close_brace_start, !items.is_empty(), false);
        if !items.is_empty() && trailing_gap.has_comments() {
            body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if trailing_gap.has_vertical_comments() {
            body_parts.push(self.render_line_comments_doc(trailing_gap.vertical_comments()));
        }

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
        let mut body_parts = Vec::new();
        let mut cursor = u32::from(item_list.syntax().text_range().start()) as usize;

        for (index, item) in items.iter().enumerate() {
            let item_start = u32::from(item.syntax().text_range().start()) as usize;
            let gap = self.comment_gap(cursor, item_start, index > 0, true);
            body_parts.push(self.gap_separator_doc(&gap, 1, index > 0, true));
            body_parts.push(self.format_item(item.clone(), indent));
            cursor = u32::from(item.syntax().text_range().end()) as usize;
        }

        Doc::concat(body_parts)
    }

    fn item_requires_raw_fallback(&self, item: Item) -> bool {
        match item {
            Item::Fn(function) => self.function_requires_raw_fallback(&function),
            Item::Stmt(stmt) => self.stmt_requires_raw_fallback(stmt),
        }
    }

    fn stmt_requires_raw_fallback(&self, stmt: Stmt) -> bool {
        let stmt_syntax = stmt.syntax();
        match stmt {
            Stmt::Import(import_stmt) => self.import_stmt_requires_raw_fallback(&import_stmt),
            Stmt::Export(export_stmt) => self.export_stmt_requires_raw_fallback(&export_stmt),
            Stmt::Try(try_stmt) => self.try_stmt_requires_raw_fallback(try_stmt),
            Stmt::Let(let_stmt) => self.let_stmt_requires_raw_fallback(let_stmt),
            Stmt::Const(const_stmt) => self.const_stmt_requires_raw_fallback(const_stmt),
            Stmt::Break(ref break_stmt) => self.value_stmt_requires_raw_fallback(
                &stmt_syntax,
                self.token(stmt_syntax.clone(), TokenKind::BreakKw),
                break_stmt.value(),
                break_stmt.value().is_some(),
            ),
            Stmt::Continue(_) => self.continue_stmt_requires_raw_fallback(&stmt_syntax),
            Stmt::Return(ref return_stmt) => self.value_stmt_requires_raw_fallback(
                &stmt_syntax,
                self.token(stmt_syntax.clone(), TokenKind::ReturnKw),
                return_stmt.value(),
                return_stmt.value().is_some(),
            ),
            Stmt::Throw(ref throw_stmt) => self.value_stmt_requires_raw_fallback(
                &stmt_syntax,
                self.token(stmt_syntax.clone(), TokenKind::ThrowKw),
                throw_stmt.value(),
                throw_stmt.value().is_some(),
            ),
            Stmt::Expr(expr_stmt) => self.expr_stmt_requires_raw_fallback(expr_stmt),
        }
    }

    fn format_statement_tail_doc(
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

    fn statement_tail_renders_single_line(&self, doc: &Doc, indent: usize) -> bool {
        !self.render_fragment(doc, indent).contains('\n')
    }

    fn function_requires_raw_fallback(&self, function: &FnItem) -> bool {
        let Some(params) = function.params() else {
            return self.node_has_unowned_comments(function.syntax());
        };
        let signature_tokens = self.function_signature_tokens(function);
        let mut allowed_boundaries = signature_tokens
            .windows(2)
            .map(|pair| TriviaBoundary::TokenToken(pair[0].clone(), pair[1].clone()))
            .collect::<Vec<_>>();
        if let Some(last_token) = signature_tokens.last().cloned() {
            allowed_boundaries.push(TriviaBoundary::TokenNode(last_token, params.syntax()));
        }
        if let Some(body) = function.body() {
            allowed_boundaries.push(TriviaBoundary::NodeNode(params.syntax(), body.syntax()));
        }

        self.node_has_unowned_comments_outside_boundaries(function.syntax(), &allowed_boundaries)
    }

    fn try_stmt_requires_raw_fallback(&self, try_stmt: rhai_syntax::TryStmt) -> bool {
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

    fn catch_clause_requires_raw_fallback(&self, catch_clause: rhai_syntax::CatchClause) -> bool {
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

    fn import_stmt_requires_raw_fallback(&self, import_stmt: &ImportStmt) -> bool {
        let mut allowed_boundaries = Vec::new();
        if let (Some(module), Some(alias)) = (import_stmt.module(), import_stmt.alias()) {
            allowed_boundaries.push(TriviaBoundary::NodeNode(module.syntax(), alias.syntax()));
            if self.alias_clause_requires_raw_fallback(alias) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside_boundaries(import_stmt.syntax(), &allowed_boundaries)
    }

    fn export_stmt_requires_raw_fallback(&self, export_stmt: &rhai_syntax::ExportStmt) -> bool {
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

    fn alias_clause_requires_raw_fallback(&self, alias: rhai_syntax::AliasClause) -> bool {
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

    fn let_stmt_requires_raw_fallback(&self, let_stmt: LetStmt) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            &let_stmt.syntax(),
            name_or_stmt_end(let_stmt.name_token(), &let_stmt.syntax()),
            let_stmt.initializer(),
            let_stmt.initializer().is_some(),
        )
    }

    fn const_stmt_requires_raw_fallback(&self, const_stmt: ConstStmt) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            &const_stmt.syntax(),
            name_or_stmt_end(const_stmt.name_token(), &const_stmt.syntax()),
            const_stmt.value(),
            const_stmt.value().is_some(),
        )
    }

    fn assignment_stmt_requires_raw_fallback(
        &self,
        stmt: &SyntaxNode,
        head_end_range: rhai_syntax::TextRange,
        value_expr: Option<Expr>,
        has_value: bool,
    ) -> bool {
        let Some(eq_token) = self.token(stmt.clone(), TokenKind::Eq) else {
            return self.node_has_unowned_comments(stmt.clone());
        };

        let mut allowed_ranges = vec![(
            range_end(head_end_range),
            range_start(eq_token.text_range()),
        )];
        if let Some(value_expr) = value_expr {
            allowed_ranges.push(self.range_after_token_before_node(&eq_token, value_expr.syntax()));
            if let Some(semicolon_token) = self.token(stmt.clone(), TokenKind::Semicolon) {
                allowed_ranges.push(
                    self.range_after_node_before_token(value_expr.syntax(), &semicolon_token),
                );
            }
        } else if has_value {
            return self.node_has_unowned_comments(stmt.clone());
        }

        self.node_has_unowned_comments_outside(stmt.clone(), &allowed_ranges)
    }

    fn value_stmt_requires_raw_fallback(
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

    fn expr_stmt_requires_raw_fallback(&self, expr_stmt: ExprStmt) -> bool {
        let Some(expr) = expr_stmt.expr() else {
            return self.node_has_unowned_comments(expr_stmt.syntax());
        };

        let mut allowed_boundaries = Vec::new();
        if let Some(semicolon_token) = self.token(expr_stmt.syntax(), TokenKind::Semicolon) {
            allowed_boundaries.push(TriviaBoundary::NodeToken(expr.syntax(), semicolon_token));
        }

        self.node_has_unowned_comments_outside_boundaries(expr_stmt.syntax(), &allowed_boundaries)
    }

    fn continue_stmt_requires_raw_fallback(&self, stmt: &SyntaxNode) -> bool {
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

    fn format_assignment_statement_doc(
        &self,
        stmt: &SyntaxNode,
        head: Doc,
        value: Doc,
        head_end: usize,
        value_start: usize,
        indent: usize,
    ) -> Doc {
        let Some(eq_range) = self.token_range(stmt.clone(), TokenKind::Eq) else {
            return self.format_statement_tail_doc(head, "= ", value, indent);
        };

        if self.range_has_comments(head_end, range_start(eq_range))
            || self.range_has_comments(range_end(eq_range), value_start)
        {
            return Doc::concat(vec![
                head,
                self.space_or_tight_statement_gap_doc(head_end, range_start(eq_range)),
                Doc::text("="),
                self.space_or_tight_statement_gap_doc(range_end(eq_range), value_start),
                value,
            ]);
        }

        self.format_statement_tail_doc(head, "= ", value, indent)
    }

    fn format_value_statement_doc(
        &self,
        head: Doc,
        value: Doc,
        keyword_end: usize,
        value_start: usize,
        indent: usize,
    ) -> Doc {
        if self.range_has_comments(keyword_end, value_start) {
            return Doc::concat(vec![
                head,
                self.space_or_tight_statement_gap_doc(keyword_end, value_start),
                value,
            ]);
        }

        self.format_statement_tail_doc(head, "", value, indent)
    }

    fn statement_semicolon_doc(
        &self,
        value_range: Option<rhai_syntax::TextRange>,
        stmt: &SyntaxNode,
    ) -> Doc {
        let Some(semicolon_range) = self.token_range(stmt.clone(), TokenKind::Semicolon) else {
            return Doc::nil();
        };
        let Some(value_range) = value_range else {
            return Doc::text(";");
        };

        Doc::concat(vec![
            self.tight_comment_gap_doc_without_trailing_space(
                range_end(value_range),
                range_start(semicolon_range),
            ),
            Doc::text(";"),
        ])
    }

    fn keyword_semicolon_doc(&self, keyword_end: usize, stmt: &SyntaxNode) -> Doc {
        let Some(semicolon_range) = self.token_range(stmt.clone(), TokenKind::Semicolon) else {
            return Doc::nil();
        };

        Doc::concat(vec![
            self.tight_comment_gap_doc_without_trailing_space(
                keyword_end,
                range_start(semicolon_range),
            ),
            Doc::text(";"),
        ])
    }

    fn format_function_signature_doc(&self, function: &FnItem) -> Doc {
        let tokens = self.function_signature_tokens(function);
        let mut parts = Vec::new();

        for (index, token) in tokens.iter().cloned().enumerate() {
            if index > 0 {
                let previous = tokens[index - 1].clone();
                parts.push(self.function_signature_separator_doc(
                    range_end(previous.text_range()),
                    range_start(token.text_range()),
                    function_signature_inline_separator(&previous, &token),
                ));
            }

            parts.push(Doc::text(token.text()));
        }

        Doc::group(Doc::concat(parts))
    }

    fn function_params_separator_doc(&self, function: &FnItem) -> Doc {
        let Some(params) = function.params() else {
            return Doc::nil();
        };
        let Some(last_token) = self.function_signature_tokens(function).last().cloned() else {
            return Doc::nil();
        };

        if self.has_comments_after_token_before_node(&last_token, params.syntax()) {
            self.tight_comment_gap_after_token_before_node(&last_token, params.syntax())
        } else {
            Doc::nil()
        }
    }

    fn function_signature_tokens(&self, function: &FnItem) -> Vec<SyntaxToken> {
        let mut tokens = Vec::new();

        for child in function.syntax().children_with_tokens() {
            match child {
                rhai_syntax::SyntaxElement::Node(node)
                    if node.kind() == rhai_syntax::SyntaxKind::ParamList.to_rowan_kind() =>
                {
                    break;
                }
                rhai_syntax::SyntaxElement::Token(token)
                    if token
                        .kind()
                        .token_kind()
                        .is_some_and(|kind| !kind.is_trivia()) =>
                {
                    tokens.push(token);
                }
                _ => {}
            }
        }

        tokens
    }

    fn space_or_tight_statement_gap_doc(&self, start: usize, end: usize) -> Doc {
        if self.range_has_comments(start, end) {
            self.tight_comment_gap_doc(start, end)
        } else {
            Doc::text(" ")
        }
    }

    fn function_signature_separator_doc(&self, start: usize, end: usize, inline_text: &str) -> Doc {
        if !self.range_has_comments(start, end) {
            return if inline_text.is_empty() {
                Doc::text("")
            } else {
                Doc::soft_line()
            };
        }

        self.tight_comment_gap_doc(start, end)
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
            self.space_or_tight_statement_gap_doc(
                range_end(as_token.text_range()),
                range_start(alias_name.text_range()),
            ),
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

        Doc::concat(vec![
            Doc::text(catch_head),
            self.head_body_separator_doc(catch_head_end, catch_body_start),
            catch_body,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevelItemKind {
    Import,
    Export,
    Function,
    Other,
}

fn root_item_separator_min_newlines(
    previous_kind: TopLevelItemKind,
    current_kind: TopLevelItemKind,
) -> usize {
    if previous_kind == TopLevelItemKind::Function || current_kind == TopLevelItemKind::Function {
        return 2;
    }

    if previous_kind != current_kind
        && matches!(
            previous_kind,
            TopLevelItemKind::Import | TopLevelItemKind::Export
        )
    {
        return 2;
    }

    if previous_kind != current_kind
        && matches!(
            current_kind,
            TopLevelItemKind::Import | TopLevelItemKind::Export
        )
    {
        return 2;
    }

    1
}

fn top_level_item_kind(item: Item) -> TopLevelItemKind {
    match item {
        Item::Fn(_) => TopLevelItemKind::Function,
        Item::Stmt(Stmt::Import(_)) => TopLevelItemKind::Import,
        Item::Stmt(Stmt::Export(_)) => TopLevelItemKind::Export,
        Item::Stmt(_) => TopLevelItemKind::Other,
    }
}

#[derive(Debug, Clone)]
struct RootEntry {
    start: usize,
    end: usize,
    kind: TopLevelItemKind,
    doc: Doc,
}

fn range_start(range: rhai_syntax::TextRange) -> usize {
    u32::from(range.start()) as usize
}

fn range_end(range: rhai_syntax::TextRange) -> usize {
    u32::from(range.end()) as usize
}

fn name_or_stmt_end(name: Option<SyntaxToken>, stmt: &SyntaxNode) -> rhai_syntax::TextRange {
    name.map(|token| token.text_range())
        .unwrap_or_else(|| stmt.text_range())
}

fn function_signature_inline_separator(
    previous: &SyntaxToken,
    current: &SyntaxToken,
) -> &'static str {
    match (previous.kind().token_kind(), current.kind().token_kind()) {
        (Some(TokenKind::PrivateKw), Some(TokenKind::FnKw)) => " ",
        (Some(TokenKind::FnKw), _) => " ",
        (Some(TokenKind::Dot), _) => "",
        (_, Some(TokenKind::Dot)) => "",
        _ => " ",
    }
}
