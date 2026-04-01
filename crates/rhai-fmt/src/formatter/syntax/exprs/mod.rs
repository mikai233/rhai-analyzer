use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};

pub(crate) mod access;
pub(crate) mod containers;
pub(crate) mod control_flow;
pub(crate) mod fallback;
pub(crate) mod operators;

impl Formatter<'_> {
    pub(crate) fn format_expr_doc(&self, expr: Expr, indent: usize) -> Doc {
        if matches!(expr_support(&expr).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(expr.syntax()));
        }

        if matches!(expr_support(&expr).level, FormatSupportLevel::Structural)
            && self.expr_requires_raw_fallback(expr.clone())
        {
            return Doc::text(self.raw(expr.syntax()));
        }

        match expr {
            Expr::Name(name) => Doc::text(
                name.token()
                    .map(|token| token.text().to_owned())
                    .unwrap_or_else(|| self.raw(name.syntax())),
            ),
            Expr::Literal(literal) => Doc::text(
                literal
                    .token()
                    .map(|token| token.text().to_owned())
                    .unwrap_or_else(|| self.raw(literal.syntax())),
            ),
            Expr::Array(array) => self.format_array_doc(array, indent),
            Expr::Object(object) => self.format_object_doc(object, indent),
            Expr::If(if_expr) => self.format_if_expr_doc(if_expr, indent),
            Expr::Switch(switch_expr) => self.format_switch_expr_doc(switch_expr, indent),
            Expr::While(while_expr) => self.format_while_expr_doc(while_expr, indent),
            Expr::Loop(loop_expr) => self.format_loop_expr_doc(loop_expr, indent),
            Expr::For(for_expr) => self.format_for_expr_doc(for_expr, indent),
            Expr::Do(do_expr) => self.format_do_expr_doc(do_expr, indent),
            Expr::Path(path) => self.format_path_doc(path, indent),
            Expr::Closure(closure) => self.format_closure_expr_doc(closure, indent),
            Expr::InterpolatedString(string) => self.format_interpolated_string_doc(string, indent),
            Expr::Unary(unary) => self.format_unary_doc(unary, indent),
            Expr::Binary(binary) => self.format_binary_doc(binary, indent),
            Expr::Assign(assign) => self.format_assign_doc(assign, indent),
            Expr::Paren(paren) => self.format_paren_doc(paren, indent),
            Expr::Call(call) => self.format_call_doc(call, indent),
            Expr::Index(index) => self.format_index_doc(index, indent),
            Expr::Field(field) => self.format_field_doc(field, indent),
            Expr::Block(block) => self.format_block_doc(block, indent),
            Expr::Error(_) => Doc::text(self.raw(expr.syntax())),
        }
    }

    pub(crate) fn expr_requires_raw_fallback(&self, expr: Expr) -> bool {
        match expr {
            Expr::If(if_expr) => self.if_requires_raw_fallback(if_expr),
            Expr::Switch(_) => false,
            Expr::While(while_expr) => self.while_requires_raw_fallback(while_expr),
            Expr::Loop(loop_expr) => self.loop_requires_raw_fallback(loop_expr),
            Expr::For(for_expr) => self.for_requires_raw_fallback(for_expr),
            Expr::Do(do_expr) => self.do_requires_raw_fallback(do_expr),
            Expr::Path(path) => self.path_requires_raw_fallback(path),
            Expr::Closure(closure) => self.closure_requires_raw_fallback(closure),
            Expr::InterpolatedString(ref string) => {
                self.node_has_unowned_comments(expr.syntax())
                    || string
                        .part_list()
                        .into_iter()
                        .flat_map(|parts| parts.parts())
                        .any(|part| match part {
                            StringPart::Segment(_) => false,
                            StringPart::Interpolation(interpolation) => interpolation
                                .body()
                                .is_some_and(|body| self.node_has_unowned_comments(body.syntax())),
                        })
            }
            Expr::Unary(unary) => self.unary_requires_raw_fallback(unary),
            Expr::Binary(binary) => self.binary_requires_raw_fallback(binary),
            Expr::Assign(assign) => self.assign_requires_raw_fallback(assign),
            Expr::Paren(paren) => self.paren_requires_raw_fallback(paren),
            Expr::Call(call) => self.call_requires_raw_fallback(call),
            Expr::Index(index) => self.index_requires_raw_fallback(index),
            Expr::Field(field) => self.field_requires_raw_fallback(field),
            Expr::Name(_)
            | Expr::Literal(_)
            | Expr::Array(_)
            | Expr::Object(_)
            | Expr::Block(_)
            | Expr::Error(_) => false,
        }
    }

    pub(crate) fn raw(&self, node: SyntaxNode) -> String {
        let start = u32::from(node.text_range().start()) as usize;
        let end = u32::from(node.text_range().end()) as usize;
        self.source.get(start..end).unwrap_or("").trim().to_owned()
    }

    pub(crate) fn doc_renders_single_line(&self, doc: &Doc, indent: usize) -> bool {
        !self.render_fragment(doc, indent).contains('\n')
    }

    pub(crate) fn collect_binary_chain(
        &self,
        expr: Expr,
        indent: usize,
        operands: &mut Vec<Doc>,
        operators: &mut Vec<String>,
    ) {
        if matches!(expr_support(&expr).level, FormatSupportLevel::RawFallback)
            || (matches!(expr_support(&expr).level, FormatSupportLevel::Structural)
                && self.expr_requires_raw_fallback(expr.clone()))
        {
            operands.push(Doc::text(self.raw(expr.syntax())));
            return;
        }

        if let Expr::Binary(binary) = expr {
            if self.binary_has_operator_comments(&binary) {
                operands.push(self.format_binary_doc(binary, indent));
                return;
            }

            if let Some(lhs) = binary.lhs() {
                self.collect_binary_chain(lhs, indent, operands, operators);
            }

            operators.push(
                binary
                    .operator_token()
                    .map(|token| token.text().to_owned())
                    .unwrap_or_default(),
            );

            if let Some(rhs) = binary.rhs() {
                self.collect_binary_chain(rhs, indent, operands, operators);
            }
        } else {
            operands.push(self.format_expr_doc(expr, indent));
        }
    }

    pub(crate) fn group_tight_suffix_doc(&self, head: Doc, suffix: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            head,
            Doc::indent(1, Doc::concat(vec![Doc::line(), suffix])),
        ]))
    }

    pub(crate) fn group_spaced_suffix_doc(&self, head: Doc, suffix_head: String, tail: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            head,
            Doc::indent(
                1,
                Doc::concat(vec![Doc::soft_line(), Doc::text(suffix_head), tail]),
            ),
        ]))
    }

    pub(crate) fn group_keyword_clause_doc(&self, keyword: &str, clause: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            Doc::text(keyword),
            Doc::indent(1, Doc::concat(vec![Doc::soft_line(), clause])),
        ]))
    }

    pub(crate) fn soft_or_tight_gap_from_gap(&self, gap: &GapTrivia) -> Doc {
        if gap.has_comments() {
            self.tight_comment_gap_from_gap(gap, true)
        } else {
            Doc::soft_line()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DelimitedItemDoc {
    element: SyntaxElement,
    range: TextRange,
    doc: Doc,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DelimitedNodeSpec<'a> {
    node: &'a SyntaxNode,
    open_kind: TokenKind,
    close_kind: TokenKind,
    open: &'a str,
    close: &'a str,
}

pub(crate) fn gap_requires_trivia_layout(gap: &GapTrivia) -> bool {
    !gap.trailing_comments.is_empty()
        || gap.has_vertical_comments()
        || gap.trailing_blank_lines_before_next > 0
}

pub(crate) fn boundary_from_element_to_token(
    element: &SyntaxElement,
    token: SyntaxToken,
) -> TriviaBoundary {
    match element {
        NodeOrToken::Node(node) => TriviaBoundary::NodeToken(node.clone(), token),
        NodeOrToken::Token(previous) => TriviaBoundary::TokenToken(previous.clone(), token),
    }
}

pub(crate) fn boundary_from_token_to_element(
    token: SyntaxToken,
    element: &SyntaxElement,
) -> TriviaBoundary {
    match element {
        NodeOrToken::Node(node) => TriviaBoundary::TokenNode(token, node.clone()),
        NodeOrToken::Token(next) => TriviaBoundary::TokenToken(token, next.clone()),
    }
}

pub(crate) fn merge_boundary_gaps(mut before: GapTrivia, after: GapTrivia) -> GapTrivia {
    before.leading_comments.extend(after.leading_comments);
    before.dangling_comments.extend(after.dangling_comments);
    before.trailing_blank_lines_before_next = before
        .trailing_blank_lines_before_next
        .max(after.trailing_blank_lines_before_next);
    before
}

pub(crate) fn hard_lines(count: usize) -> Doc {
    Doc::concat(vec![Doc::hard_line(); count])
}

pub(crate) fn range_start(range: TextRange) -> usize {
    u32::from(range.start()) as usize
}

pub(crate) fn range_end(range: TextRange) -> usize {
    u32::from(range.end()) as usize
}
