use rhai_syntax::{
    ArgList, ArrayExpr, ArrayItemList, AstNode, BinaryExpr, CallExpr, ClosureExpr,
    ClosureParamList, DoExpr, Expr, FieldExpr, ForBindings, ForExpr, IfExpr, IndexExpr,
    InterpolatedStringExpr, LoopExpr, ObjectExpr, ObjectField, ParamList, ParenExpr, PathExpr,
    StringPart, SwitchArm, SwitchExpr, SyntaxNode, TextRange, TokenKind, WhileExpr,
};

use crate::ContainerLayoutStyle;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};
use crate::formatter::trivia::comments::{GapSeparatorOptions, GapTrivia};

impl Formatter<'_> {
    pub(crate) fn format_expr_doc(&self, expr: Expr<'_>, indent: usize) -> Doc {
        if matches!(expr_support(expr).level, FormatSupportLevel::RawFallback) {
            return Doc::text(self.raw(expr.syntax()));
        }

        if matches!(expr_support(expr).level, FormatSupportLevel::Structural)
            && self.expr_requires_raw_fallback(expr)
        {
            return Doc::text(self.raw(expr.syntax()));
        }

        match expr {
            Expr::Name(name) => Doc::text(
                name.token()
                    .map(|token| token.text(self.source).to_owned())
                    .unwrap_or_else(|| self.raw(expr.syntax())),
            ),
            Expr::Literal(literal) => Doc::text(
                literal
                    .token()
                    .map(|token| token.text(self.source).to_owned())
                    .unwrap_or_else(|| self.raw(expr.syntax())),
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

    fn expr_requires_raw_fallback(&self, expr: Expr<'_>) -> bool {
        match expr {
            Expr::If(if_expr) => self.if_requires_raw_fallback(if_expr),
            Expr::Switch(_) => false,
            Expr::While(while_expr) => self.while_requires_raw_fallback(while_expr),
            Expr::Loop(loop_expr) => self.loop_requires_raw_fallback(loop_expr),
            Expr::For(for_expr) => self.for_requires_raw_fallback(for_expr),
            Expr::Do(do_expr) => self.do_requires_raw_fallback(do_expr),
            Expr::Path(path) => self.path_requires_raw_fallback(path),
            Expr::Closure(closure) => self.closure_requires_raw_fallback(closure),
            Expr::InterpolatedString(string) => {
                self.node_has_unowned_comments(expr.syntax())
                    || string.parts().any(|part| match part {
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

    fn call_requires_raw_fallback(&self, call: CallExpr<'_>) -> bool {
        let Some(callee) = call.callee() else {
            return self.node_has_unowned_comments(call.syntax());
        };
        let Some(args_open_range) = self.token_range(call.syntax(), TokenKind::OpenParen) else {
            return self.node_has_unowned_comments(call.syntax());
        };

        let callee_end = u32::from(callee.syntax().range().end()) as usize;
        let mut allowed_ranges = Vec::new();
        let args_open_start = range_start(args_open_range);
        let call_end = range_end(call.syntax().range());

        if let Some(bang_range) = self.token_range(call.syntax(), TokenKind::Bang) {
            allowed_ranges.push((callee_end, range_start(bang_range)));
            allowed_ranges.push((range_end(bang_range), args_open_start));
        } else {
            allowed_ranges.push((callee_end, args_open_start));
        }
        allowed_ranges.push((args_open_start, call_end));

        self.node_has_unowned_comments_outside(call.syntax(), &allowed_ranges)
    }

    fn unary_requires_raw_fallback(&self, unary: rhai_syntax::UnaryExpr<'_>) -> bool {
        let Some(operator_range) = unary.operator_token().map(|token| token.range()) else {
            return self.node_has_unowned_comments(unary.syntax());
        };
        let Some(inner) = unary.expr() else {
            return self.node_has_unowned_comments(unary.syntax());
        };

        self.node_has_unowned_comments_outside(
            unary.syntax(),
            &[(
                range_end(operator_range),
                range_start(inner.syntax().range()),
            )],
        )
    }

    fn binary_requires_raw_fallback(&self, binary: BinaryExpr<'_>) -> bool {
        let Some(lhs) = binary.lhs() else {
            return self.node_has_unowned_comments(binary.syntax());
        };
        let Some(rhs) = binary.rhs() else {
            return self.node_has_unowned_comments(binary.syntax());
        };
        let Some(operator_range) = binary.operator_token().map(|token| token.range()) else {
            return self.node_has_unowned_comments(binary.syntax());
        };

        self.node_has_unowned_comments_outside(
            binary.syntax(),
            &[
                (range_end(lhs.syntax().range()), range_start(operator_range)),
                (range_end(operator_range), range_start(rhs.syntax().range())),
            ],
        )
    }

    fn assign_requires_raw_fallback(&self, assign: rhai_syntax::AssignExpr<'_>) -> bool {
        let Some(lhs) = assign.lhs() else {
            return self.node_has_unowned_comments(assign.syntax());
        };
        let Some(rhs) = assign.rhs() else {
            return self.node_has_unowned_comments(assign.syntax());
        };
        let Some(operator_range) = assign.operator_token().map(|token| token.range()) else {
            return self.node_has_unowned_comments(assign.syntax());
        };

        self.node_has_unowned_comments_outside(
            assign.syntax(),
            &[
                (range_end(lhs.syntax().range()), range_start(operator_range)),
                (range_end(operator_range), range_start(rhs.syntax().range())),
            ],
        )
    }

    fn paren_requires_raw_fallback(&self, paren: ParenExpr<'_>) -> bool {
        let Some(inner) = paren.expr() else {
            return self.node_has_unowned_comments(paren.syntax());
        };
        let Some(open_range) = self.token_range(paren.syntax(), TokenKind::OpenParen) else {
            return self.node_has_unowned_comments(paren.syntax());
        };
        let Some(close_range) = self.token_range(paren.syntax(), TokenKind::CloseParen) else {
            return self.node_has_unowned_comments(paren.syntax());
        };

        self.node_has_unowned_comments_outside(
            paren.syntax(),
            &[
                (range_end(open_range), range_start(inner.syntax().range())),
                (range_end(inner.syntax().range()), range_start(close_range)),
            ],
        )
    }

    fn format_unary_doc(&self, unary: rhai_syntax::UnaryExpr<'_>, indent: usize) -> Doc {
        let operator = unary
            .operator_token()
            .map(|token| token.text(self.source))
            .unwrap_or("");
        let inner_expr = unary.expr();
        let inner = inner_expr
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(self.raw(unary.syntax())));

        let Some(inner_expr) = inner_expr else {
            return Doc::concat(vec![Doc::text(operator), inner]);
        };
        let Some(operator_range) = unary.operator_token().map(|token| token.range()) else {
            return Doc::concat(vec![Doc::text(operator), inner]);
        };

        if self.range_has_comments(
            range_end(operator_range),
            range_start(inner_expr.syntax().range()),
        ) {
            return Doc::concat(vec![
                Doc::text(operator),
                self.tight_comment_gap_doc(
                    range_end(operator_range),
                    range_start(inner_expr.syntax().range()),
                ),
                inner,
            ]);
        }

        if self.doc_renders_single_line(&inner, indent) {
            self.group_tight_suffix_doc(Doc::text(operator), inner)
        } else {
            Doc::concat(vec![Doc::text(operator), inner])
        }
    }

    fn format_path_doc(&self, path: PathExpr<'_>, indent: usize) -> Doc {
        let segments = path.segments().collect::<Vec<_>>();
        let separators = self
            .token_ranges(path.syntax(), TokenKind::ColonColon)
            .collect::<Vec<_>>();

        let (mut doc, segment_start_index, mut previous_end) = if let Some(base) = path.base() {
            if separators.len() != segments.len() {
                return Doc::text(self.raw(path.syntax()));
            }

            (
                self.format_expr_doc(base, indent),
                0,
                u32::from(base.syntax().range().end()) as usize,
            )
        } else if let Some(first) = segments.first() {
            if separators.len() + 1 != segments.len() {
                return Doc::text(self.raw(path.syntax()));
            }

            (
                Doc::text(first.text(self.source)),
                1,
                range_end(first.range()),
            )
        } else {
            return Doc::text(self.raw(path.syntax()));
        };

        for (separator_range, segment) in separators
            .into_iter()
            .zip(segments.into_iter().skip(segment_start_index))
        {
            let before_separator_has_comments =
                self.range_has_comments(previous_end, range_start(separator_range));
            let after_separator_has_comments =
                self.range_has_comments(range_end(separator_range), range_start(segment.range()));

            if before_separator_has_comments || after_separator_has_comments {
                doc = Doc::concat(vec![
                    doc,
                    self.tight_comment_gap_doc(previous_end, range_start(separator_range)),
                    Doc::text("::"),
                    self.tight_comment_gap_doc(
                        range_end(separator_range),
                        range_start(segment.range()),
                    ),
                    Doc::text(segment.text(self.source)),
                ]);
            } else {
                doc = self.group_tight_suffix_doc(
                    doc,
                    Doc::text(format!("::{}", segment.text(self.source))),
                );
            }

            previous_end = range_end(segment.range());
        }

        doc
    }

    fn format_assign_doc(&self, assign: rhai_syntax::AssignExpr<'_>, indent: usize) -> Doc {
        let lhs_expr = assign.lhs();
        let rhs_expr = assign.rhs();
        let lhs = lhs_expr
            .map(|lhs| self.format_expr_doc(lhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let rhs = rhs_expr
            .map(|rhs| self.format_expr_doc(rhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let operator = assign
            .operator_token()
            .map(|token| token.text(self.source))
            .unwrap_or("=");
        let Some(lhs_expr) = lhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(rhs_expr) = rhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(operator_range) = assign.operator_token().map(|token| token.range()) else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };

        let before_operator_has_comments = self.range_has_comments(
            range_end(lhs_expr.syntax().range()),
            range_start(operator_range),
        );
        let after_operator_has_comments = self.range_has_comments(
            range_end(operator_range),
            range_start(rhs_expr.syntax().range()),
        );

        if before_operator_has_comments || after_operator_has_comments {
            return Doc::concat(vec![
                lhs,
                self.space_or_tight_gap_doc(
                    range_end(lhs_expr.syntax().range()),
                    range_start(operator_range),
                ),
                Doc::text(operator),
                self.space_or_tight_gap_doc(
                    range_end(operator_range),
                    range_start(rhs_expr.syntax().range()),
                ),
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

    fn format_binary_doc(&self, binary: BinaryExpr<'_>, indent: usize) -> Doc {
        if self.binary_has_operator_comments(binary) {
            return self.format_binary_with_operator_comments_doc(binary, indent);
        }

        let mut operands = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain(Expr::Binary(binary), indent, &mut operands, &mut operators);

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

    fn format_binary_with_operator_comments_doc(
        &self,
        binary: BinaryExpr<'_>,
        indent: usize,
    ) -> Doc {
        let lhs_expr = binary.lhs();
        let rhs_expr = binary.rhs();
        let lhs = lhs_expr
            .map(|lhs| self.format_expr_doc(lhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let rhs = rhs_expr
            .map(|rhs| self.format_expr_doc(rhs, indent))
            .unwrap_or_else(|| Doc::text(""));
        let operator = binary
            .operator_token()
            .map(|token| token.text(self.source))
            .unwrap_or("");
        let Some(lhs_expr) = lhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(rhs_expr) = rhs_expr else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };
        let Some(operator_range) = binary.operator_token().map(|token| token.range()) else {
            return Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs]);
        };

        Doc::concat(vec![
            lhs,
            self.space_or_tight_gap_doc(
                range_end(lhs_expr.syntax().range()),
                range_start(operator_range),
            ),
            Doc::text(operator),
            self.space_or_tight_gap_doc(
                range_end(operator_range),
                range_start(rhs_expr.syntax().range()),
            ),
            rhs,
        ])
    }

    fn format_paren_doc(&self, paren: ParenExpr<'_>, indent: usize) -> Doc {
        let Some(inner_expr) = paren.expr() else {
            return Doc::concat(vec![Doc::text("("), Doc::text(")")]);
        };

        let inner = paren
            .expr()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(open_range) = self.token_range(paren.syntax(), TokenKind::OpenParen) else {
            return Doc::concat(vec![Doc::text("("), inner, Doc::text(")")]);
        };
        let Some(close_range) = self.token_range(paren.syntax(), TokenKind::CloseParen) else {
            return Doc::concat(vec![Doc::text("("), inner, Doc::text(")")]);
        };

        Doc::concat(vec![
            Doc::text("("),
            self.tight_comment_gap_doc(
                range_end(open_range),
                range_start(inner_expr.syntax().range()),
            ),
            inner,
            self.tight_comment_gap_doc(
                range_end(inner_expr.syntax().range()),
                range_start(close_range),
            ),
            Doc::text(")"),
        ])
    }

    fn format_call_doc(&self, call: CallExpr<'_>, indent: usize) -> Doc {
        let callee = call
            .callee()
            .map(|callee| self.format_expr_doc(callee, indent))
            .unwrap_or_else(|| Doc::text(""));
        let args = self.format_arg_list_doc(call, indent);
        let Some(callee_expr) = call.callee() else {
            return Doc::concat(vec![callee, args]);
        };
        let Some(args_open_range) = self.token_range(call.syntax(), TokenKind::OpenParen) else {
            return Doc::concat(vec![callee, args]);
        };

        let callee_end = u32::from(callee_expr.syntax().range().end()) as usize;
        if let Some(bang_range) = self.token_range(call.syntax(), TokenKind::Bang) {
            Doc::concat(vec![
                callee,
                self.tight_comment_gap_doc(callee_end, range_start(bang_range)),
                Doc::text("!"),
                self.tight_comment_gap_doc(range_end(bang_range), range_start(args_open_range)),
                args,
            ])
        } else {
            Doc::concat(vec![
                callee,
                self.tight_comment_gap_doc(callee_end, range_start(args_open_range)),
                args,
            ])
        }
    }

    fn format_arg_list_doc(&self, call: CallExpr<'_>, indent: usize) -> Doc {
        let values = call
            .args()
            .map(|args| self.arg_list_items(args, indent))
            .unwrap_or_default();
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: call.syntax(),
                open_kind: TokenKind::OpenParen,
                close_kind: TokenKind::CloseParen,
                open: "(",
                close: ")",
            },
            values,
            None,
        )
    }

    pub(crate) fn format_arg_list_body_doc(&self, args: ArgList<'_>, indent: usize) -> Doc {
        let items = self.arg_list_items(args, indent);
        self.format_comma_separated_body_doc(args.syntax(), items)
    }

    pub(crate) fn format_array_item_list_body_doc(
        &self,
        items: ArrayItemList<'_>,
        indent: usize,
    ) -> Doc {
        let item_docs = self.array_item_list_items(items, indent);
        self.format_comma_separated_body_doc(items.syntax(), item_docs)
    }

    fn format_array_doc(&self, array: ArrayExpr<'_>, indent: usize) -> Doc {
        let items = array
            .items()
            .map(|items| self.array_item_list_items(items, indent))
            .unwrap_or_default();
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: array.syntax(),
                open_kind: TokenKind::OpenBracket,
                close_kind: TokenKind::CloseBracket,
                open: "[",
                close: "]",
            },
            items,
            None,
        )
    }

    fn format_index_doc(&self, index: IndexExpr<'_>, indent: usize) -> Doc {
        let receiver_expr = index.receiver();
        let receiver = receiver_expr
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let inner_expr = index.index();
        let inner = inner_expr
            .map(|inner| self.format_expr_doc(inner, indent))
            .unwrap_or_else(|| Doc::text(""));
        let open_range = self
            .token_range(index.syntax(), TokenKind::QuestionOpenBracket)
            .or_else(|| self.token_range(index.syntax(), TokenKind::OpenBracket));
        let close_range = self.token_range(index.syntax(), TokenKind::CloseBracket);
        let open = if self
            .token_range(index.syntax(), TokenKind::QuestionOpenBracket)
            .is_some()
        {
            "?["
        } else {
            "["
        };
        let suffix = Doc::group(Doc::concat(vec![
            Doc::text(open),
            Doc::indent(1, Doc::concat(vec![Doc::line(), inner.clone()])),
            Doc::line(),
            Doc::text("]"),
        ]));

        let Some(receiver_expr) = receiver_expr else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };
        let Some(inner_expr) = inner_expr else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };
        let Some(open_range) = open_range else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };
        let Some(close_range) = close_range else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };

        let receiver_end = u32::from(receiver_expr.syntax().range().end()) as usize;
        let open_end = range_end(open_range);
        let inner_start = range_start(inner_expr.syntax().range());
        let inner_end = range_end(inner_expr.syntax().range());
        let close_start = range_start(close_range);

        if self.range_has_comments(receiver_end, range_start(open_range))
            || self.range_has_comments(open_end, inner_start)
            || self.range_has_comments(inner_end, close_start)
        {
            Doc::concat(vec![
                receiver,
                self.tight_comment_gap_doc(receiver_end, range_start(open_range)),
                Doc::text(open),
                self.tight_comment_gap_doc(open_end, inner_start),
                inner,
                self.tight_comment_gap_doc(inner_end, close_start),
                Doc::text("]"),
            ])
        } else {
            self.group_tight_suffix_doc(receiver, suffix)
        }
    }

    fn format_field_doc(&self, field: FieldExpr<'_>, indent: usize) -> Doc {
        let receiver_expr = field.receiver();
        let receiver = receiver_expr
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let name = field
            .name_token()
            .map(|name| name.text(self.source).to_owned())
            .unwrap_or_default();
        let accessor_range = self
            .token_range(field.syntax(), TokenKind::QuestionDot)
            .or_else(|| self.token_range(field.syntax(), TokenKind::Dot));
        let accessor = if self
            .token_range(field.syntax(), TokenKind::QuestionDot)
            .is_some()
        {
            "?."
        } else {
            "."
        };

        let Some(receiver_expr) = receiver_expr else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };
        let Some(accessor_range) = accessor_range else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };
        let Some(name_range) = field.name_token().map(|token| token.range()) else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };

        let receiver_end = u32::from(receiver_expr.syntax().range().end()) as usize;
        let before_accessor_has_comments =
            self.range_has_comments(receiver_end, range_start(accessor_range));
        let after_accessor_has_comments =
            self.range_has_comments(range_end(accessor_range), range_start(name_range));

        if before_accessor_has_comments || after_accessor_has_comments {
            Doc::concat(vec![
                receiver,
                self.tight_comment_gap_doc(receiver_end, range_start(accessor_range)),
                Doc::text(accessor),
                self.tight_comment_gap_doc(range_end(accessor_range), range_start(name_range)),
                Doc::text(name),
            ])
        } else {
            self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")))
        }
    }

    fn binary_has_operator_comments(&self, binary: BinaryExpr<'_>) -> bool {
        let Some(lhs) = binary.lhs() else {
            return false;
        };
        let Some(rhs) = binary.rhs() else {
            return false;
        };
        let Some(operator_range) = binary.operator_token().map(|token| token.range()) else {
            return false;
        };

        self.range_has_comments(range_end(lhs.syntax().range()), range_start(operator_range))
            || self.range_has_comments(range_end(operator_range), range_start(rhs.syntax().range()))
    }

    fn format_object_doc(&self, object: ObjectExpr<'_>, indent: usize) -> Doc {
        let fields = object
            .fields()
            .map(|field| DelimitedItemDoc {
                range: field.syntax().range(),
                doc: self.format_object_field_doc(field, indent + 1),
            })
            .collect::<Vec<_>>();
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: object.syntax(),
                open_kind: TokenKind::HashBraceOpen,
                close_kind: TokenKind::CloseBrace,
                open: "#{",
                close: "}",
            },
            fields,
            Some(60),
        )
    }

    fn format_object_field_doc(&self, field: ObjectField<'_>, indent: usize) -> Doc {
        if self.object_field_requires_raw_fallback(field) {
            return Doc::text(self.raw(field.syntax()));
        }

        let name_token = field.name_token();
        let name = field
            .name_token()
            .map(|token| token.text(self.source).to_owned())
            .unwrap_or_default();
        let value_expr = field.value();
        let value = value_expr
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(name_range) = name_token.map(|token| token.range()) else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(colon_range) = self.token_range(field.syntax(), TokenKind::Colon) else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(value_expr) = value_expr else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };

        if self.range_has_comments(range_end(name_range), range_start(colon_range))
            || self.range_has_comments(
                range_end(colon_range),
                range_start(value_expr.syntax().range()),
            )
        {
            return Doc::concat(vec![
                Doc::text(name),
                self.tight_comment_gap_doc_without_trailing_space(
                    range_end(name_range),
                    range_start(colon_range),
                ),
                Doc::text(":"),
                self.space_or_tight_gap_doc(
                    range_end(colon_range),
                    range_start(value_expr.syntax().range()),
                ),
                value,
            ]);
        }

        Doc::concat(vec![Doc::text(format!("{name}: ")), value])
    }

    fn format_if_expr_doc(&self, if_expr: IfExpr<'_>, indent: usize) -> Doc {
        let condition_expr = if_expr.condition();
        let condition = condition_expr
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let then_expr = if_expr.then_branch();
        let then_branch = then_expr
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let condition_end = condition_expr
            .map(|expr| u32::from(expr.syntax().range().end()) as usize)
            .unwrap_or_else(|| {
                self.token_range(if_expr.syntax(), TokenKind::IfKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(if_expr.syntax().range().start()) as usize)
            });
        let then_start = then_expr
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(if_expr.syntax().range().end()) as usize);
        let mut parts = vec![
            Doc::text("if "),
            condition,
            self.head_body_separator_doc(condition_end, then_start),
            then_branch,
        ];

        if let Some(else_branch) = if_expr.else_branch() {
            let else_start = self
                .token_range(else_branch.syntax(), TokenKind::ElseKw)
                .map(range_start)
                .unwrap_or_else(|| u32::from(else_branch.syntax().range().start()) as usize);
            let then_end = then_expr
                .map(|body| u32::from(body.syntax().range().end()) as usize)
                .unwrap_or(else_start);
            parts.push(self.inline_or_gap_separator_doc(
                then_end,
                else_start,
                GapSeparatorOptions {
                    inline_text: " ",
                    minimum_newlines: 1,
                    has_previous: true,
                    has_next: true,
                    include_terminal_newline: true,
                },
            ));
            parts.push(Doc::text("else"));

            let else_body = else_branch.body();
            let else_body_start = else_body
                .map(|body| u32::from(body.syntax().range().start()) as usize)
                .unwrap_or_else(|| u32::from(else_branch.syntax().range().end()) as usize);
            let else_kw_end = self
                .token_range(else_branch.syntax(), TokenKind::ElseKw)
                .map(range_end)
                .unwrap_or(else_body_start);
            parts.push(self.head_body_separator_doc(else_kw_end, else_body_start));
            parts.push(match else_body {
                Some(Expr::If(nested_if)) => self.format_if_expr_doc(nested_if, indent),
                Some(Expr::Block(block)) => self.format_block_doc(block, indent),
                Some(other) => self.format_expr_doc(other, indent),
                None => Doc::text("{}"),
            });
        }

        Doc::concat(parts)
    }

    fn format_switch_expr_doc(&self, switch_expr: SwitchExpr<'_>, indent: usize) -> Doc {
        let scrutinee = switch_expr
            .scrutinee()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let arms = switch_expr.arms().collect::<Vec<_>>();
        let open_brace_end = self
            .token_range(switch_expr.syntax(), TokenKind::OpenBrace)
            .map(|range| u32::from(range.end()) as usize)
            .unwrap_or_else(|| u32::from(switch_expr.syntax().range().start()) as usize);
        let close_brace_start = self
            .token_range(switch_expr.syntax(), TokenKind::CloseBrace)
            .map(|range| u32::from(range.start()) as usize)
            .unwrap_or_else(|| u32::from(switch_expr.syntax().range().end()) as usize);
        let first_arm_start = arms
            .first()
            .map(|arm| u32::from(arm.syntax().range().start()) as usize)
            .unwrap_or(close_brace_start);
        let leading_gap =
            self.comment_gap(open_brace_end, first_arm_start, false, !arms.is_empty());

        if arms.is_empty() && !leading_gap.has_vertical_comments() {
            return Doc::concat(vec![Doc::text("switch "), scrutinee, Doc::text(" {}")]);
        }

        let mut body_parts = Vec::new();
        if leading_gap.has_vertical_comments() {
            body_parts.push(self.render_line_comments_doc(leading_gap.vertical_comments()));

            let suffix_newlines = if arms.is_empty() {
                leading_gap.trailing_blank_lines_before_next
            } else {
                leading_gap.trailing_blank_lines_before_next + 1
            };
            if suffix_newlines > 0 {
                body_parts.push(Doc::concat(vec![Doc::hard_line(); suffix_newlines]));
            }
        }

        let mut cursor = first_arm_start;
        for (index, arm) in arms.iter().enumerate() {
            let arm_start = u32::from(arm.syntax().range().start()) as usize;
            let has_leading_content = !body_parts.is_empty();
            let skip_separator = index == 0 && has_leading_content && arm_start == cursor;

            if !skip_separator {
                let gap =
                    self.comment_gap(cursor, arm_start, index > 0 || !body_parts.is_empty(), true);
                body_parts.push(self.gap_separator_doc(
                    &gap,
                    1,
                    index > 0 || !body_parts.is_empty(),
                    true,
                ));
            }

            body_parts.push(self.format_switch_arm_doc(*arm, indent + 1));
            if index + 1 < arms.len() {
                body_parts.push(Doc::text(","));
            }
            cursor = u32::from(arm.syntax().range().end()) as usize;
        }

        let trailing_gap = self.comment_gap(cursor, close_brace_start, !arms.is_empty(), false);
        if !arms.is_empty() && trailing_gap.has_comments() {
            body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if trailing_gap.has_vertical_comments() {
            body_parts.push(self.render_line_comments_doc(trailing_gap.vertical_comments()));
        }

        Doc::concat(vec![
            Doc::text("switch "),
            scrutinee,
            Doc::text(" {"),
            Doc::indent(
                1,
                Doc::concat(vec![Doc::hard_line(), Doc::concat(body_parts)]),
            ),
            Doc::hard_line(),
            Doc::text("}"),
        ])
    }

    fn format_switch_arm_doc(&self, arm: SwitchArm<'_>, indent: usize) -> Doc {
        if self.switch_arm_requires_raw_fallback(arm) {
            return Doc::text(self.raw(arm.syntax()));
        }

        let patterns_node = arm.patterns();
        let patterns = patterns_node
            .map(|patterns| self.format_switch_patterns_doc(patterns, indent))
            .unwrap_or_else(|| Doc::text("_"));
        let value_expr = arm.value();
        let value = value_expr
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(patterns_node) = patterns_node else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };
        let Some(value_expr) = value_expr else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };
        let Some(arrow_range) = self.token_range(arm.syntax(), TokenKind::FatArrow) else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };

        if self.range_has_comments(
            range_end(patterns_node.syntax().range()),
            range_start(arrow_range),
        ) || self.range_has_comments(
            range_end(arrow_range),
            range_start(value_expr.syntax().range()),
        ) {
            return Doc::concat(vec![
                patterns,
                self.space_or_tight_gap_doc(
                    range_end(patterns_node.syntax().range()),
                    range_start(arrow_range),
                ),
                Doc::text("=>"),
                self.space_or_tight_gap_doc(
                    range_end(arrow_range),
                    range_start(value_expr.syntax().range()),
                ),
                value,
            ]);
        }

        if self.doc_renders_single_line(&patterns, indent)
            && self.doc_renders_single_line(&value, indent)
        {
            self.group_spaced_suffix_doc(patterns, "=> ".to_owned(), value)
        } else {
            Doc::concat(vec![patterns, Doc::text(" => "), value])
        }
    }

    fn format_while_expr_doc(&self, while_expr: WhileExpr<'_>, indent: usize) -> Doc {
        let condition_expr = while_expr.condition();
        let condition = condition_expr
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body_expr = while_expr.body();
        let body = body_expr
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let condition_end = condition_expr
            .map(|expr| u32::from(expr.syntax().range().end()) as usize)
            .unwrap_or_else(|| {
                self.token_range(while_expr.syntax(), TokenKind::WhileKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(while_expr.syntax().range().start()) as usize)
            });
        let body_start = body_expr
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(while_expr.syntax().range().end()) as usize);
        Doc::concat(vec![
            Doc::text("while "),
            condition,
            self.head_body_separator_doc(condition_end, body_start),
            body,
        ])
    }

    fn format_loop_expr_doc(&self, loop_expr: LoopExpr<'_>, indent: usize) -> Doc {
        let body_expr = loop_expr.body();
        let body = body_expr
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let head_end = self
            .token_range(loop_expr.syntax(), TokenKind::LoopKw)
            .map(range_end)
            .unwrap_or_else(|| u32::from(loop_expr.syntax().range().start()) as usize);
        let body_start = body_expr
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(loop_expr.syntax().range().end()) as usize);
        Doc::concat(vec![
            Doc::text("loop"),
            self.head_body_separator_doc(head_end, body_start),
            body,
        ])
    }

    fn format_for_expr_doc(&self, for_expr: ForExpr<'_>, indent: usize) -> Doc {
        let bindings = self.format_for_bindings_doc(for_expr.bindings());
        let iterable_expr = for_expr.iterable();
        let iterable = iterable_expr
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body_expr = for_expr.body();
        let body = body_expr
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let iterable_end = iterable_expr
            .map(|expr| u32::from(expr.syntax().range().end()) as usize)
            .unwrap_or_else(|| u32::from(for_expr.syntax().range().start()) as usize);
        let body_start = body_expr
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(for_expr.syntax().range().end()) as usize);
        Doc::concat(vec![
            Doc::text("for "),
            bindings,
            Doc::text(" in "),
            iterable,
            self.head_body_separator_doc(iterable_end, body_start),
            body,
        ])
    }

    fn format_for_bindings_doc(&self, bindings: Option<ForBindings<'_>>) -> Doc {
        let Some(bindings) = bindings else {
            return Doc::text("_");
        };
        let names = bindings.names().collect::<Vec<_>>();
        match names.as_slice() {
            [] => Doc::text("_"),
            [name] => Doc::text(name.text(self.source)),
            [first, second] => {
                let Some(open_range) = self.token_range(bindings.syntax(), TokenKind::OpenParen)
                else {
                    return Doc::text(self.raw(bindings.syntax()));
                };
                let Some(comma_range) = self.token_range(bindings.syntax(), TokenKind::Comma)
                else {
                    return Doc::text(self.raw(bindings.syntax()));
                };
                let Some(close_range) = self.token_range(bindings.syntax(), TokenKind::CloseParen)
                else {
                    return Doc::text(self.raw(bindings.syntax()));
                };

                if self.range_has_comments(range_end(open_range), range_start(first.range()))
                    || self.range_has_comments(range_end(first.range()), range_start(comma_range))
                    || self.range_has_comments(range_end(comma_range), range_start(second.range()))
                    || self.range_has_comments(range_end(second.range()), range_start(close_range))
                {
                    Doc::concat(vec![
                        Doc::text("("),
                        self.tight_comment_gap_doc(
                            range_end(open_range),
                            range_start(first.range()),
                        ),
                        Doc::text(first.text(self.source)),
                        self.tight_comment_gap_doc_without_trailing_space(
                            range_end(first.range()),
                            range_start(comma_range),
                        ),
                        Doc::text(","),
                        self.space_or_tight_gap_doc(
                            range_end(comma_range),
                            range_start(second.range()),
                        ),
                        Doc::text(second.text(self.source)),
                        self.tight_comment_gap_doc(
                            range_end(second.range()),
                            range_start(close_range),
                        ),
                        Doc::text(")"),
                    ])
                } else {
                    Doc::text(format!(
                        "({}, {})",
                        first.text(self.source),
                        second.text(self.source)
                    ))
                }
            }
            _ => Doc::text(self.raw(bindings.syntax())),
        }
    }

    fn format_do_expr_doc(&self, do_expr: DoExpr<'_>, indent: usize) -> Doc {
        let body_expr = do_expr.body();
        let body = body_expr
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let condition = do_expr.condition();
        let keyword = condition
            .and_then(|condition| condition.keyword_token())
            .map(|token| token.text(self.source))
            .unwrap_or("while");
        let expr = condition
            .and_then(|condition| condition.expr())
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let do_kw_end = self
            .token_range(do_expr.syntax(), TokenKind::DoKw)
            .map(range_end)
            .unwrap_or_else(|| u32::from(do_expr.syntax().range().start()) as usize);
        let body_start = body_expr
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(do_expr.syntax().range().end()) as usize);
        let body_end = body_expr
            .map(|body| u32::from(body.syntax().range().end()) as usize)
            .unwrap_or(body_start);
        let condition_start = condition
            .map(|condition| u32::from(condition.syntax().range().start()) as usize)
            .unwrap_or(body_end);
        let condition_kw_end = condition
            .and_then(|condition| condition.keyword_token())
            .map(|token| u32::from(token.range().end()) as usize)
            .unwrap_or(condition_start);
        let expr_start = condition
            .and_then(|condition| condition.expr())
            .map(|expr| u32::from(expr.syntax().range().start()) as usize)
            .unwrap_or(condition_kw_end);
        Doc::concat(vec![
            Doc::text("do"),
            self.head_body_separator_doc(do_kw_end, body_start),
            body,
            self.inline_or_gap_separator_doc(
                body_end,
                condition_start,
                GapSeparatorOptions {
                    inline_text: " ",
                    minimum_newlines: 1,
                    has_previous: true,
                    has_next: true,
                    include_terminal_newline: true,
                },
            ),
            Doc::text(keyword),
            self.head_body_separator_doc(condition_kw_end, expr_start),
            expr,
        ])
    }

    fn format_closure_expr_doc(&self, closure: ClosureExpr<'_>, indent: usize) -> Doc {
        let params_node = closure.params();
        let params = self.format_closure_params_doc(params_node);
        let body_expr = closure.body();
        let body = body_expr
            .map(|body| self.format_expr_doc(body, indent))
            .unwrap_or_else(|| Doc::text(""));
        let separator = match (params_node, body_expr) {
            (Some(params), Some(body)) => self.space_or_tight_gap_doc(
                range_end(params.syntax().range()),
                range_start(body.syntax().range()),
            ),
            _ => Doc::text(" "),
        };

        Doc::concat(vec![params, separator, body])
    }

    fn format_interpolated_string_doc(
        &self,
        string: InterpolatedStringExpr<'_>,
        indent: usize,
    ) -> Doc {
        let mut parts = vec![Doc::text("`")];

        for part in string.parts() {
            match part {
                StringPart::Segment(segment) => {
                    if let Some(token) = segment.text_token() {
                        parts.push(Doc::text(token.text(self.source)));
                    }
                }
                StringPart::Interpolation(interpolation) => {
                    let body = interpolation
                        .body()
                        .map(|body| self.format_interpolation_body_doc(body, indent))
                        .unwrap_or_else(Doc::nil);
                    parts.push(Doc::concat(vec![Doc::text("${"), body, Doc::text("}")]));
                }
            }
        }

        parts.push(Doc::text("`"));
        Doc::concat(parts)
    }

    pub(crate) fn format_closure_params_doc(&self, params: Option<ClosureParamList<'_>>) -> Doc {
        let Some(params) = params else {
            return Doc::text("||");
        };
        if self
            .token_range(params.syntax(), TokenKind::PipePipe)
            .is_some()
        {
            return Doc::text("||");
        }

        let names = params.params().collect::<Vec<_>>();
        let Some(open_range) = self.token_ranges(params.syntax(), TokenKind::Pipe).next() else {
            return Doc::text(self.raw(params.syntax()));
        };
        let Some(close_range) = self.token_ranges(params.syntax(), TokenKind::Pipe).last() else {
            return Doc::text(self.raw(params.syntax()));
        };

        if names.is_empty() {
            if self.range_has_comments(range_end(open_range), range_start(close_range)) {
                return Doc::concat(vec![
                    Doc::text("|"),
                    self.tight_comment_gap_doc(range_end(open_range), range_start(close_range)),
                    Doc::text("|"),
                ]);
            }

            return Doc::text("||");
        }

        let commas = self
            .token_ranges(params.syntax(), TokenKind::Comma)
            .collect::<Vec<_>>();
        if commas.len() + 1 != names.len() {
            return Doc::text(self.raw(params.syntax()));
        }

        let mut parts = vec![Doc::text("|")];
        parts
            .push(self.tight_comment_gap_doc(range_end(open_range), range_start(names[0].range())));
        parts.push(Doc::text(names[0].text(self.source)));
        let mut previous_end = range_end(names[0].range());

        for (comma_range, next_name) in commas.into_iter().zip(names.iter().skip(1)) {
            parts.push(self.tight_comment_gap_doc_without_trailing_space(
                previous_end,
                range_start(comma_range),
            ));
            parts.push(Doc::text(","));
            parts.push(
                self.space_or_tight_gap_doc(range_end(comma_range), range_start(next_name.range())),
            );
            parts.push(Doc::text(next_name.text(self.source)));
            previous_end = range_end(next_name.range());
        }

        let last_name = names.last().copied().expect("non-empty closure params");
        parts.push(
            self.tight_comment_gap_doc(range_end(last_name.range()), range_start(close_range)),
        );
        parts.push(Doc::text("|"));

        Doc::concat(parts)
    }

    fn format_interpolation_body_doc(
        &self,
        body: rhai_syntax::InterpolationBody<'_>,
        indent: usize,
    ) -> Doc {
        if self.node_has_unowned_comments(body.syntax()) {
            return Doc::text(self.raw(body.syntax()));
        }

        let items = body.items().collect::<Vec<_>>();
        if items.is_empty() {
            return Doc::nil();
        }

        let item_docs = items
            .iter()
            .map(|item| self.format_item(*item, indent))
            .collect::<Vec<_>>();
        let inline_fragments = item_docs
            .iter()
            .map(|item| self.render_fragment(item, 0))
            .collect::<Vec<_>>();
        let inline = inline_fragments.join(" ");
        let should_inline = inline_fragments.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= self.options.max_line_length.saturating_sub(3);
        if should_inline {
            return Doc::text(inline);
        }

        let mut parts = vec![Doc::hard_line()];
        for (index, item) in item_docs.into_iter().enumerate() {
            if index > 0 {
                parts.push(Doc::hard_line());
            }
            parts.push(item);
        }
        parts.push(Doc::hard_line());

        Doc::indent(1, Doc::concat(parts))
    }

    fn format_delimited_doc_with_limit(
        &self,
        open: &str,
        close: &str,
        items: Vec<Doc>,
        inline_limit: Option<usize>,
    ) -> Doc {
        if items.is_empty() {
            return Doc::text(format!("{open}{close}"));
        }

        let inline_items = items
            .iter()
            .map(|item| self.render_fragment(item, 0))
            .collect::<Vec<_>>();
        let inline = format!("{open}{}{close}", inline_items.join(", "));
        let max_inline_width = match self.options.container_layout {
            ContainerLayoutStyle::Auto | ContainerLayoutStyle::PreferMultiLine => {
                inline_limit.unwrap_or(self.options.max_line_length)
            }
            ContainerLayoutStyle::PreferSingleLine => self.options.max_line_length,
        };
        let should_inline =
            self.should_inline_delimited_items(&inline_items, &inline, max_inline_width);
        if should_inline {
            return Doc::text(inline);
        }

        let mut item_docs = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                item_docs.push(Doc::text(","));
                item_docs.push(Doc::soft_line());
            }
            item_docs.push(item);
        }

        let mut parts = vec![
            Doc::text(open),
            Doc::indent(
                1,
                Doc::concat(vec![Doc::soft_line(), Doc::concat(item_docs)]),
            ),
        ];
        if self.options.trailing_commas {
            parts.push(Doc::text(","));
        }
        parts.push(Doc::soft_line());
        parts.push(Doc::text(close));

        if inline_limit.is_some()
            || matches!(
                self.options.container_layout,
                ContainerLayoutStyle::PreferMultiLine
            )
        {
            Doc::concat(parts)
        } else {
            Doc::group(Doc::concat(parts))
        }
    }

    fn should_inline_delimited_items(
        &self,
        inline_items: &[String],
        inline: &str,
        max_inline_width: usize,
    ) -> bool {
        if matches!(
            self.options.container_layout,
            ContainerLayoutStyle::PreferMultiLine
        ) {
            return false;
        }

        inline_items.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= max_inline_width
    }

    pub(crate) fn format_params_doc(&self, params: Option<ParamList<'_>>, _indent: usize) -> Doc {
        let names = params
            .map(|params| {
                params
                    .params()
                    .map(|param| DelimitedItemDoc {
                        range: param.range(),
                        doc: Doc::text(param.text(self.source)),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match params {
            Some(params) => self.format_delimited_node_doc(
                DelimitedNodeSpec {
                    node: params.syntax(),
                    open_kind: TokenKind::OpenParen,
                    close_kind: TokenKind::CloseParen,
                    open: "(",
                    close: ")",
                },
                names,
                None,
            ),
            None => Doc::text("()"),
        }
    }

    fn arg_list_items(&self, args: ArgList<'_>, indent: usize) -> Vec<DelimitedItemDoc> {
        args.args()
            .map(|expr| DelimitedItemDoc {
                range: expr.syntax().range(),
                doc: self.format_expr_doc(expr, indent),
            })
            .collect::<Vec<_>>()
    }

    fn array_item_list_items(
        &self,
        items: ArrayItemList<'_>,
        indent: usize,
    ) -> Vec<DelimitedItemDoc> {
        items
            .exprs()
            .map(|expr| DelimitedItemDoc {
                range: expr.syntax().range(),
                doc: self.format_expr_doc(expr, indent + 1),
            })
            .collect::<Vec<_>>()
    }

    fn format_delimited_node_doc(
        &self,
        spec: DelimitedNodeSpec<'_>,
        items: Vec<DelimitedItemDoc>,
        inline_limit: Option<usize>,
    ) -> Doc {
        let open_end = self
            .token_range(spec.node, spec.open_kind)
            .map(range_end)
            .unwrap_or_else(|| range_start(spec.node.range()));
        let close_start = self
            .token_range(spec.node, spec.close_kind)
            .map(range_start)
            .unwrap_or_else(|| range_end(spec.node.range()));

        if items.is_empty() {
            let gap = self.comment_gap(open_end, close_start, false, false);
            if !gap_requires_trivia_layout(&gap) {
                return Doc::text(format!("{}{}", spec.open, spec.close));
            }

            return Doc::concat(vec![
                Doc::text(spec.open),
                Doc::indent(1, self.leading_delimited_gap_doc(&gap, false)),
                Doc::hard_line(),
                Doc::text(spec.close),
            ]);
        }

        let leading_gap = self.comment_gap(open_end, range_start(items[0].range), false, true);
        let mut requires_trivia_layout = gap_requires_trivia_layout(&leading_gap);
        let mut cursor = range_end(items[0].range);

        for item in items.iter().skip(1) {
            let gap = self.comment_gap(cursor, range_start(item.range), true, true);
            requires_trivia_layout |= gap_requires_trivia_layout(&gap);
            cursor = range_end(item.range);
        }

        let trailing_gap = self.comment_gap(cursor, close_start, true, false);
        requires_trivia_layout |= gap_requires_trivia_layout(&trailing_gap);

        if !requires_trivia_layout {
            let docs = items.into_iter().map(|item| item.doc).collect::<Vec<_>>();
            return self.format_delimited_doc_with_limit(spec.open, spec.close, docs, inline_limit);
        }

        let mut body_parts = vec![self.leading_delimited_gap_doc(&leading_gap, true)];
        let mut items = items.into_iter();
        let first_item = items.next().expect("non-empty delimited items");
        let mut previous_end = range_end(first_item.range);
        body_parts.push(first_item.doc);

        for item in items {
            let gap = self.comment_gap(previous_end, range_start(item.range), true, true);
            body_parts.push(Doc::text(","));
            body_parts.push(self.gap_separator_doc(&gap, 1, true, true));
            body_parts.push(item.doc);
            previous_end = range_end(item.range);
        }

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }

        body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));

        Doc::concat(vec![
            Doc::text(spec.open),
            Doc::indent(1, Doc::concat(body_parts)),
            Doc::hard_line(),
            Doc::text(spec.close),
        ])
    }

    fn format_comma_separated_body_doc(
        &self,
        node: &SyntaxNode,
        items: Vec<DelimitedItemDoc>,
    ) -> Doc {
        if items.is_empty() {
            return Doc::nil();
        }

        let mut requires_trivia_layout = false;
        let mut cursor = range_end(items[0].range);
        for item in items.iter().skip(1) {
            let gap = self.comment_gap(cursor, range_start(item.range), true, true);
            requires_trivia_layout |= gap_requires_trivia_layout(&gap);
            cursor = range_end(item.range);
        }
        let trailing_gap = self.comment_gap(cursor, range_end(node.range()), true, false);
        requires_trivia_layout |= gap_requires_trivia_layout(&trailing_gap);

        let inline_items = items
            .iter()
            .map(|item| self.render_fragment(&item.doc, 0))
            .collect::<Vec<_>>();
        let inline = inline_items.join(", ");
        let should_inline = !requires_trivia_layout
            && self.should_inline_delimited_items(
                &inline_items,
                &inline,
                self.options.max_line_length,
            );
        if should_inline {
            return Doc::text(inline);
        }

        let mut body_parts = vec![Doc::hard_line()];
        let mut items = items.into_iter();
        let first_item = items.next().expect("non-empty comma-separated items");
        let mut previous_end = range_end(first_item.range);
        body_parts.push(first_item.doc);

        for item in items {
            let gap = self.comment_gap(previous_end, range_start(item.range), true, true);
            body_parts.push(Doc::text(","));
            body_parts.push(self.gap_separator_doc(&gap, 1, true, true));
            body_parts.push(item.doc);
            previous_end = range_end(item.range);
        }

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }
        if trailing_gap.has_comments() || trailing_gap.trailing_blank_lines_before_next > 0 {
            body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        }
        body_parts.push(Doc::hard_line());

        Doc::indent(1, Doc::concat(body_parts))
    }

    fn leading_delimited_gap_doc(&self, gap: &GapTrivia, include_terminal_newline: bool) -> Doc {
        if gap.has_vertical_comments() {
            let vertical_comments = gap.vertical_comments();
            let mut parts = vec![hard_lines(vertical_comments[0].blank_lines_before + 1)];
            parts.push(self.render_line_comments_doc(vertical_comments));

            let suffix_newlines =
                gap.trailing_blank_lines_before_next + usize::from(include_terminal_newline);
            if suffix_newlines > 0 {
                parts.push(hard_lines(suffix_newlines));
            }

            return Doc::concat(parts);
        }

        hard_lines(gap.trailing_blank_lines_before_next + usize::from(include_terminal_newline))
    }

    pub(crate) fn raw(&self, node: &SyntaxNode) -> String {
        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;
        self.source[start..end].trim().to_owned()
    }

    fn doc_renders_single_line(&self, doc: &Doc, indent: usize) -> bool {
        !self.render_fragment(doc, indent).contains('\n')
    }

    fn while_requires_raw_fallback(&self, while_expr: WhileExpr<'_>) -> bool {
        let Some(condition) = while_expr.condition() else {
            return self.node_has_unowned_comments(while_expr.syntax());
        };
        let body_start = while_expr
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(while_expr.syntax().range().end()) as usize);

        self.node_has_unowned_comments_outside(
            while_expr.syntax(),
            &[(
                u32::from(condition.syntax().range().end()) as usize,
                body_start,
            )],
        )
    }

    fn loop_requires_raw_fallback(&self, loop_expr: LoopExpr<'_>) -> bool {
        let body_start = loop_expr
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(loop_expr.syntax().range().end()) as usize);
        let loop_kw_end = self
            .token_range(loop_expr.syntax(), TokenKind::LoopKw)
            .map(range_end)
            .unwrap_or(body_start);

        self.node_has_unowned_comments_outside(loop_expr.syntax(), &[(loop_kw_end, body_start)])
    }

    fn if_requires_raw_fallback(&self, if_expr: IfExpr<'_>) -> bool {
        let Some(condition) = if_expr.condition() else {
            return self.node_has_unowned_comments(if_expr.syntax());
        };
        let then_start = if_expr
            .then_branch()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(if_expr.syntax().range().end()) as usize);
        let mut allowed_ranges = vec![(
            u32::from(condition.syntax().range().end()) as usize,
            then_start,
        )];

        if let Some(else_branch) = if_expr.else_branch() {
            let else_start = self
                .token_range(else_branch.syntax(), TokenKind::ElseKw)
                .map(range_start)
                .unwrap_or_else(|| u32::from(else_branch.syntax().range().start()) as usize);
            let then_end = if_expr
                .then_branch()
                .map(|body| u32::from(body.syntax().range().end()) as usize)
                .unwrap_or(else_start);
            allowed_ranges.push((then_end, else_start));

            if self.else_branch_requires_raw_fallback(else_branch) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside(if_expr.syntax(), &allowed_ranges)
    }

    fn else_branch_requires_raw_fallback(&self, else_branch: rhai_syntax::ElseBranch<'_>) -> bool {
        let body_start = else_branch
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(else_branch.syntax().range().end()) as usize);
        let else_kw_end = self
            .token_range(else_branch.syntax(), TokenKind::ElseKw)
            .map(range_end)
            .unwrap_or(body_start);

        self.node_has_unowned_comments_outside(else_branch.syntax(), &[(else_kw_end, body_start)])
    }

    fn for_requires_raw_fallback(&self, for_expr: ForExpr<'_>) -> bool {
        let Some(iterable) = for_expr.iterable() else {
            return self.node_has_unowned_comments(for_expr.syntax());
        };
        let body_start = for_expr
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or_else(|| u32::from(for_expr.syntax().range().end()) as usize);

        self.node_has_unowned_comments_outside(
            for_expr.syntax(),
            &[(
                u32::from(iterable.syntax().range().end()) as usize,
                body_start,
            )],
        ) || for_expr
            .bindings()
            .is_some_and(|bindings| self.for_bindings_requires_raw_fallback(bindings))
    }

    fn do_requires_raw_fallback(&self, do_expr: DoExpr<'_>) -> bool {
        let body_end = do_expr
            .body()
            .map(|body| u32::from(body.syntax().range().end()) as usize)
            .unwrap_or_else(|| u32::from(do_expr.syntax().range().start()) as usize);
        let body_start = do_expr
            .body()
            .map(|body| u32::from(body.syntax().range().start()) as usize)
            .unwrap_or(body_end);
        let do_kw_end = self
            .token_range(do_expr.syntax(), TokenKind::DoKw)
            .map(range_end)
            .unwrap_or(body_start);
        let Some(condition) = do_expr.condition() else {
            return self
                .node_has_unowned_comments_outside(do_expr.syntax(), &[(do_kw_end, body_start)]);
        };
        let condition_start = u32::from(condition.syntax().range().start()) as usize;
        let condition_kw_end = condition
            .keyword_token()
            .map(|token| u32::from(token.range().end()) as usize)
            .unwrap_or(condition_start);
        let expr_start = condition
            .expr()
            .map(|expr| u32::from(expr.syntax().range().start()) as usize)
            .unwrap_or(condition_kw_end);

        self.node_has_unowned_comments_outside(
            do_expr.syntax(),
            &[(do_kw_end, body_start), (body_end, condition_start)],
        ) || self.node_has_unowned_comments_outside(
            condition.syntax(),
            &[(condition_kw_end, expr_start)],
        )
    }

    fn path_requires_raw_fallback(&self, path: PathExpr<'_>) -> bool {
        let segments = path.segments().collect::<Vec<_>>();
        let separators = self
            .token_ranges(path.syntax(), TokenKind::ColonColon)
            .collect::<Vec<_>>();
        let mut allowed_ranges = Vec::new();

        let (segment_start_index, mut previous_end) = if let Some(base) = path.base() {
            if separators.len() != segments.len() {
                return self.node_has_unowned_comments(path.syntax());
            }

            (0, u32::from(base.syntax().range().end()) as usize)
        } else if let Some(first) = segments.first() {
            if separators.len() + 1 != segments.len() {
                return self.node_has_unowned_comments(path.syntax());
            }

            (1, range_end(first.range()))
        } else {
            return self.node_has_unowned_comments(path.syntax());
        };

        for (separator_range, segment) in separators
            .into_iter()
            .zip(segments.into_iter().skip(segment_start_index))
        {
            allowed_ranges.push((previous_end, range_start(separator_range)));
            allowed_ranges.push((range_end(separator_range), range_start(segment.range())));
            previous_end = range_end(segment.range());
        }

        self.node_has_unowned_comments_outside(path.syntax(), &allowed_ranges)
    }

    fn index_requires_raw_fallback(&self, index: IndexExpr<'_>) -> bool {
        let Some(receiver) = index.receiver() else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(inner) = index.index() else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(open_range) = self
            .token_range(index.syntax(), TokenKind::QuestionOpenBracket)
            .or_else(|| self.token_range(index.syntax(), TokenKind::OpenBracket))
        else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(close_range) = self.token_range(index.syntax(), TokenKind::CloseBracket) else {
            return self.node_has_unowned_comments(index.syntax());
        };

        self.node_has_unowned_comments_outside(
            index.syntax(),
            &[
                (
                    u32::from(receiver.syntax().range().end()) as usize,
                    range_start(open_range),
                ),
                (range_end(open_range), range_start(inner.syntax().range())),
                (range_end(inner.syntax().range()), range_start(close_range)),
            ],
        )
    }

    fn closure_requires_raw_fallback(&self, closure: ClosureExpr<'_>) -> bool {
        let mut allowed_ranges = Vec::new();
        if let Some(params) = closure.params() {
            if self.closure_params_requires_raw_fallback(params) {
                return true;
            }

            if let Some(body) = closure.body() {
                allowed_ranges.push((
                    range_end(params.syntax().range()),
                    range_start(body.syntax().range()),
                ));
            }
        }

        self.node_has_unowned_comments_outside(closure.syntax(), &allowed_ranges)
    }

    fn closure_params_requires_raw_fallback(&self, params: ClosureParamList<'_>) -> bool {
        if self
            .token_range(params.syntax(), TokenKind::PipePipe)
            .is_some()
        {
            return false;
        }

        let names = params.params().collect::<Vec<_>>();
        let Some(open_range) = self.token_ranges(params.syntax(), TokenKind::Pipe).next() else {
            return self.node_has_unowned_comments(params.syntax());
        };
        let Some(close_range) = self.token_ranges(params.syntax(), TokenKind::Pipe).last() else {
            return self.node_has_unowned_comments(params.syntax());
        };

        if names.is_empty() {
            return self.node_has_unowned_comments_outside(
                params.syntax(),
                &[(range_end(open_range), range_start(close_range))],
            );
        }

        let commas = self
            .token_ranges(params.syntax(), TokenKind::Comma)
            .collect::<Vec<_>>();
        if commas.len() + 1 != names.len() {
            return self.node_has_unowned_comments(params.syntax());
        }

        let mut allowed_ranges = vec![(range_end(open_range), range_start(names[0].range()))];
        let mut previous_end = range_end(names[0].range());
        for (comma_range, next_name) in commas.into_iter().zip(names.iter().skip(1)) {
            allowed_ranges.push((previous_end, range_start(comma_range)));
            allowed_ranges.push((range_end(comma_range), range_start(next_name.range())));
            previous_end = range_end(next_name.range());
        }
        allowed_ranges.push((previous_end, range_start(close_range)));

        self.node_has_unowned_comments_outside(params.syntax(), &allowed_ranges)
    }

    fn for_bindings_requires_raw_fallback(&self, bindings: ForBindings<'_>) -> bool {
        let names = bindings.names().collect::<Vec<_>>();
        match names.as_slice() {
            [] | [_] => self.node_has_unowned_comments(bindings.syntax()),
            [first, second] => {
                let Some(open_range) = self.token_range(bindings.syntax(), TokenKind::OpenParen)
                else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };
                let Some(comma_range) = self.token_range(bindings.syntax(), TokenKind::Comma)
                else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };
                let Some(close_range) = self.token_range(bindings.syntax(), TokenKind::CloseParen)
                else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };

                self.node_has_unowned_comments_outside(
                    bindings.syntax(),
                    &[
                        (range_end(open_range), range_start(first.range())),
                        (range_end(first.range()), range_start(comma_range)),
                        (range_end(comma_range), range_start(second.range())),
                        (range_end(second.range()), range_start(close_range)),
                    ],
                )
            }
            _ => self.node_has_unowned_comments(bindings.syntax()),
        }
    }

    fn object_field_requires_raw_fallback(&self, field: ObjectField<'_>) -> bool {
        let Some(name_range) = field.name_token().map(|token| token.range()) else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(colon_range) = self.token_range(field.syntax(), TokenKind::Colon) else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(value) = field.value() else {
            return self.node_has_unowned_comments(field.syntax());
        };

        self.node_has_unowned_comments_outside(
            field.syntax(),
            &[
                (range_end(name_range), range_start(colon_range)),
                (range_end(colon_range), range_start(value.syntax().range())),
            ],
        )
    }

    fn switch_arm_requires_raw_fallback(&self, arm: SwitchArm<'_>) -> bool {
        let Some(patterns) = arm.patterns() else {
            return self.node_has_unowned_comments(arm.syntax());
        };
        let Some(value) = arm.value() else {
            return self.node_has_unowned_comments(arm.syntax());
        };
        let Some(arrow_range) = self.token_range(arm.syntax(), TokenKind::FatArrow) else {
            return self.node_has_unowned_comments(arm.syntax());
        };

        self.switch_patterns_requires_raw_fallback(patterns)
            || self.node_has_unowned_comments_outside(
                arm.syntax(),
                &[
                    (
                        range_end(patterns.syntax().range()),
                        range_start(arrow_range),
                    ),
                    (range_end(arrow_range), range_start(value.syntax().range())),
                ],
            )
    }

    fn format_switch_patterns_doc(
        &self,
        patterns: rhai_syntax::SwitchPatternList<'_>,
        indent: usize,
    ) -> Doc {
        if patterns.wildcard_token().is_some() {
            return Doc::text("_");
        }

        let values = patterns.exprs().collect::<Vec<_>>();
        if values.is_empty() {
            return Doc::text(self.raw(patterns.syntax()));
        }

        let separators = self
            .token_ranges(patterns.syntax(), TokenKind::Pipe)
            .collect::<Vec<_>>();
        if separators.len() + 1 != values.len() {
            return Doc::text(self.raw(patterns.syntax()));
        }

        let mut doc = self.format_expr_doc(values[0], indent);
        let mut previous_end = range_end(values[0].syntax().range());

        for (separator_range, next_value) in separators.into_iter().zip(values.into_iter().skip(1))
        {
            let next_doc = self.format_expr_doc(next_value, indent);
            if self.range_has_comments(previous_end, range_start(separator_range))
                || self.range_has_comments(
                    range_end(separator_range),
                    range_start(next_value.syntax().range()),
                )
            {
                doc = Doc::concat(vec![
                    doc,
                    self.tight_comment_gap_doc_without_trailing_space(
                        previous_end,
                        range_start(separator_range),
                    ),
                    Doc::text("|"),
                    self.space_or_tight_gap_doc(
                        range_end(separator_range),
                        range_start(next_value.syntax().range()),
                    ),
                    next_doc,
                ]);
            } else {
                doc = Doc::concat(vec![doc, Doc::text(" | "), next_doc]);
            }

            previous_end = range_end(next_value.syntax().range());
        }

        doc
    }

    fn switch_patterns_requires_raw_fallback(
        &self,
        patterns: rhai_syntax::SwitchPatternList<'_>,
    ) -> bool {
        if patterns.wildcard_token().is_some() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let values = patterns.exprs().collect::<Vec<_>>();
        if values.is_empty() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let separators = self
            .token_ranges(patterns.syntax(), TokenKind::Pipe)
            .collect::<Vec<_>>();
        if separators.len() + 1 != values.len() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let mut allowed_ranges = Vec::new();
        let mut previous_end = range_end(values[0].syntax().range());
        for (separator_range, next_value) in separators.into_iter().zip(values.iter().skip(1)) {
            allowed_ranges.push((previous_end, range_start(separator_range)));
            allowed_ranges.push((
                range_end(separator_range),
                range_start(next_value.syntax().range()),
            ));
            previous_end = range_end(next_value.syntax().range());
        }

        self.node_has_unowned_comments_outside(patterns.syntax(), &allowed_ranges)
    }

    fn field_requires_raw_fallback(&self, field: FieldExpr<'_>) -> bool {
        let Some(receiver) = field.receiver() else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(accessor_range) = self
            .token_range(field.syntax(), TokenKind::QuestionDot)
            .or_else(|| self.token_range(field.syntax(), TokenKind::Dot))
        else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(name_range) = field.name_token().map(|token| token.range()) else {
            return self.node_has_unowned_comments(field.syntax());
        };

        self.node_has_unowned_comments_outside(
            field.syntax(),
            &[
                (
                    u32::from(receiver.syntax().range().end()) as usize,
                    range_start(accessor_range),
                ),
                (range_end(accessor_range), range_start(name_range)),
            ],
        )
    }

    fn collect_binary_chain(
        &self,
        expr: Expr<'_>,
        indent: usize,
        operands: &mut Vec<Doc>,
        operators: &mut Vec<String>,
    ) {
        if matches!(expr_support(expr).level, FormatSupportLevel::RawFallback)
            || (matches!(expr_support(expr).level, FormatSupportLevel::Structural)
                && self.expr_requires_raw_fallback(expr))
        {
            operands.push(Doc::text(self.raw(expr.syntax())));
            return;
        }

        if let Expr::Binary(binary) = expr {
            if self.binary_has_operator_comments(binary) {
                operands.push(self.format_binary_doc(binary, indent));
                return;
            }

            if let Some(lhs) = binary.lhs() {
                self.collect_binary_chain(lhs, indent, operands, operators);
            }

            operators.push(
                binary
                    .operator_token()
                    .map(|token| token.text(self.source).to_owned())
                    .unwrap_or_default(),
            );

            if let Some(rhs) = binary.rhs() {
                self.collect_binary_chain(rhs, indent, operands, operators);
            }
        } else {
            operands.push(self.format_expr_doc(expr, indent));
        }
    }

    fn group_tight_suffix_doc(&self, head: Doc, suffix: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            head,
            Doc::indent(1, Doc::concat(vec![Doc::line(), suffix])),
        ]))
    }

    fn group_spaced_suffix_doc(&self, head: Doc, suffix_head: String, tail: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            head,
            Doc::indent(
                1,
                Doc::concat(vec![Doc::soft_line(), Doc::text(suffix_head), tail]),
            ),
        ]))
    }

    fn space_or_tight_gap_doc(&self, start: usize, end: usize) -> Doc {
        if self.range_has_comments(start, end) {
            self.tight_comment_gap_doc(start, end)
        } else {
            Doc::text(" ")
        }
    }

    fn token_ranges<'a>(
        &self,
        node: &'a SyntaxNode,
        kind: TokenKind,
    ) -> impl Iterator<Item = TextRange> + 'a {
        node.children()
            .iter()
            .filter_map(|child| child.as_token())
            .filter(move |token| token.kind() == kind)
            .map(|token| token.range())
    }
}

#[derive(Debug, Clone)]
struct DelimitedItemDoc {
    range: TextRange,
    doc: Doc,
}

#[derive(Debug, Clone, Copy)]
struct DelimitedNodeSpec<'a> {
    node: &'a SyntaxNode,
    open_kind: TokenKind,
    close_kind: TokenKind,
    open: &'a str,
    close: &'a str,
}

fn gap_requires_trivia_layout(gap: &GapTrivia) -> bool {
    !gap.trailing_comments.is_empty()
        || gap.has_vertical_comments()
        || gap.trailing_blank_lines_before_next > 0
}

fn hard_lines(count: usize) -> Doc {
    Doc::concat(vec![Doc::hard_line(); count])
}

fn range_start(range: TextRange) -> usize {
    u32::from(range.start()) as usize
}

fn range_end(range: TextRange) -> usize {
    u32::from(range.end()) as usize
}
