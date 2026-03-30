use rhai_syntax::{
    AstNode, BlockExpr, ConstStmt, ExprStmt, FnItem, ImportStmt, Item, LetStmt, Root, Stmt,
    TokenKind,
};

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn format_root(&self, root: Root<'_>) -> Doc {
        let items = root.items().collect::<Vec<_>>();
        let mut parts = Vec::new();
        let mut cursor = u32::from(root.syntax().range().start()) as usize;

        for (index, item) in items.iter().enumerate() {
            let item_start = u32::from(item.syntax().range().start()) as usize;
            if let Some(comments) = self.format_comment_region(cursor, item_start, 0) {
                let previous_is_fn = index > 0 && matches!(items[index - 1], Item::Fn(_));
                let current_is_fn = matches!(item, Item::Fn(_));
                if !parts.is_empty() {
                    parts.push(self.separator_doc(
                        if previous_is_fn || current_is_fn {
                            2
                        } else {
                            1
                        },
                        cursor,
                        item_start,
                    ));
                }
                parts.push(Doc::text(comments));
                parts.push(Doc::hard_line());
            } else if index > 0 {
                parts.push(self.separator_doc(
                    if matches!(items[index - 1], Item::Fn(_)) || matches!(item, Item::Fn(_)) {
                        2
                    } else {
                        1
                    },
                    cursor,
                    item_start,
                ));
            }
            parts.push(self.format_item(*item, 0));
            cursor = u32::from(item.syntax().range().end()) as usize;
        }

        let root_end = u32::from(root.syntax().range().end()) as usize;
        if let Some(comments) = self.format_comment_region(cursor, root_end, 0) {
            if !parts.is_empty() {
                parts.push(Doc::hard_line());
            }
            parts.push(Doc::text(comments));
        }

        if parts.is_empty() {
            Doc::nil()
        } else {
            parts.push(Doc::hard_line());
            Doc::concat(parts)
        }
    }

    pub(crate) fn format_item(&self, item: Item<'_>, indent: usize) -> Doc {
        match item {
            Item::Fn(function) => self.format_function(function, indent),
            Item::Stmt(stmt) => self.format_stmt(stmt, indent),
        }
    }

    fn format_function(&self, function: FnItem<'_>, indent: usize) -> Doc {
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

        signature.push_str(&self.format_params(function.params(), indent));
        signature.push(' ');

        let body = function
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));

        Doc::concat(vec![Doc::text(signature), body])
    }

    pub(crate) fn format_stmt(&self, stmt: Stmt<'_>, indent: usize) -> Doc {
        match stmt {
            Stmt::Let(let_stmt) => self.format_let_stmt(let_stmt, indent),
            Stmt::Const(const_stmt) => self.format_const_stmt(const_stmt, indent),
            Stmt::Import(import_stmt) => self.format_import_stmt(import_stmt, indent),
            Stmt::Export(export_stmt) => {
                let mut out = String::from("export ");
                if let Some(declaration) = export_stmt.declaration() {
                    out.push_str(self.format_stmt_inline(declaration, indent).trim_start());
                } else if let Some(target) = export_stmt.target() {
                    out.push_str(&self.format_expr(target, indent));
                    if let Some(alias) =
                        export_stmt.alias().and_then(|alias| self.alias_name(alias))
                    {
                        out.push_str(" as ");
                        out.push_str(alias);
                    }
                    out.push(';');
                } else {
                    out.push(';');
                }
                Doc::text(out)
            }
            Stmt::Break(break_stmt) => {
                let mut out = String::from("break");
                if let Some(value) = break_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                Doc::text(out)
            }
            Stmt::Continue(_) => Doc::text("continue;"),
            Stmt::Return(return_stmt) => {
                let mut out = String::from("return");
                if let Some(value) = return_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                Doc::text(out)
            }
            Stmt::Throw(throw_stmt) => {
                let mut out = String::from("throw");
                if let Some(value) = throw_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                Doc::text(out)
            }
            Stmt::Try(try_stmt) => {
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
                        catch_head.push(' ');
                        catch_head.push_str(binding.text(self.source));
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

    fn format_stmt_inline(&self, stmt: Stmt<'_>, indent: usize) -> String {
        self.render_fragment(&self.format_stmt(stmt, indent), indent)
    }

    fn format_let_stmt(&self, let_stmt: LetStmt<'_>, indent: usize) -> Doc {
        let mut out = String::from("let ");
        if let Some(name) = let_stmt.name_token() {
            out.push_str(name.text(self.source));
        }
        if let Some(initializer) = let_stmt.initializer() {
            out.push_str(" = ");
            out.push_str(&self.format_expr(initializer, indent));
        }
        out.push(';');
        Doc::text(out)
    }

    fn format_const_stmt(&self, const_stmt: ConstStmt<'_>, indent: usize) -> Doc {
        let mut out = String::from("const ");
        if let Some(name) = const_stmt.name_token() {
            out.push_str(name.text(self.source));
        }
        if let Some(value) = const_stmt.value() {
            out.push_str(" = ");
            out.push_str(&self.format_expr(value, indent));
        }
        out.push(';');
        Doc::text(out)
    }

    fn format_import_stmt(&self, import_stmt: ImportStmt<'_>, indent: usize) -> Doc {
        let mut out = String::from("import ");
        if let Some(module) = import_stmt.module() {
            out.push_str(&self.format_expr(module, indent));
        }
        if let Some(alias) = import_stmt.alias().and_then(|alias| self.alias_name(alias)) {
            out.push_str(" as ");
            out.push_str(alias);
        }
        out.push(';');
        Doc::text(out)
    }

    fn format_expr_stmt(&self, expr_stmt: ExprStmt<'_>, indent: usize) -> Doc {
        let mut out = String::new();
        if let Some(expr) = expr_stmt.expr() {
            out.push_str(&self.format_expr(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            out.push(';');
        }
        Doc::text(out)
    }

    pub(crate) fn format_block(&self, block: BlockExpr<'_>, indent: usize) -> String {
        self.render_fragment(&self.format_block_doc(block, indent), indent)
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
        let leading_comments = self.format_comment_region(open_brace_end, first_item_start, 0);
        if items.is_empty() && leading_comments.is_none() {
            return Doc::text("{}");
        }

        let mut body_parts = Vec::new();
        if let Some(comments) = leading_comments {
            body_parts.push(Doc::text(comments));
            body_parts.push(Doc::hard_line());
        }

        let mut cursor = first_item_start;
        for (index, item) in items.iter().enumerate() {
            let item_start = u32::from(item.syntax().range().start()) as usize;
            if let Some(comments) = self.format_comment_region(cursor, item_start, 0) {
                if index > 0 || !body_parts.is_empty() {
                    body_parts.push(self.separator_doc(1, cursor, item_start));
                }
                body_parts.push(Doc::text(comments));
                body_parts.push(Doc::hard_line());
            } else if index > 0 {
                body_parts.push(self.separator_doc(1, cursor, item_start));
            }
            body_parts.push(self.format_item(*item, indent + 1));
            cursor = u32::from(item.syntax().range().end()) as usize;
        }
        if let Some(comments) = self.format_comment_region(cursor, close_brace_start, 0) {
            body_parts.push(self.separator_doc(1, cursor, close_brace_start));
            body_parts.push(Doc::text(comments));
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
