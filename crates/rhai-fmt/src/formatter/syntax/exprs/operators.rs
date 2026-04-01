use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn format_unary_doc(&self, unary: rhai_syntax::UnaryExpr, indent: usize) -> Doc {
        let operator = unary
            .operator_token()
            .map(|token| token.text().to_owned())
            .unwrap_or_default();
        let inner_expr = unary.expr();
        let inner = inner_expr
            .clone()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(self.raw(unary.syntax())));

        let Some(inner_expr) = inner_expr else {
            return Doc::concat(vec![Doc::text(operator), inner]);
        };
        let Some(operator_token) = unary.operator_token() else {
            return Doc::concat(vec![Doc::text(operator), inner]);
        };

        let gap = self
            .boundary_trivia(
                unary.syntax(),
                TriviaBoundary::TokenNode(operator_token, inner_expr.syntax()),
            )
            .unwrap_or_default();
        if gap.has_comments() {
            return Doc::concat(vec![
                Doc::text(operator),
                self.tight_comment_gap_from_gap(&gap, true),
                inner,
            ]);
        }

        if self.doc_renders_single_line(&inner, indent) {
            self.group_tight_suffix_doc(Doc::text(operator), inner)
        } else {
            Doc::concat(vec![Doc::text(operator), inner])
        }
    }

    pub(crate) fn format_assign_doc(&self, assign: rhai_syntax::AssignExpr, indent: usize) -> Doc {
        let lhs_expr = assign.lhs();
        let rhs_expr = assign.rhs();
        let lhs = lhs_expr
            .clone()
            .map(|lhs| self.format_expr_doc(lhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let rhs = rhs_expr
            .clone()
            .map(|rhs| self.format_expr_doc(rhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let operator = assign
            .operator_token()
            .map(|token| token.text().to_owned())
            .unwrap_or_else(|| "=".to_owned());
        let Some(lhs_expr) = lhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(rhs_expr) = rhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(operator_token) = assign.operator_token() else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };

        let lhs_gap = self
            .boundary_trivia(
                assign.syntax(),
                TriviaBoundary::NodeToken(lhs_expr.syntax(), operator_token.clone()),
            )
            .unwrap_or_default();
        let rhs_gap = self
            .boundary_trivia(
                assign.syntax(),
                TriviaBoundary::TokenNode(operator_token.clone(), rhs_expr.syntax()),
            )
            .unwrap_or_default();

        if lhs_gap.has_comments() || rhs_gap.has_comments() {
            return Doc::concat(vec![
                lhs,
                self.space_or_tight_gap_from_gap(&lhs_gap),
                Doc::text(operator),
                self.space_or_tight_gap_from_gap(&rhs_gap),
                rhs,
            ]);
        }

        if self.doc_renders_single_line(&lhs, indent) && self.doc_renders_single_line(&rhs, indent)
        {
            self.group_spaced_suffix_doc(lhs, format!("{operator} "), rhs)
        } else {
            Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs])
        }
    }

    pub(crate) fn format_binary_doc(&self, binary: BinaryExpr, indent: usize) -> Doc {
        if self.binary_has_operator_comments(&binary) {
            return self.format_binary_with_operator_comments_doc(binary, indent);
        }

        let mut operands = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain(
            Expr::Binary(binary.clone()),
            indent,
            &mut operands,
            &mut operators,
        );

        if operands.is_empty() {
            return Doc::text(self.raw(binary.syntax()));
        }

        if operands
            .iter()
            .all(|operand| self.doc_renders_single_line(operand, indent))
        {
            let mut parts = vec![operands.remove(0)];
            for (operator, operand) in operators.into_iter().zip(operands.into_iter()) {
                parts.push(Doc::indent(
                    1,
                    Doc::concat(vec![
                        Doc::soft_line(),
                        Doc::text(format!("{operator} ")),
                        operand,
                    ]),
                ));
            }
            Doc::group(Doc::concat(parts))
        } else {
            let mut parts = vec![operands.remove(0)];
            for (operator, operand) in operators.into_iter().zip(operands.into_iter()) {
                parts.push(Doc::text(format!(" {operator} ")));
                parts.push(operand);
            }
            Doc::concat(parts)
        }
    }

    pub(crate) fn format_binary_with_operator_comments_doc(
        &self,
        binary: BinaryExpr,
        indent: usize,
    ) -> Doc {
        let lhs_expr = binary.lhs();
        let rhs_expr = binary.rhs();
        let lhs = lhs_expr
            .clone()
            .map(|lhs| self.format_expr_doc(lhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let rhs = rhs_expr
            .clone()
            .map(|rhs| self.format_expr_doc(rhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let operator = binary
            .operator_token()
            .map(|token| token.text().to_owned())
            .unwrap_or_default();
        let Some(lhs_expr) = lhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(rhs_expr) = rhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(operator_token) = binary.operator_token() else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };

        let lhs_gap = self
            .boundary_trivia(
                binary.syntax(),
                TriviaBoundary::NodeToken(lhs_expr.syntax(), operator_token.clone()),
            )
            .unwrap_or_default();
        let rhs_gap = self
            .boundary_trivia(
                binary.syntax(),
                TriviaBoundary::TokenNode(operator_token.clone(), rhs_expr.syntax()),
            )
            .unwrap_or_default();
        Doc::concat(vec![
            lhs,
            self.space_or_tight_gap_from_gap(&lhs_gap),
            Doc::text(operator),
            self.space_or_tight_gap_from_gap(&rhs_gap),
            rhs,
        ])
    }

    pub(crate) fn format_paren_doc(&self, paren: ParenExpr, indent: usize) -> Doc {
        let Some(inner_expr) = paren.expr() else {
            return Doc::concat(vec![Doc::text("("), Doc::text(")")]);
        };

        let inner = paren
            .expr()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(open_token) = paren
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::OpenParen))
        else {
            return Doc::concat(vec![Doc::text("("), inner, Doc::text(")")]);
        };
        let Some(close_token) = paren
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::CloseParen))
        else {
            return Doc::concat(vec![Doc::text("("), inner, Doc::text(")")]);
        };

        Doc::concat(vec![
            Doc::text("("),
            self.tight_comment_gap_for_boundary(
                paren.syntax(),
                TriviaBoundary::TokenNode(open_token, inner_expr.syntax()),
                true,
            ),
            inner,
            self.tight_comment_gap_for_boundary(
                paren.syntax(),
                TriviaBoundary::NodeToken(inner_expr.syntax(), close_token),
                true,
            ),
            Doc::text(")"),
        ])
    }

    pub(crate) fn binary_has_operator_comments(&self, binary: &BinaryExpr) -> bool {
        let Some(lhs) = binary.lhs() else {
            return false;
        };
        let Some(rhs) = binary.rhs() else {
            return false;
        };
        let Some(operator_token) = binary.operator_token() else {
            return false;
        };

        self.trivia
            .boundary_has_comments(&TriviaBoundary::NodeToken(
                lhs.syntax(),
                operator_token.clone(),
            ))
            || self
                .trivia
                .boundary_has_comments(&TriviaBoundary::TokenNode(operator_token, rhs.syntax()))
    }
}
