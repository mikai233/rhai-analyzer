use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, item_support};

pub(crate) mod functions;
pub(crate) mod root;
pub(crate) mod statements;

impl Formatter<'_> {
    pub(crate) fn format_item(&self, item: Item, indent: usize) -> Doc {
        if self.is_skipped(item.syntax()) {
            return Doc::text(self.raw(item.syntax()));
        }

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

    pub(crate) fn item_requires_raw_fallback(&self, item: Item) -> bool {
        match item {
            Item::Fn(function) => self.function_requires_raw_fallback(&function),
            Item::Stmt(stmt) => self.stmt_requires_raw_fallback(stmt),
        }
    }

    pub(crate) fn stmt_requires_raw_fallback(&self, stmt: Stmt) -> bool {
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

    pub(crate) fn statement_tail_renders_single_line(&self, doc: &Doc, indent: usize) -> bool {
        !self.render_fragment(doc, indent).contains('\n')
    }

}
