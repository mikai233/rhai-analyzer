use rhai_syntax::{
    AstNode, BlockExpr, ConstStmt, ExprStmt, FnItem, ImportStmt, Item, LetStmt, Root, Stmt,
    TokenKind,
};

use crate::ImportSortOrder;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn format_root(&self, root: Root<'_>) -> Doc {
        let items = root.items().collect::<Vec<_>>();
        let entries = self.root_entries(&items);
        let mut parts = Vec::new();
        let mut cursor = u32::from(root.syntax().range().start()) as usize;

        for (index, entry) in entries.iter().enumerate() {
            let gap = self.comment_gap(cursor, entry.start, index > 0);
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
        let trailing_gap = self.comment_gap(cursor, root_end, !items.is_empty());
        if !items.is_empty()
            && (!trailing_gap.trailing_comments.is_empty()
                || !trailing_gap.line_comments.is_empty())
        {
            parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if !trailing_gap.line_comments.is_empty() {
            parts.push(self.render_line_comments_doc(&trailing_gap.line_comments));
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
        match item {
            Item::Fn(function) => self.format_function(function, indent),
            Item::Stmt(stmt) => self.format_stmt(stmt, indent),
        }
    }

    fn format_function(&self, function: FnItem<'_>, indent: usize) -> Doc {
        if self.node_has_unowned_comments(function.syntax()) {
            return Doc::text(self.raw(function.syntax()));
        }

        let mut signature = String::new();
        if function.is_private() {
            signature.push_str("private ");
        }
        signature.push_str("fn ");

        if let Some(receiver) = function.this_type_token() {
            signature.push_str(receiver.text(self.source));
            signature.push('.');
        }

        if let Some(name) = function.name_token() {
            signature.push_str(name.text(self.source));
        }

        let body = function
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));

        Doc::concat(vec![
            Doc::text(signature),
            self.format_params_doc(function.params(), indent),
            Doc::text(" "),
            body,
        ])
    }

    pub(crate) fn format_stmt(&self, stmt: Stmt<'_>, indent: usize) -> Doc {
        if !matches!(stmt, Stmt::Expr(_)) && self.node_has_unowned_comments(stmt.syntax()) {
            return Doc::text(self.raw(stmt.syntax()));
        }

        match stmt {
            Stmt::Let(let_stmt) => self.format_let_stmt(let_stmt, indent),
            Stmt::Const(const_stmt) => self.format_const_stmt(const_stmt, indent),
            Stmt::Import(import_stmt) => self.format_import_stmt(import_stmt, indent),
            Stmt::Export(export_stmt) => self.format_export_stmt(export_stmt, indent),
            Stmt::Break(break_stmt) => {
                let mut parts = vec![Doc::text("break")];
                if let Some(value) = break_stmt.value() {
                    parts.push(Doc::text(" "));
                    parts.push(self.format_expr_doc(value, indent));
                }
                parts.push(Doc::text(";"));
                Doc::concat(parts)
            }
            Stmt::Continue(_) => Doc::text("continue;"),
            Stmt::Return(return_stmt) => {
                let mut parts = vec![Doc::text("return")];
                if let Some(value) = return_stmt.value() {
                    parts.push(Doc::text(" "));
                    parts.push(self.format_expr_doc(value, indent));
                }
                parts.push(Doc::text(";"));
                Doc::concat(parts)
            }
            Stmt::Throw(throw_stmt) => {
                let mut parts = vec![Doc::text("throw")];
                if let Some(value) = throw_stmt.value() {
                    parts.push(Doc::text(" "));
                    parts.push(self.format_expr_doc(value, indent));
                }
                parts.push(Doc::text(";"));
                Doc::concat(parts)
            }
            Stmt::Try(try_stmt) => {
                if try_stmt.catch_clause().is_some_and(|catch_clause| {
                    self.node_has_unowned_comments(catch_clause.syntax())
                }) {
                    return Doc::text(self.raw(try_stmt.syntax()));
                }

                let mut parts = vec![
                    Doc::text("try "),
                    try_stmt
                        .body()
                        .map(|body| self.format_block_doc(body, indent))
                        .unwrap_or_else(|| Doc::text("{}")),
                ];
                if let Some(catch_clause) = try_stmt.catch_clause() {
                    let mut catch_head = String::from(" catch");
                    if let Some(binding) = catch_clause.binding_token() {
                        catch_head.push_str(" (");
                        catch_head.push_str(binding.text(self.source));
                        catch_head.push(')');
                    }
                    catch_head.push(' ');
                    parts.push(Doc::text(catch_head));
                    parts.push(
                        catch_clause
                            .body()
                            .map(|body| self.format_block_doc(body, indent))
                            .unwrap_or_else(|| Doc::text("{}")),
                    );
                }
                Doc::concat(parts)
            }
            Stmt::Expr(expr_stmt) => self.format_expr_stmt(expr_stmt, indent),
        }
    }

    fn format_let_stmt(&self, let_stmt: LetStmt<'_>, indent: usize) -> Doc {
        let mut parts = vec![Doc::text("let ")];
        if let Some(name) = let_stmt.name_token() {
            parts.push(Doc::text(name.text(self.source)));
        }
        if let Some(initializer) = let_stmt.initializer() {
            parts.push(Doc::text(" = "));
            parts.push(self.format_expr_doc(initializer, indent));
        }
        parts.push(Doc::text(";"));
        Doc::concat(parts)
    }

    fn format_const_stmt(&self, const_stmt: ConstStmt<'_>, indent: usize) -> Doc {
        let mut parts = vec![Doc::text("const ")];
        if let Some(name) = const_stmt.name_token() {
            parts.push(Doc::text(name.text(self.source)));
        }
        if let Some(value) = const_stmt.value() {
            parts.push(Doc::text(" = "));
            parts.push(self.format_expr_doc(value, indent));
        }
        parts.push(Doc::text(";"));
        Doc::concat(parts)
    }

    fn format_import_stmt(&self, import_stmt: ImportStmt<'_>, indent: usize) -> Doc {
        if import_stmt
            .alias()
            .is_some_and(|alias| self.node_has_comments(alias.syntax()))
        {
            return Doc::text(self.raw(import_stmt.syntax()));
        }

        let mut parts = vec![Doc::text("import ")];
        if let Some(module) = import_stmt.module() {
            parts.push(self.format_expr_doc(module, indent));
        }
        if let Some(alias) = import_stmt.alias().and_then(|alias| self.alias_name(alias)) {
            parts.push(Doc::text(format!(" as {alias}")));
        }
        parts.push(Doc::text(";"));
        Doc::concat(parts)
    }

    fn format_export_stmt(&self, export_stmt: rhai_syntax::ExportStmt<'_>, indent: usize) -> Doc {
        if export_stmt
            .alias()
            .is_some_and(|alias| self.node_has_comments(alias.syntax()))
        {
            return Doc::text(self.raw(export_stmt.syntax()));
        }

        let mut parts = vec![Doc::text("export ")];
        if let Some(declaration) = export_stmt.declaration() {
            parts.push(self.format_stmt(declaration, indent));
        } else if let Some(target) = export_stmt.target() {
            parts.push(self.format_expr_doc(target, indent));
            if let Some(alias) = export_stmt.alias().and_then(|alias| self.alias_name(alias)) {
                parts.push(Doc::text(format!(" as {alias}")));
            }
            parts.push(Doc::text(";"));
        } else {
            parts.push(Doc::text(";"));
        }
        Doc::concat(parts)
    }

    fn format_expr_stmt(&self, expr_stmt: ExprStmt<'_>, indent: usize) -> Doc {
        let mut parts = Vec::new();
        if let Some(expr) = expr_stmt.expr() {
            parts.push(self.format_expr_doc(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            parts.push(Doc::text(";"));
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
        let leading_gap = self.comment_gap(open_brace_end, first_item_start, false);
        if items.is_empty() && leading_gap.line_comments.is_empty() {
            return Doc::text("{}");
        }

        let mut body_parts = Vec::new();
        if !leading_gap.line_comments.is_empty() {
            body_parts.push(self.render_line_comments_doc(&leading_gap.line_comments));

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
                let gap = self.comment_gap(cursor, item_start, index > 0 || !body_parts.is_empty());
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
        let trailing_gap = self.comment_gap(cursor, close_brace_start, !items.is_empty());
        if !items.is_empty()
            && (!trailing_gap.trailing_comments.is_empty()
                || !trailing_gap.line_comments.is_empty())
        {
            body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if !trailing_gap.line_comments.is_empty() {
            body_parts.push(self.render_line_comments_doc(&trailing_gap.line_comments));
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
