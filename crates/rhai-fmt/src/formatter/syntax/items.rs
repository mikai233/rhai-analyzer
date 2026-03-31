use rhai_syntax::{
    AstNode, BlockExpr, ConstStmt, ExprStmt, FnItem, ImportStmt, Item, LetStmt, Root, Stmt,
    SyntaxElement, SyntaxToken, TokenKind,
};

use crate::ImportSortOrder;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, item_support, stmt_support};
use crate::formatter::trivia::comments::GapSeparatorOptions;

impl Formatter<'_> {
    pub(crate) fn format_root(&self, root: Root<'_>) -> Doc {
        let items = root.items().collect::<Vec<_>>();
        let entries = self.root_entries(&items);
        let mut parts = Vec::new();
        let mut cursor = u32::from(root.syntax().range().start()) as usize;

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

        let root_end = u32::from(root.syntax().range().end()) as usize;
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

    fn root_entries(&self, items: &[Item<'_>]) -> Vec<RootEntry> {
        let mut entries = Vec::new();
        let mut index = 0;

        while index < items.len() {
            let item = items[index];
            if matches!(item, Item::Stmt(Stmt::Import(_)))
                && let Some((entry, next_index)) = self.sorted_import_entry(items, index)
            {
                entries.push(entry);
                index = next_index;
                continue;
            }

            entries.push(RootEntry {
                start: u32::from(item.syntax().range().start()) as usize,
                end: u32::from(item.syntax().range().end()) as usize,
                kind: top_level_item_kind(item),
                doc: self.format_item(item, 0),
            });
            index += 1;
        }

        entries
    }

    fn sorted_import_entry(
        &self,
        items: &[Item<'_>],
        start_index: usize,
    ) -> Option<(RootEntry, usize)> {
        if self.options.import_sort_order == ImportSortOrder::Preserve {
            return None;
        }

        let mut end_index = start_index;
        while end_index < items.len() && matches!(items[end_index], Item::Stmt(Stmt::Import(_))) {
            if end_index > start_index
                && self.import_boundary_starts_new_group(items[end_index - 1], items[end_index])
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
            .map(|item| self.render_fragment(&self.format_item(*item, 0), 0))
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
                start: u32::from(import_items[0].syntax().range().start()) as usize,
                end: u32::from(import_items[import_items.len() - 1].syntax().range().end())
                    as usize,
                kind: TopLevelItemKind::Import,
                doc: Doc::concat(parts),
            },
            end_index,
        ))
    }

    fn can_reorder_import_run(&self, items: &[Item<'_>]) -> bool {
        items.windows(2).all(|pair| {
            let [left, right] = pair else {
                return true;
            };
            let between = &self.source[u32::from(left.syntax().range().end()) as usize
                ..u32::from(right.syntax().range().start()) as usize];
            between.chars().all(char::is_whitespace)
        })
    }

    fn import_boundary_starts_new_group(&self, left: Item<'_>, right: Item<'_>) -> bool {
        let between = &self.source[u32::from(left.syntax().range().end()) as usize
            ..u32::from(right.syntax().range().start()) as usize];
        between.matches('\n').count() >= 2
    }

    pub(crate) fn format_item(&self, item: Item<'_>, indent: usize) -> Doc {
        if matches!(item_support(item).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(item.syntax()));
        }

        if matches!(item_support(item).level, FormatSupportLevel::Structural)
            && self.item_requires_raw_fallback(item)
        {
            return Doc::text(self.raw(item.syntax()));
        }

        match item {
            Item::Fn(function) => self.format_function(function, indent),
            Item::Stmt(stmt) => self.format_stmt(stmt, indent),
        }
    }

    fn format_function(&self, function: FnItem<'_>, indent: usize) -> Doc {
        let params = function.params();
        let params_doc = self.format_params_doc(params, indent);
        let signature = self.format_function_signature_doc(function);
        let body = function
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));

        let params_end = params
            .map(|params| u32::from(params.syntax().range().end()) as usize)
            .unwrap_or_else(|| u32::from(function.syntax().range().start()) as usize);
        let body_start = function
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(function.syntax().range().end()) as usize);

        Doc::concat(vec![
            signature,
            self.function_params_separator_doc(function),
            params_doc,
            self.head_body_separator_doc(params_end, body_start),
            body,
        ])
    }

    pub(crate) fn format_stmt(&self, stmt: Stmt<'_>, indent: usize) -> Doc {
        if matches!(stmt_support(stmt).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(stmt.syntax()));
        }

        if matches!(stmt_support(stmt).level, FormatSupportLevel::Structural)
            && self.stmt_requires_raw_fallback(stmt)
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
                if let Some(value) = break_stmt.value() {
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(break_stmt.syntax(), TokenKind::BreakKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(break_stmt.syntax().range().start()) as usize
                            }),
                        u32::from(value.syntax().range().start()) as usize,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(
                    break_stmt.value().map(|expr| expr.syntax().range()),
                    break_stmt.syntax(),
                ));
                Doc::concat(parts)
            }
            Stmt::Continue(continue_stmt) => {
                let keyword_end = self
                    .token_range(continue_stmt.syntax(), TokenKind::ContinueKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(continue_stmt.syntax().range().start()) as usize);
                Doc::concat(vec![
                    Doc::text("continue"),
                    self.keyword_semicolon_doc(keyword_end, continue_stmt.syntax()),
                ])
            }
            Stmt::Return(return_stmt) => {
                let head = Doc::text("return");
                let mut parts = vec![head.clone()];
                if let Some(value) = return_stmt.value() {
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(return_stmt.syntax(), TokenKind::ReturnKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(return_stmt.syntax().range().start()) as usize
                            }),
                        u32::from(value.syntax().range().start()) as usize,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(
                    return_stmt.value().map(|expr| expr.syntax().range()),
                    return_stmt.syntax(),
                ));
                Doc::concat(parts)
            }
            Stmt::Throw(throw_stmt) => {
                let head = Doc::text("throw");
                let mut parts = vec![head.clone()];
                if let Some(value) = throw_stmt.value() {
                    parts[0] = self.format_value_statement_doc(
                        head,
                        self.format_expr_doc(value, indent),
                        self.token_range(throw_stmt.syntax(), TokenKind::ThrowKw)
                            .map(range_end)
                            .unwrap_or_else(|| {
                                u32::from(throw_stmt.syntax().range().start()) as usize
                            }),
                        u32::from(value.syntax().range().start()) as usize,
                        indent,
                    );
                }
                parts.push(self.statement_semicolon_doc(
                    throw_stmt.value().map(|expr| expr.syntax().range()),
                    throw_stmt.syntax(),
                ));
                Doc::concat(parts)
            }
            Stmt::Try(try_stmt) => {
                let body_expr = try_stmt.body();
                let body = body_expr
                    .map(|body| self.format_block_doc(body, indent))
                    .unwrap_or_else(|| Doc::text("{}"));
                let try_kw_end = self
                    .token_range(try_stmt.syntax(), TokenKind::TryKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(try_stmt.syntax().range().start()) as usize);
                let body_start = body_expr
                    .map(|body| u32::from(body.syntax().range().start()) as usize)
                    .unwrap_or_else(|| u32::from(try_stmt.syntax().range().end()) as usize);
                let body_end = body_expr
                    .map(|body| u32::from(body.syntax().range().end()) as usize)
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
                            u32::from(catch_clause.syntax().range().start()) as usize
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

    fn format_let_stmt(&self, let_stmt: LetStmt<'_>, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("let ")];
        if let Some(name) = let_stmt.name_token() {
            head_parts.push(Doc::text(name.text(self.source)));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        if let Some(initializer) = let_stmt.initializer() {
            parts[0] = self.format_assignment_statement_doc(
                let_stmt.syntax(),
                head,
                self.format_expr_doc(initializer, indent),
                range_end(name_or_stmt_end(let_stmt.name_token(), let_stmt.syntax())),
                u32::from(initializer.syntax().range().start()) as usize,
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(
            let_stmt.initializer().map(|expr| expr.syntax().range()),
            let_stmt.syntax(),
        ));
        Doc::concat(parts)
    }

    fn format_const_stmt(&self, const_stmt: ConstStmt<'_>, indent: usize) -> Doc {
        let mut head_parts = vec![Doc::text("const ")];
        if let Some(name) = const_stmt.name_token() {
            head_parts.push(Doc::text(name.text(self.source)));
        }
        let head = Doc::concat(head_parts);
        let mut parts = vec![head.clone()];
        if let Some(value) = const_stmt.value() {
            parts[0] = self.format_assignment_statement_doc(
                const_stmt.syntax(),
                head,
                self.format_expr_doc(value, indent),
                range_end(name_or_stmt_end(
                    const_stmt.name_token(),
                    const_stmt.syntax(),
                )),
                u32::from(value.syntax().range().start()) as usize,
                indent,
            );
        }
        parts.push(self.statement_semicolon_doc(
            const_stmt.value().map(|expr| expr.syntax().range()),
            const_stmt.syntax(),
        ));
        Doc::concat(parts)
    }

    fn format_import_stmt(&self, import_stmt: ImportStmt<'_>, indent: usize) -> Doc {
        if self.import_stmt_requires_raw_fallback(import_stmt) {
            return Doc::text(self.raw(import_stmt.syntax()));
        }

        let module = import_stmt.module();
        let Some(module) = module else {
            return Doc::text("import;");
        };

        let mut tail_parts = vec![self.format_expr_doc(module, indent)];
        if let Some(alias) = import_stmt.alias() {
            tail_parts.push(self.space_or_tight_statement_gap_doc(
                range_end(module.syntax().range()),
                range_start(alias.syntax().range()),
            ));
            tail_parts.push(self.format_alias_clause_doc(alias));
        }
        let tail = Doc::concat(tail_parts);
        let module_start = range_start(module.syntax().range());
        let keyword_end = self
            .token_range(import_stmt.syntax(), TokenKind::ImportKw)
            .map(range_end)
            .unwrap_or(module_start);

        if !self.range_has_comments(keyword_end, module_start)
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
            self.space_or_tight_statement_gap_doc(keyword_end, module_start),
            tail,
            Doc::text(";"),
        ])
    }

    fn format_export_stmt(&self, export_stmt: rhai_syntax::ExportStmt<'_>, indent: usize) -> Doc {
        if self.export_stmt_requires_raw_fallback(export_stmt) {
            return Doc::text(self.raw(export_stmt.syntax()));
        }

        if let Some(declaration) = export_stmt.declaration() {
            let declaration_doc = self.format_stmt(declaration, indent);
            let declaration_start = range_start(declaration.syntax().range());
            let keyword_end = self
                .token_range(export_stmt.syntax(), TokenKind::ExportKw)
                .map(range_end)
                .unwrap_or(declaration_start);

            if !self.range_has_comments(keyword_end, declaration_start)
                && self.statement_tail_renders_single_line(&declaration_doc, indent)
            {
                return Doc::group(Doc::concat(vec![
                    Doc::text("export"),
                    Doc::indent(1, Doc::concat(vec![Doc::soft_line(), declaration_doc])),
                ]));
            }

            return Doc::concat(vec![
                Doc::text("export"),
                self.space_or_tight_statement_gap_doc(keyword_end, declaration_start),
                declaration_doc,
            ]);
        } else if let Some(target) = export_stmt.target() {
            let mut tail_parts = vec![self.format_expr_doc(target, indent)];
            if let Some(alias) = export_stmt.alias() {
                tail_parts.push(self.space_or_tight_statement_gap_doc(
                    range_end(target.syntax().range()),
                    range_start(alias.syntax().range()),
                ));
                tail_parts.push(self.format_alias_clause_doc(alias));
            }
            let tail = Doc::concat(tail_parts);
            let target_start = range_start(target.syntax().range());
            let keyword_end = self
                .token_range(export_stmt.syntax(), TokenKind::ExportKw)
                .map(range_end)
                .unwrap_or(target_start);

            if !self.range_has_comments(keyword_end, target_start)
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
                self.space_or_tight_statement_gap_doc(keyword_end, target_start),
                tail,
                Doc::text(";"),
            ]);
        }

        Doc::text("export;")
    }

    fn format_expr_stmt(&self, expr_stmt: ExprStmt<'_>, indent: usize) -> Doc {
        let mut parts = Vec::new();
        if let Some(expr) = expr_stmt.expr() {
            parts.push(self.format_expr_doc(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            parts.push(self.statement_semicolon_doc(
                expr_stmt.expr().map(|expr| expr.syntax().range()),
                expr_stmt.syntax(),
            ));
        }
        Doc::concat(parts)
    }

    pub(crate) fn format_block_doc(&self, block: BlockExpr<'_>, indent: usize) -> Doc {
        let items = block.items().collect::<Vec<_>>();
        let open_brace_end = self
            .token_range(block.syntax(), TokenKind::OpenBrace)
            .map(|range| u32::from(range.end()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().range().start()) as usize);
        let close_brace_start = self
            .token_range(block.syntax(), TokenKind::CloseBrace)
            .map(|range| u32::from(range.start()) as usize)
            .unwrap_or_else(|| u32::from(block.syntax().range().end()) as usize);

        let first_item_start = items
            .first()
            .map(|item| u32::from(item.syntax().range().start()) as usize)
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
            let item_start = u32::from(item.syntax().range().start()) as usize;
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
            body_parts.push(self.format_item(*item, indent + 1));
            cursor = u32::from(item.syntax().range().end()) as usize;
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

    fn item_requires_raw_fallback(&self, item: Item<'_>) -> bool {
        match item {
            Item::Fn(function) => self.function_requires_raw_fallback(function),
            Item::Stmt(stmt) => self.stmt_requires_raw_fallback(stmt),
        }
    }

    fn stmt_requires_raw_fallback(&self, stmt: Stmt<'_>) -> bool {
        match stmt {
            Stmt::Import(import_stmt) => self.import_stmt_requires_raw_fallback(import_stmt),
            Stmt::Export(export_stmt) => self.export_stmt_requires_raw_fallback(export_stmt),
            Stmt::Try(try_stmt) => self.try_stmt_requires_raw_fallback(try_stmt),
            Stmt::Let(let_stmt) => self.let_stmt_requires_raw_fallback(let_stmt),
            Stmt::Const(const_stmt) => self.const_stmt_requires_raw_fallback(const_stmt),
            Stmt::Break(break_stmt) => self.value_stmt_requires_raw_fallback(
                stmt.syntax(),
                self.token_range(stmt.syntax(), TokenKind::BreakKw)
                    .map(range_end),
                break_stmt.value().map(|expr| expr.syntax().range()),
                break_stmt.value().is_some(),
            ),
            Stmt::Continue(_) => self.continue_stmt_requires_raw_fallback(stmt.syntax()),
            Stmt::Return(return_stmt) => self.value_stmt_requires_raw_fallback(
                stmt.syntax(),
                self.token_range(stmt.syntax(), TokenKind::ReturnKw)
                    .map(range_end),
                return_stmt.value().map(|expr| expr.syntax().range()),
                return_stmt.value().is_some(),
            ),
            Stmt::Throw(throw_stmt) => self.value_stmt_requires_raw_fallback(
                stmt.syntax(),
                self.token_range(stmt.syntax(), TokenKind::ThrowKw)
                    .map(range_end),
                throw_stmt.value().map(|expr| expr.syntax().range()),
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

    fn function_requires_raw_fallback(&self, function: FnItem<'_>) -> bool {
        let Some(params) = function.params() else {
            return self.node_has_unowned_comments(function.syntax());
        };
        let params_end = u32::from(params.syntax().range().end()) as usize;
        let signature_tokens = self.function_signature_tokens(function);
        let params_start = u32::from(params.syntax().range().start()) as usize;
        let mut allowed_ranges = signature_tokens
            .windows(2)
            .map(|pair| (range_end(pair[0].range()), range_start(pair[1].range())))
            .collect::<Vec<_>>();
        if let Some(last_token) = signature_tokens.last().copied() {
            allowed_ranges.push((range_end(last_token.range()), params_start));
        }
        if let Some(body) = function.body() {
            allowed_ranges.push((
                params_end,
                u32::from(body.syntax().range().start()) as usize,
            ));
        }

        self.node_has_unowned_comments_outside(function.syntax(), &allowed_ranges)
    }

    fn try_stmt_requires_raw_fallback(&self, try_stmt: rhai_syntax::TryStmt<'_>) -> bool {
        let body_start = try_stmt
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(try_stmt.syntax().range().end()) as usize);
        let body_end = try_stmt
            .body()
            .map(|body| u32::from(body.syntax().range().end()) as usize)
            .unwrap_or(body_start);
        let try_kw_end = self
            .token_range(try_stmt.syntax(), TokenKind::TryKw)
            .map(range_end)
            .unwrap_or(body_start);

        let mut allowed_ranges = vec![(try_kw_end, body_start)];
        if let Some(catch_clause) = try_stmt.catch_clause() {
            let catch_start = self
                .token_range(catch_clause.syntax(), TokenKind::CatchKw)
                .map(range_start)
                .unwrap_or_else(|| u32::from(catch_clause.syntax().range().start()) as usize);
            allowed_ranges.push((body_end, catch_start));

            if self.catch_clause_requires_raw_fallback(catch_clause) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside(try_stmt.syntax(), &allowed_ranges)
    }

    fn catch_clause_requires_raw_fallback(
        &self,
        catch_clause: rhai_syntax::CatchClause<'_>,
    ) -> bool {
        let body_start = catch_clause
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(catch_clause.syntax().range().end()) as usize);
        let catch_head_end = self
            .token_range(catch_clause.syntax(), TokenKind::CloseParen)
            .map(range_end)
            .or_else(|| {
                self.token_range(catch_clause.syntax(), TokenKind::CatchKw)
                    .map(range_end)
            })
            .unwrap_or(body_start);

        self.node_has_unowned_comments_outside(
            catch_clause.syntax(),
            &[(catch_head_end, body_start)],
        )
    }

    fn import_stmt_requires_raw_fallback(&self, import_stmt: ImportStmt<'_>) -> bool {
        let mut allowed_ranges = Vec::new();
        if let (Some(module), Some(alias)) = (import_stmt.module(), import_stmt.alias()) {
            allowed_ranges.push((
                range_end(module.syntax().range()),
                range_start(alias.syntax().range()),
            ));
            if self.alias_clause_requires_raw_fallback(alias) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside(import_stmt.syntax(), &allowed_ranges)
    }

    fn export_stmt_requires_raw_fallback(&self, export_stmt: rhai_syntax::ExportStmt<'_>) -> bool {
        let mut allowed_ranges = Vec::new();
        if let Some(alias) = export_stmt.alias() {
            if self.alias_clause_requires_raw_fallback(alias) {
                return true;
            }

            if let Some(target) = export_stmt.target() {
                allowed_ranges.push((
                    range_end(target.syntax().range()),
                    range_start(alias.syntax().range()),
                ));
            }
        }

        self.node_has_unowned_comments_outside(export_stmt.syntax(), &allowed_ranges)
    }

    fn alias_clause_requires_raw_fallback(&self, alias: rhai_syntax::AliasClause<'_>) -> bool {
        let Some(as_range) = self.token_range(alias.syntax(), TokenKind::AsKw) else {
            return self.node_has_unowned_comments(alias.syntax());
        };
        let Some(alias_name) = alias.alias_token() else {
            return self.node_has_unowned_comments(alias.syntax());
        };

        self.node_has_unowned_comments_outside(
            alias.syntax(),
            &[(range_end(as_range), range_start(alias_name.range()))],
        )
    }

    fn let_stmt_requires_raw_fallback(&self, let_stmt: LetStmt<'_>) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            let_stmt.syntax(),
            name_or_stmt_end(let_stmt.name_token(), let_stmt.syntax()),
            let_stmt.initializer().map(|expr| expr.syntax().range()),
            let_stmt.initializer().is_some(),
        )
    }

    fn const_stmt_requires_raw_fallback(&self, const_stmt: ConstStmt<'_>) -> bool {
        self.assignment_stmt_requires_raw_fallback(
            const_stmt.syntax(),
            name_or_stmt_end(const_stmt.name_token(), const_stmt.syntax()),
            const_stmt.value().map(|expr| expr.syntax().range()),
            const_stmt.value().is_some(),
        )
    }

    fn assignment_stmt_requires_raw_fallback(
        &self,
        stmt: &rhai_syntax::SyntaxNode,
        head_end_range: rhai_syntax::TextRange,
        value_range: Option<rhai_syntax::TextRange>,
        has_value: bool,
    ) -> bool {
        let Some(eq_range) = self.token_range(stmt, TokenKind::Eq) else {
            return self.node_has_unowned_comments(stmt);
        };

        let mut allowed_ranges = vec![(range_end(head_end_range), range_start(eq_range))];
        if let Some(value_range) = value_range {
            allowed_ranges.push((range_end(eq_range), range_start(value_range)));
            if let Some(semicolon_range) = self.token_range(stmt, TokenKind::Semicolon) {
                allowed_ranges.push((range_end(value_range), range_start(semicolon_range)));
            }
        } else if has_value {
            return self.node_has_unowned_comments(stmt);
        }

        self.node_has_unowned_comments_outside(stmt, &allowed_ranges)
    }

    fn value_stmt_requires_raw_fallback(
        &self,
        stmt: &rhai_syntax::SyntaxNode,
        keyword_end: Option<usize>,
        value_range: Option<rhai_syntax::TextRange>,
        has_value: bool,
    ) -> bool {
        let mut allowed_ranges = Vec::new();
        if let (Some(keyword_end), Some(value_range)) = (keyword_end, value_range) {
            allowed_ranges.push((keyword_end, range_start(value_range)));
            if let Some(semicolon_range) = self.token_range(stmt, TokenKind::Semicolon) {
                allowed_ranges.push((range_end(value_range), range_start(semicolon_range)));
            }
        } else if has_value {
            return self.node_has_unowned_comments(stmt);
        }

        self.node_has_unowned_comments_outside(stmt, &allowed_ranges)
    }

    fn expr_stmt_requires_raw_fallback(&self, expr_stmt: ExprStmt<'_>) -> bool {
        let Some(expr_range) = expr_stmt.expr().map(|expr| expr.syntax().range()) else {
            return self.node_has_unowned_comments(expr_stmt.syntax());
        };

        let mut allowed_ranges = Vec::new();
        if let Some(semicolon_range) = self.token_range(expr_stmt.syntax(), TokenKind::Semicolon) {
            allowed_ranges.push((range_end(expr_range), range_start(semicolon_range)));
        }

        self.node_has_unowned_comments_outside(expr_stmt.syntax(), &allowed_ranges)
    }

    fn continue_stmt_requires_raw_fallback(&self, stmt: &rhai_syntax::SyntaxNode) -> bool {
        let Some(keyword_end) = self.token_range(stmt, TokenKind::ContinueKw).map(range_end) else {
            return self.node_has_unowned_comments(stmt);
        };
        let Some(semicolon_range) = self.token_range(stmt, TokenKind::Semicolon) else {
            return self.node_has_unowned_comments(stmt);
        };

        self.node_has_unowned_comments_outside(stmt, &[(keyword_end, range_start(semicolon_range))])
    }

    fn format_assignment_statement_doc(
        &self,
        stmt: &rhai_syntax::SyntaxNode,
        head: Doc,
        value: Doc,
        head_end: usize,
        value_start: usize,
        indent: usize,
    ) -> Doc {
        let Some(eq_range) = self.token_range(stmt, TokenKind::Eq) else {
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
        stmt: &rhai_syntax::SyntaxNode,
    ) -> Doc {
        let Some(semicolon_range) = self.token_range(stmt, TokenKind::Semicolon) else {
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

    fn keyword_semicolon_doc(&self, keyword_end: usize, stmt: &rhai_syntax::SyntaxNode) -> Doc {
        let Some(semicolon_range) = self.token_range(stmt, TokenKind::Semicolon) else {
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

    fn format_function_signature_doc(&self, function: FnItem<'_>) -> Doc {
        let tokens = self.function_signature_tokens(function);
        let mut parts = Vec::new();

        for (index, token) in tokens.iter().copied().enumerate() {
            if index > 0 {
                let previous = tokens[index - 1];
                parts.push(self.function_signature_separator_doc(
                    range_end(previous.range()),
                    range_start(token.range()),
                    function_signature_inline_separator(previous, token),
                ));
            }

            parts.push(Doc::text(token.text(self.source)));
        }

        Doc::group(Doc::concat(parts))
    }

    fn function_params_separator_doc(&self, function: FnItem<'_>) -> Doc {
        let Some(params_start) = function
            .params()
            .map(|params| range_start(params.syntax().range()))
        else {
            return Doc::nil();
        };
        let Some(last_token) = self.function_signature_tokens(function).last().copied() else {
            return Doc::nil();
        };

        if self.range_has_comments(range_end(last_token.range()), params_start) {
            self.tight_comment_gap_doc(range_end(last_token.range()), params_start)
        } else {
            Doc::nil()
        }
    }

    fn function_signature_tokens(&self, function: FnItem<'_>) -> Vec<SyntaxToken> {
        let mut tokens = Vec::new();

        for child in function.syntax().children() {
            match child {
                SyntaxElement::Node(node) if node.kind() == rhai_syntax::SyntaxKind::ParamList => {
                    break;
                }
                SyntaxElement::Token(token) => tokens.push(*token),
                SyntaxElement::Node(_) => {}
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

    pub(crate) fn format_alias_clause_doc(&self, alias: rhai_syntax::AliasClause<'_>) -> Doc {
        let Some(alias_name) = alias.alias_token() else {
            return Doc::text(self.raw(alias.syntax()));
        };
        let Some(as_range) = self.token_range(alias.syntax(), TokenKind::AsKw) else {
            return Doc::text(self.raw(alias.syntax()));
        };

        Doc::concat(vec![
            Doc::text("as"),
            self.space_or_tight_statement_gap_doc(
                range_end(as_range),
                range_start(alias_name.range()),
            ),
            Doc::text(alias_name.text(self.source)),
        ])
    }

    pub(crate) fn format_catch_clause_doc(
        &self,
        catch_clause: rhai_syntax::CatchClause<'_>,
        indent: usize,
    ) -> Doc {
        let mut catch_head = String::from("catch");
        if let Some(binding) = catch_clause.binding_token() {
            catch_head.push_str(" (");
            catch_head.push_str(binding.text(self.source));
            catch_head.push(')');
        }
        let catch_body = catch_clause
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let catch_body_start = catch_clause
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(catch_clause.syntax().range().end()) as usize);
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

fn top_level_item_kind(item: Item<'_>) -> TopLevelItemKind {
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

fn name_or_stmt_end(
    name: Option<SyntaxToken>,
    stmt: &rhai_syntax::SyntaxNode,
) -> rhai_syntax::TextRange {
    name.map(|token| token.range())
        .unwrap_or_else(|| stmt.range())
}

fn function_signature_inline_separator(
    previous: SyntaxToken,
    current: SyntaxToken,
) -> &'static str {
    match (previous.kind(), current.kind()) {
        (TokenKind::PrivateKw, TokenKind::FnKw) => " ",
        (TokenKind::FnKw, _) => " ",
        (TokenKind::Dot, _) => "",
        (_, TokenKind::Dot) => "",
        _ => " ",
    }
}
