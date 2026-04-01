use rhai_syntax::{
    ArgList, ArrayExpr, ArrayItemList, AstNode, BinaryExpr, CallExpr, ClosureExpr,
    ClosureParamList, DoCondition, DoExpr, ElseBranch, Expr, FieldExpr, ForBindings, ForExpr,
    GapTrivia, IfExpr, IndexExpr, InterpolatedStringExpr, InterpolationItemList, LoopExpr,
    NodeOrToken, ObjectExpr, ObjectField, ObjectFieldList, ParamList, ParenExpr, PathExpr,
    StringPart, StringPartList, SwitchArm, SwitchArmList, SwitchExpr, SwitchPatternList,
    SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange, TokenKind, TriviaBoundary,
    WhileExpr,
};

use crate::ContainerLayoutStyle;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};
use crate::formatter::trivia::comments::GapSeparatorOptions;

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

    fn expr_requires_raw_fallback(&self, expr: Expr) -> bool {
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

    fn call_requires_raw_fallback(&self, call: CallExpr) -> bool {
        let Some(callee) = call.callee() else {
            return self.node_has_unowned_comments(call.syntax());
        };
        let Some(args) = call.args() else {
            return self.node_has_unowned_comments(call.syntax());
        };

        let mut allowed_boundaries = Vec::new();
        if let Some(bang_token) = self.token(call.syntax(), TokenKind::Bang) {
            allowed_boundaries.push(TriviaBoundary::NodeToken(
                callee.syntax(),
                bang_token.clone(),
            ));
            allowed_boundaries.push(TriviaBoundary::TokenNode(bang_token, args.syntax()));
        } else {
            allowed_boundaries.push(TriviaBoundary::NodeNode(callee.syntax(), args.syntax()));
        }
        if self.node_has_unowned_comments_outside_boundaries(call.syntax(), &allowed_boundaries) {
            return true;
        }

        false
    }

    fn unary_requires_raw_fallback(&self, unary: rhai_syntax::UnaryExpr) -> bool {
        let Some(operator_token) = unary.operator_token() else {
            return self.node_has_unowned_comments(unary.syntax());
        };
        let Some(inner) = unary.expr() else {
            return self.node_has_unowned_comments(unary.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            unary.syntax(),
            &[TriviaBoundary::TokenNode(operator_token, inner.syntax())],
        )
    }

    fn binary_requires_raw_fallback(&self, binary: BinaryExpr) -> bool {
        let Some(lhs) = binary.lhs() else {
            return self.node_has_unowned_comments(binary.syntax());
        };
        let Some(rhs) = binary.rhs() else {
            return self.node_has_unowned_comments(binary.syntax());
        };
        let Some(operator_token) = binary.operator_token() else {
            return self.node_has_unowned_comments(binary.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            binary.syntax(),
            &[
                TriviaBoundary::NodeToken(lhs.syntax(), operator_token.clone()),
                TriviaBoundary::TokenNode(operator_token, rhs.syntax()),
            ],
        )
    }

    fn assign_requires_raw_fallback(&self, assign: rhai_syntax::AssignExpr) -> bool {
        let Some(lhs) = assign.lhs() else {
            return self.node_has_unowned_comments(assign.syntax());
        };
        let Some(rhs) = assign.rhs() else {
            return self.node_has_unowned_comments(assign.syntax());
        };
        let Some(operator_token) = assign.operator_token() else {
            return self.node_has_unowned_comments(assign.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            assign.syntax(),
            &[
                TriviaBoundary::NodeToken(lhs.syntax(), operator_token.clone()),
                TriviaBoundary::TokenNode(operator_token, rhs.syntax()),
            ],
        )
    }

    fn paren_requires_raw_fallback(&self, paren: ParenExpr) -> bool {
        let Some(inner) = paren.expr() else {
            return self.node_has_unowned_comments(paren.syntax());
        };
        let Some(open_token) = paren
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::OpenParen))
        else {
            return self.node_has_unowned_comments(paren.syntax());
        };
        let Some(close_token) = paren
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::CloseParen))
        else {
            return self.node_has_unowned_comments(paren.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            paren.syntax(),
            &[
                TriviaBoundary::TokenNode(open_token, inner.syntax()),
                TriviaBoundary::NodeToken(inner.syntax(), close_token),
            ],
        )
    }

    fn format_unary_doc(&self, unary: rhai_syntax::UnaryExpr, indent: usize) -> Doc {
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

    fn format_path_doc(&self, path: PathExpr, indent: usize) -> Doc {
        let segments = path.segments().collect::<Vec<_>>();
        let separators = self
            .tokens(path.syntax(), TokenKind::ColonColon)
            .collect::<Vec<_>>();

        let (mut doc, segment_start_index, mut previous_node, mut previous_token) =
            if let Some(base) = path.base() {
                if separators.len() != segments.len() {
                    return Doc::text(self.raw(path.syntax()));
                }
                (
                    self.format_expr_doc(base.clone(), indent),
                    0,
                    Some(base.syntax()),
                    None,
                )
            } else if let Some(first) = segments.first() {
                if separators.len() + 1 != segments.len() {
                    return Doc::text(self.raw(path.syntax()));
                }

                (Doc::text(first.text()), 1, None, Some(first.clone()))
            } else {
                return Doc::text(self.raw(path.syntax()));
            };

        for (separator_token, segment) in separators
            .into_iter()
            .zip(segments.into_iter().skip(segment_start_index))
        {
            let before_gap = previous_node
                .clone()
                .and_then(|previous_node| {
                    self.boundary_trivia(
                        path.syntax(),
                        TriviaBoundary::NodeToken(previous_node, separator_token.clone()),
                    )
                })
                .or_else(|| {
                    previous_token.clone().and_then(|previous_token| {
                        self.boundary_trivia(
                            path.syntax(),
                            TriviaBoundary::TokenToken(previous_token, separator_token.clone()),
                        )
                    })
                })
                .unwrap_or_default();
            let after_gap = self
                .boundary_trivia(
                    path.syntax(),
                    TriviaBoundary::TokenToken(separator_token.clone(), segment.clone()),
                )
                .unwrap_or_default();

            if before_gap.has_comments() || after_gap.has_comments() {
                doc = Doc::concat(vec![
                    doc,
                    self.tight_comment_gap_from_gap(&before_gap, true),
                    Doc::text("::"),
                    self.tight_comment_gap_from_gap(&after_gap, true),
                    Doc::text(segment.text()),
                ]);
            } else {
                doc = self.group_tight_suffix_doc(doc, Doc::text(format!("::{}", segment.text())));
            }

            previous_node = None;
            previous_token = Some(segment);
        }

        doc
    }

    fn format_assign_doc(&self, assign: rhai_syntax::AssignExpr, indent: usize) -> Doc {
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

    fn format_binary_doc(&self, binary: BinaryExpr, indent: usize) -> Doc {
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

    fn format_binary_with_operator_comments_doc(&self, binary: BinaryExpr, indent: usize) -> Doc {
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

    fn format_paren_doc(&self, paren: ParenExpr, indent: usize) -> Doc {
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

    fn format_call_doc(&self, call: CallExpr, indent: usize) -> Doc {
        let callee = call
            .callee()
            .map(|callee| self.format_expr_doc(callee, indent))
            .unwrap_or_else(|| Doc::text(""));
        let args = call
            .args()
            .map(|args| self.format_arg_list_doc(args, indent))
            .unwrap_or_else(|| Doc::text("()"));
        let Some(callee_expr) = call.callee() else {
            return Doc::concat(vec![callee, args]);
        };
        let Some(arg_list) = call.args() else {
            return Doc::concat(vec![callee, args]);
        };
        let Some(args_open_token) = arg_list
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::OpenParen))
        else {
            return Doc::concat(vec![callee, args]);
        };

        if let Some(bang_token) = call
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::Bang))
        {
            Doc::concat(vec![
                callee,
                self.tight_comment_gap_for_boundary(
                    call.syntax(),
                    TriviaBoundary::NodeToken(callee_expr.syntax(), bang_token.clone()),
                    true,
                ),
                Doc::text("!"),
                self.tight_comment_gap_for_boundary(
                    call.syntax(),
                    TriviaBoundary::TokenToken(bang_token, args_open_token),
                    true,
                ),
                args,
            ])
        } else {
            Doc::concat(vec![
                callee,
                self.tight_comment_gap_for_boundary(
                    call.syntax(),
                    TriviaBoundary::NodeToken(callee_expr.syntax(), args_open_token),
                    true,
                ),
                args,
            ])
        }
    }

    fn format_arg_list_doc(&self, args: ArgList, indent: usize) -> Doc {
        let values = self.arg_list_items(args.clone(), indent);
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: &args.syntax(),
                open_kind: TokenKind::OpenParen,
                close_kind: TokenKind::CloseParen,
                open: "(",
                close: ")",
            },
            values,
            None,
        )
    }

    pub(crate) fn format_arg_list_body_doc(&self, args: ArgList, indent: usize) -> Doc {
        self.format_arg_list_doc(args, indent)
    }

    pub(crate) fn format_array_item_list_body_doc(
        &self,
        items: ArrayItemList,
        indent: usize,
    ) -> Doc {
        self.format_array_item_list_doc(items, indent)
    }

    pub(crate) fn format_interpolation_item_list_body_doc(
        &self,
        items: InterpolationItemList,
        indent: usize,
    ) -> Doc {
        self.format_interpolation_item_docs(items.items().collect::<Vec<_>>(), indent)
    }

    pub(crate) fn format_string_part_list_body_doc(
        &self,
        parts: StringPartList,
        indent: usize,
    ) -> Doc {
        self.format_string_part_docs(parts.parts().collect::<Vec<_>>(), indent)
    }

    pub(crate) fn format_object_field_list_body_doc(
        &self,
        fields: ObjectFieldList,
        indent: usize,
    ) -> Doc {
        let item_docs = self.object_field_list_items(fields.clone(), indent);
        self.format_comma_separated_body_doc(&fields.syntax(), item_docs)
    }

    pub(crate) fn format_switch_arm_list_body_doc(
        &self,
        arms: SwitchArmList,
        indent: usize,
    ) -> Doc {
        let item_docs = self.switch_arm_list_items(arms.clone(), indent);
        self.format_comma_separated_body_doc(&arms.syntax(), item_docs)
    }

    fn format_array_doc(&self, array: ArrayExpr, indent: usize) -> Doc {
        array
            .items()
            .map(|items| self.format_array_item_list_doc(items, indent))
            .unwrap_or_else(|| Doc::text("[]"))
    }

    fn format_index_doc(&self, index: IndexExpr, indent: usize) -> Doc {
        let receiver_expr = index.receiver();
        let receiver = receiver_expr
            .clone()
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let inner_expr = index.index();
        let inner = inner_expr
            .clone()
            .map(|inner| self.format_expr_doc(inner, indent))
            .unwrap_or_else(|| Doc::text(""));
        let open_token = index.syntax().direct_significant_tokens().find(|token| {
            matches!(
                token.kind().token_kind(),
                Some(TokenKind::QuestionOpenBracket | TokenKind::OpenBracket)
            )
        });
        let close_token = index
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::CloseBracket));
        let open = if open_token
            .clone()
            .is_some_and(|token| token.kind().token_kind() == Some(TokenKind::QuestionOpenBracket))
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
        let Some(open_token) = open_token else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };
        let Some(close_token) = close_token else {
            return self.group_tight_suffix_doc(receiver, suffix);
        };

        let receiver_gap = self
            .boundary_trivia(
                index.syntax(),
                TriviaBoundary::NodeToken(receiver_expr.syntax(), open_token.clone()),
            )
            .unwrap_or_default();
        let inner_gap = self
            .boundary_trivia(
                index.syntax(),
                TriviaBoundary::TokenNode(open_token.clone(), inner_expr.syntax()),
            )
            .unwrap_or_default();
        let close_gap = self
            .boundary_trivia(
                index.syntax(),
                TriviaBoundary::NodeToken(inner_expr.syntax(), close_token.clone()),
            )
            .unwrap_or_default();

        if receiver_gap.has_comments() || inner_gap.has_comments() || close_gap.has_comments() {
            Doc::concat(vec![
                receiver,
                self.tight_comment_gap_from_gap(&receiver_gap, true),
                Doc::text(open),
                self.tight_comment_gap_from_gap(&inner_gap, true),
                inner,
                self.tight_comment_gap_from_gap(&close_gap, true),
                Doc::text("]"),
            ])
        } else {
            self.group_tight_suffix_doc(receiver, suffix)
        }
    }

    fn format_field_doc(&self, field: FieldExpr, indent: usize) -> Doc {
        let receiver_expr = field.receiver();
        let receiver = receiver_expr
            .clone()
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let name = field
            .name_token()
            .map(|name| name.text().to_owned())
            .unwrap_or_default();
        let accessor_token = field.syntax().direct_significant_tokens().find(|token| {
            matches!(
                token.kind().token_kind(),
                Some(TokenKind::QuestionDot | TokenKind::Dot)
            )
        });
        let accessor = if accessor_token
            .clone()
            .is_some_and(|token| token.kind().token_kind() == Some(TokenKind::QuestionDot))
        {
            "?."
        } else {
            "."
        };

        let Some(receiver_expr) = receiver_expr else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };
        let Some(accessor_token) = accessor_token else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };
        let Some(name_token) = field.name_token() else {
            return self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")));
        };

        let receiver_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::NodeToken(receiver_expr.syntax(), accessor_token.clone()),
            )
            .unwrap_or_default();
        let name_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::TokenToken(accessor_token.clone(), name_token.clone()),
            )
            .unwrap_or_default();

        if receiver_gap.has_comments() || name_gap.has_comments() {
            Doc::concat(vec![
                receiver,
                self.tight_comment_gap_from_gap(&receiver_gap, true),
                Doc::text(accessor),
                self.tight_comment_gap_from_gap(&name_gap, true),
                Doc::text(name),
            ])
        } else {
            self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")))
        }
    }

    fn binary_has_operator_comments(&self, binary: &BinaryExpr) -> bool {
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

    fn format_object_doc(&self, object: ObjectExpr, indent: usize) -> Doc {
        let fields = object
            .field_list()
            .map(|fields| self.object_field_list_items(fields, indent + 1))
            .unwrap_or_default();
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: &object.syntax(),
                open_kind: TokenKind::HashBraceOpen,
                close_kind: TokenKind::CloseBrace,
                open: "#{",
                close: "}",
            },
            fields,
            Some(60),
        )
    }

    fn object_field_list_items(
        &self,
        fields: ObjectFieldList,
        indent: usize,
    ) -> Vec<DelimitedItemDoc> {
        fields
            .fields()
            .map(|field| DelimitedItemDoc {
                element: field.syntax().into(),
                range: field.syntax().text_range(),
                doc: self.format_object_field_doc(field, indent),
            })
            .collect()
    }

    fn switch_arm_list_items(&self, arms: SwitchArmList, indent: usize) -> Vec<DelimitedItemDoc> {
        arms.arms()
            .map(|arm| DelimitedItemDoc {
                element: arm.syntax().into(),
                range: arm.syntax().text_range(),
                doc: self.format_switch_arm_doc(arm, indent),
            })
            .collect()
    }

    fn format_object_field_doc(&self, field: ObjectField, indent: usize) -> Doc {
        if self.object_field_requires_raw_fallback(field.clone()) {
            return Doc::text(self.raw(field.syntax()));
        }

        let name_token = field.name_token();
        let name = field
            .name_token()
            .map(|token| token.text().to_owned())
            .unwrap_or_default();
        let value_expr = field.value();
        let value = value_expr
            .clone()
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(name_token) = name_token else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(colon_token) = field
            .syntax()
            .direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(TokenKind::Colon))
        else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };
        let Some(value_expr) = value_expr else {
            return Doc::concat(vec![Doc::text(format!("{name}: ")), value]);
        };

        let before_colon_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::TokenToken(name_token.clone(), colon_token.clone()),
            )
            .unwrap_or_default();
        let after_colon_gap = self
            .boundary_trivia(
                field.syntax(),
                TriviaBoundary::TokenNode(colon_token.clone(), value_expr.syntax()),
            )
            .unwrap_or_default();

        if before_colon_gap.has_comments() || after_colon_gap.has_comments() {
            return Doc::concat(vec![
                Doc::text(name),
                self.tight_comment_gap_from_gap(&before_colon_gap, false),
                Doc::text(":"),
                self.space_or_tight_gap_from_gap(&after_colon_gap),
                value,
            ]);
        }

        Doc::concat(vec![Doc::text(format!("{name}: ")), value])
    }

    fn format_if_expr_doc(&self, if_expr: IfExpr, indent: usize) -> Doc {
        let condition_expr = if_expr.condition();
        let condition = condition_expr
            .clone()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let then_expr = if_expr.then_branch();
        let then_branch = then_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let if_kw = self.token(if_expr.syntax(), TokenKind::IfKw);
        let condition_end = condition_expr
            .as_ref()
            .map(|expr| u32::from(expr.syntax().text_range().end()) as usize)
            .unwrap_or_else(|| {
                self.token_range(if_expr.syntax(), TokenKind::IfKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(if_expr.syntax().text_range().start()) as usize)
            });
        let then_start = then_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(if_expr.syntax().text_range().end()) as usize);
        let then_separator = match (condition_expr.clone(), if_kw, then_expr.clone()) {
            (Some(condition_expr), _, Some(then_expr)) => self.head_body_separator_for_boundary(
                if_expr.syntax(),
                TriviaBoundary::NodeNode(condition_expr.syntax(), then_expr.syntax()),
            ),
            (None, Some(if_kw), Some(then_expr)) => self.head_body_separator_for_boundary(
                if_expr.syntax(),
                TriviaBoundary::TokenNode(if_kw, then_expr.syntax()),
            ),
            _ => self.head_body_separator_doc(condition_end, then_start),
        };
        let mut parts = vec![
            self.group_keyword_clause_doc("if", condition),
            then_separator,
            then_branch,
        ];

        if let Some(else_branch) = if_expr.else_branch() {
            let else_start = self
                .token_range(else_branch.syntax(), TokenKind::ElseKw)
                .map(range_start)
                .unwrap_or_else(|| u32::from(else_branch.syntax().text_range().start()) as usize);
            let then_end = then_expr
                .as_ref()
                .map(|body| u32::from(body.syntax().text_range().end()) as usize)
                .unwrap_or(else_start);
            parts.push(match then_expr.clone() {
                Some(then_expr) => self.inline_or_boundary_separator_doc(
                    if_expr.syntax(),
                    TriviaBoundary::NodeNode(then_expr.syntax(), else_branch.syntax()),
                    GapSeparatorOptions {
                        inline_text: " ",
                        minimum_newlines: 1,
                        has_previous: true,
                        has_next: true,
                        include_terminal_newline: true,
                    },
                ),
                None => self.inline_or_gap_separator_doc(
                    then_end,
                    else_start,
                    GapSeparatorOptions {
                        inline_text: " ",
                        minimum_newlines: 1,
                        has_previous: true,
                        has_next: true,
                        include_terminal_newline: true,
                    },
                ),
            });
            parts.push(self.format_else_branch_doc(else_branch, indent));
        }

        Doc::concat(parts)
    }

    fn format_switch_expr_doc(&self, switch_expr: SwitchExpr, indent: usize) -> Doc {
        let scrutinee = switch_expr
            .scrutinee()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let arms = switch_expr
            .arm_list()
            .map(|arms| arms.arms().collect::<Vec<_>>())
            .unwrap_or_default();
        let open_brace = self.token(switch_expr.syntax(), TokenKind::OpenBrace);
        let close_brace = self.token(switch_expr.syntax(), TokenKind::CloseBrace);
        let open_brace_end = open_brace
            .as_ref()
            .map(|range| u32::from(range.text_range().end()) as usize)
            .unwrap_or_else(|| u32::from(switch_expr.syntax().text_range().start()) as usize);
        let close_brace_start = close_brace
            .as_ref()
            .map(|range| u32::from(range.text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(switch_expr.syntax().text_range().end()) as usize);
        let arm_elements = arms
            .iter()
            .map(|arm| arm.syntax().into())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(open_brace_end, close_brace_start, &arm_elements);
        let leading_gap = owned.leading.clone();

        if arms.is_empty() {
            if !leading_gap.has_comments() {
                return Doc::concat(vec![
                    self.group_keyword_clause_doc("switch", scrutinee),
                    Doc::text(" {}"),
                ]);
            }

            return Doc::concat(vec![
                self.group_keyword_clause_doc("switch", scrutinee),
                Doc::text(" {"),
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

        let mut body_parts = self.format_comma_sequence_body_doc(
            arms.iter()
                .map(|arm| self.format_switch_arm_doc(arm.clone(), indent + 1))
                .collect(),
            &owned,
        );

        self.append_sequence_trailing_doc(&mut body_parts, &owned.trailing, !arms.is_empty(), 1);

        Doc::concat(vec![
            self.group_keyword_clause_doc("switch", scrutinee),
            Doc::text(" {"),
            Doc::indent(
                1,
                Doc::concat(vec![Doc::hard_line(), Doc::concat(body_parts)]),
            ),
            Doc::hard_line(),
            Doc::text("}"),
        ])
    }

    fn format_switch_arm_doc(&self, arm: SwitchArm, indent: usize) -> Doc {
        if self.switch_arm_requires_raw_fallback(arm.clone()) {
            return Doc::text(self.raw(arm.syntax()));
        }

        let patterns_node = arm.patterns();
        let patterns = patterns_node
            .clone()
            .map(|patterns| self.format_switch_patterns_doc(patterns, indent))
            .unwrap_or_else(|| Doc::text("_"));
        let value_expr = arm.value();
        let value = value_expr
            .clone()
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        let Some(patterns_node) = patterns_node else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };
        let Some(value_expr) = value_expr else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };
        let Some(arrow_token) = self.token(arm.syntax(), TokenKind::FatArrow) else {
            return Doc::concat(vec![patterns, Doc::text(" => "), value]);
        };

        let before_arrow_gap = self.boundary_trivia(
            arm.syntax(),
            TriviaBoundary::NodeToken(patterns_node.syntax(), arrow_token.clone()),
        );
        let after_arrow_gap = self.boundary_trivia(
            arm.syntax(),
            TriviaBoundary::TokenNode(arrow_token, value_expr.syntax()),
        );

        if before_arrow_gap
            .as_ref()
            .is_some_and(|gap| gap.has_comments())
            || after_arrow_gap
                .as_ref()
                .is_some_and(|gap| gap.has_comments())
        {
            return Doc::concat(vec![
                patterns,
                self.space_or_tight_gap_from_gap(&before_arrow_gap.unwrap_or_default()),
                Doc::text("=>"),
                self.space_or_tight_gap_from_gap(&after_arrow_gap.unwrap_or_default()),
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

    fn format_while_expr_doc(&self, while_expr: WhileExpr, indent: usize) -> Doc {
        let condition_expr = while_expr.condition();
        let condition = condition_expr
            .clone()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body_expr = while_expr.body();
        let body = body_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let while_kw = self.token(while_expr.syntax(), TokenKind::WhileKw);
        let condition_end = condition_expr
            .as_ref()
            .map(|expr| u32::from(expr.syntax().text_range().end()) as usize)
            .unwrap_or_else(|| {
                self.token_range(while_expr.syntax(), TokenKind::WhileKw)
                    .map(range_end)
                    .unwrap_or_else(|| u32::from(while_expr.syntax().text_range().start()) as usize)
            });
        let body_start = body_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(while_expr.syntax().text_range().end()) as usize);
        let body_separator = match (condition_expr.clone(), while_kw, body_expr.clone()) {
            (Some(condition_expr), _, Some(body_expr)) => self.head_body_separator_for_boundary(
                while_expr.syntax(),
                TriviaBoundary::NodeNode(condition_expr.syntax(), body_expr.syntax()),
            ),
            (None, Some(while_kw), Some(body_expr)) => self.head_body_separator_for_boundary(
                while_expr.syntax(),
                TriviaBoundary::TokenNode(while_kw, body_expr.syntax()),
            ),
            _ => self.head_body_separator_doc(condition_end, body_start),
        };
        Doc::concat(vec![
            self.group_keyword_clause_doc("while", condition),
            body_separator,
            body,
        ])
    }

    fn format_loop_expr_doc(&self, loop_expr: LoopExpr, indent: usize) -> Doc {
        let body_expr = loop_expr.body();
        let body = body_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let loop_kw = self.token(loop_expr.syntax(), TokenKind::LoopKw);
        let head_end = self
            .token_range(loop_expr.syntax(), TokenKind::LoopKw)
            .map(range_end)
            .unwrap_or_else(|| u32::from(loop_expr.syntax().text_range().start()) as usize);
        let body_start = body_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(loop_expr.syntax().text_range().end()) as usize);
        let body_separator = match (loop_kw, body_expr.clone()) {
            (Some(loop_kw), Some(body_expr)) => self.head_body_separator_for_boundary(
                loop_expr.syntax(),
                TriviaBoundary::TokenNode(loop_kw, body_expr.syntax()),
            ),
            _ => self.head_body_separator_doc(head_end, body_start),
        };
        Doc::concat(vec![Doc::text("loop"), body_separator, body])
    }

    fn format_for_expr_doc(&self, for_expr: ForExpr, indent: usize) -> Doc {
        let bindings = self.format_for_bindings_doc(for_expr.bindings());
        let iterable_expr = for_expr.iterable();
        let iterable = iterable_expr
            .clone()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body_expr = for_expr.body();
        let body = body_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let iterable_end = iterable_expr
            .as_ref()
            .map(|expr| u32::from(expr.syntax().text_range().end()) as usize)
            .unwrap_or_else(|| u32::from(for_expr.syntax().text_range().start()) as usize);
        let body_start = body_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(for_expr.syntax().text_range().end()) as usize);
        let head = Doc::group(Doc::concat(vec![
            Doc::text("for"),
            Doc::indent(
                1,
                Doc::concat(vec![
                    Doc::soft_line(),
                    bindings,
                    Doc::soft_line(),
                    Doc::text("in "),
                    iterable,
                ]),
            ),
        ]));
        let body_separator = match (iterable_expr.clone(), body_expr.clone()) {
            (Some(iterable_expr), Some(body_expr)) => self.head_body_separator_for_boundary(
                for_expr.syntax(),
                TriviaBoundary::NodeNode(iterable_expr.syntax(), body_expr.syntax()),
            ),
            _ => self.head_body_separator_doc(iterable_end, body_start),
        };
        Doc::concat(vec![head, body_separator, body])
    }

    pub(crate) fn format_for_bindings_doc(&self, bindings: Option<ForBindings>) -> Doc {
        let Some(bindings) = bindings else {
            return Doc::text("_");
        };
        let names = bindings.names().collect::<Vec<_>>();
        match names.as_slice() {
            [] => Doc::text("_"),
            [name] => Doc::text(name.text()),
            [first, second] => {
                let Some(open_token) = self.token(bindings.syntax(), TokenKind::OpenParen) else {
                    return Doc::text(self.raw(bindings.syntax()));
                };
                let Some(comma_token) = self.token(bindings.syntax(), TokenKind::Comma) else {
                    return Doc::text(self.raw(bindings.syntax()));
                };
                let Some(close_token) = self.token(bindings.syntax(), TokenKind::CloseParen) else {
                    return Doc::text(self.raw(bindings.syntax()));
                };
                let open_gap = self.boundary_trivia(
                    bindings.syntax(),
                    TriviaBoundary::TokenToken(open_token.clone(), first.clone()),
                );
                let before_comma_gap = self.boundary_trivia(
                    bindings.syntax(),
                    TriviaBoundary::TokenToken(first.clone(), comma_token.clone()),
                );
                let after_comma_gap = self.boundary_trivia(
                    bindings.syntax(),
                    TriviaBoundary::TokenToken(comma_token.clone(), second.clone()),
                );
                let close_gap = self.boundary_trivia(
                    bindings.syntax(),
                    TriviaBoundary::TokenToken(second.clone(), close_token.clone()),
                );

                if open_gap.as_ref().is_some_and(|gap| gap.has_comments())
                    || before_comma_gap
                        .as_ref()
                        .is_some_and(|gap| gap.has_comments())
                    || after_comma_gap
                        .as_ref()
                        .is_some_and(|gap| gap.has_comments())
                    || close_gap.as_ref().is_some_and(|gap| gap.has_comments())
                {
                    Doc::concat(vec![
                        Doc::text("("),
                        self.tight_comment_gap_from_gap(&open_gap.unwrap_or_default(), true),
                        Doc::text(first.text()),
                        self.tight_comment_gap_from_gap(
                            &before_comma_gap.unwrap_or_default(),
                            false,
                        ),
                        Doc::text(","),
                        self.space_or_tight_gap_from_gap(&after_comma_gap.unwrap_or_default()),
                        Doc::text(second.text()),
                        self.tight_comment_gap_from_gap(&close_gap.unwrap_or_default(), true),
                        Doc::text(")"),
                    ])
                } else {
                    Doc::group(Doc::concat(vec![
                        Doc::text("("),
                        Doc::indent(
                            1,
                            Doc::concat(vec![
                                Doc::line(),
                                Doc::text(first.text()),
                                Doc::text(","),
                                Doc::soft_line(),
                                Doc::text(second.text()),
                            ]),
                        ),
                        Doc::line(),
                        Doc::text(")"),
                    ]))
                }
            }
            _ => Doc::text(self.raw(bindings.syntax())),
        }
    }

    fn format_do_expr_doc(&self, do_expr: DoExpr, indent: usize) -> Doc {
        let body_expr = do_expr.body();
        let body = body_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let condition = do_expr.condition();
        let do_kw_end = self
            .token_range(do_expr.syntax(), TokenKind::DoKw)
            .map(range_end)
            .unwrap_or_else(|| u32::from(do_expr.syntax().text_range().start()) as usize);
        let body_start = body_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(do_expr.syntax().text_range().end()) as usize);
        let body_end = body_expr
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().end()) as usize)
            .unwrap_or(body_start);
        let condition_start = condition
            .as_ref()
            .map(|condition| u32::from(condition.syntax().text_range().start()) as usize)
            .unwrap_or(body_end);
        let condition_doc = condition
            .clone()
            .map(|condition| self.format_do_condition_doc(condition, indent))
            .unwrap_or_else(|| Doc::text("while"));
        let do_kw = self.token(do_expr.syntax(), TokenKind::DoKw);
        Doc::concat(vec![
            Doc::text("do"),
            match (do_kw, body_expr.clone()) {
                (Some(do_kw), Some(body_expr)) => self.head_body_separator_for_boundary(
                    do_expr.syntax(),
                    TriviaBoundary::TokenNode(do_kw, body_expr.syntax()),
                ),
                _ => self.head_body_separator_doc(do_kw_end, body_start),
            },
            body,
            match (body_expr.clone(), condition.clone()) {
                (Some(body_expr), Some(condition)) => self.inline_or_boundary_separator_doc(
                    do_expr.syntax(),
                    TriviaBoundary::NodeNode(body_expr.syntax(), condition.syntax()),
                    GapSeparatorOptions {
                        inline_text: " ",
                        minimum_newlines: 1,
                        has_previous: true,
                        has_next: true,
                        include_terminal_newline: true,
                    },
                ),
                _ => self.inline_or_gap_separator_doc(
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
            },
            condition_doc,
        ])
    }

    pub(crate) fn format_do_condition_doc(&self, condition: DoCondition, indent: usize) -> Doc {
        let keyword_token = condition.keyword_token();
        let keyword = keyword_token
            .as_ref()
            .map(|token| token.text().to_owned())
            .unwrap_or_else(|| "while".to_owned());
        let expr = condition
            .expr()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let expr_node = condition.expr();

        if expr_node.is_none() {
            return Doc::text(keyword);
        }

        let separator = match (keyword_token, expr_node) {
            (Some(keyword_token), Some(expr_node)) => self
                .boundary_trivia(
                    condition.syntax(),
                    TriviaBoundary::TokenNode(keyword_token, expr_node.syntax()),
                )
                .map(|gap| self.soft_or_tight_gap_from_gap(&gap))
                .unwrap_or_else(Doc::soft_line),
            _ => Doc::soft_line(),
        };

        Doc::group(Doc::concat(vec![
            Doc::text(keyword),
            Doc::indent(1, Doc::concat(vec![separator, expr])),
        ]))
    }

    fn format_closure_expr_doc(&self, closure: ClosureExpr, indent: usize) -> Doc {
        let params_node = closure.params();
        let params = self.format_closure_params_doc(params_node.clone());
        let body_expr = closure.body();
        let body = body_expr
            .clone()
            .map(|body| self.format_expr_doc(body, indent))
            .unwrap_or_else(|| Doc::text(""));
        let head_gap = match (params_node.clone(), body_expr.clone()) {
            (Some(params), Some(body)) => self.boundary_trivia(
                closure.syntax(),
                TriviaBoundary::NodeNode(params.syntax(), body.syntax()),
            ),
            _ => None,
        };
        let has_head_comments = head_gap.as_ref().is_some_and(|gap| gap.has_comments());

        if !has_head_comments && self.doc_renders_single_line(&body, indent) {
            return Doc::group(Doc::concat(vec![
                params,
                Doc::indent(1, Doc::concat(vec![Doc::soft_line(), body])),
            ]));
        }

        let separator = match (params_node, body_expr) {
            (Some(_), Some(_)) => head_gap
                .as_ref()
                .map(|gap| self.space_or_tight_gap_from_gap(gap))
                .unwrap_or_else(|| Doc::text(" ")),
            _ => Doc::text(" "),
        };

        Doc::concat(vec![params, separator, body])
    }

    fn format_interpolated_string_doc(&self, string: InterpolatedStringExpr, indent: usize) -> Doc {
        let mut parts = vec![Doc::text("`")];
        parts.push(
            string
                .part_list()
                .map(|part_list| self.format_string_part_list_body_doc(part_list, indent))
                .unwrap_or_else(Doc::nil),
        );

        parts.push(Doc::text("`"));
        Doc::concat(parts)
    }

    pub(crate) fn format_closure_params_doc(&self, params: Option<ClosureParamList>) -> Doc {
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
        let Some(open_token) = self.tokens(params.syntax(), TokenKind::Pipe).next() else {
            return Doc::text(self.raw(params.syntax()));
        };
        let Some(close_token) = self.tokens(params.syntax(), TokenKind::Pipe).last() else {
            return Doc::text(self.raw(params.syntax()));
        };

        if names.is_empty() {
            let gap = self.boundary_trivia(
                params.syntax(),
                TriviaBoundary::TokenToken(open_token, close_token),
            );
            if gap.as_ref().is_some_and(|gap| gap.has_comments()) {
                return Doc::concat(vec![
                    Doc::text("|"),
                    self.tight_comment_gap_from_gap(&gap.unwrap_or_default(), true),
                    Doc::text("|"),
                ]);
            }

            return Doc::text("||");
        }

        let commas = self
            .tokens(params.syntax(), TokenKind::Comma)
            .collect::<Vec<_>>();
        if commas.len() + 1 != names.len() {
            return Doc::text(self.raw(params.syntax()));
        }

        let mut parts = vec![Doc::text("|")];
        parts.push(
            self.tight_comment_gap_from_gap(
                &self
                    .boundary_trivia(
                        params.syntax(),
                        TriviaBoundary::TokenToken(open_token.clone(), names[0].clone()),
                    )
                    .unwrap_or_default(),
                true,
            ),
        );
        parts.push(Doc::text(names[0].text()));
        let mut previous_name = names[0].clone();

        for (comma_token, next_name) in commas.into_iter().zip(names.iter().skip(1)) {
            parts.push(
                self.tight_comment_gap_from_gap(
                    &self
                        .boundary_trivia(
                            params.syntax(),
                            TriviaBoundary::TokenToken(previous_name.clone(), comma_token.clone()),
                        )
                        .unwrap_or_default(),
                    false,
                ),
            );
            parts.push(Doc::text(","));
            parts.push(
                self.space_or_tight_gap_from_gap(
                    &self
                        .boundary_trivia(
                            params.syntax(),
                            TriviaBoundary::TokenToken(comma_token, next_name.clone()),
                        )
                        .unwrap_or_default(),
                ),
            );
            parts.push(Doc::text(next_name.text()));
            previous_name = next_name.clone();
        }

        parts.push(
            self.tight_comment_gap_from_gap(
                &self
                    .boundary_trivia(
                        params.syntax(),
                        TriviaBoundary::TokenToken(previous_name, close_token),
                    )
                    .unwrap_or_default(),
                true,
            ),
        );
        parts.push(Doc::text("|"));

        Doc::concat(parts)
    }

    fn format_interpolation_body_doc(
        &self,
        body: rhai_syntax::InterpolationBody,
        indent: usize,
    ) -> Doc {
        if self.node_has_unowned_comments(body.syntax()) {
            return Doc::text(self.raw(body.syntax()));
        }

        body.item_list()
            .map(|items| self.format_interpolation_item_list_body_doc(items, indent))
            .unwrap_or_else(Doc::nil)
    }

    fn format_interpolation_item_docs(&self, items: Vec<rhai_syntax::Item>, indent: usize) -> Doc {
        if items.is_empty() {
            return Doc::nil();
        }

        let item_docs = items
            .iter()
            .map(|item| self.format_item(item.clone(), indent))
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

    fn format_string_part_docs(&self, parts: Vec<StringPart>, indent: usize) -> Doc {
        let mut docs = Vec::new();

        for part in parts {
            match part {
                StringPart::Segment(segment) => {
                    if let Some(token) = segment.text_token() {
                        docs.push(Doc::text(token.text()));
                    }
                }
                StringPart::Interpolation(interpolation) => {
                    let body = interpolation
                        .body()
                        .map(|body| self.format_interpolation_body_doc(body, indent))
                        .unwrap_or_else(Doc::nil);
                    docs.push(Doc::concat(vec![Doc::text("${"), body, Doc::text("}")]));
                }
            }
        }

        Doc::concat(docs)
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

    pub(crate) fn format_params_doc(&self, params: Option<ParamList>, _indent: usize) -> Doc {
        let names = params
            .clone()
            .map(|params| {
                params
                    .params()
                    .map(|param| DelimitedItemDoc {
                        element: param.clone().into(),
                        range: param.text_range(),
                        doc: Doc::text(param.text()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match params {
            Some(params) => self.format_delimited_node_doc(
                DelimitedNodeSpec {
                    node: &params.syntax(),
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

    fn arg_list_items(&self, args: ArgList, indent: usize) -> Vec<DelimitedItemDoc> {
        args.args()
            .map(|expr| DelimitedItemDoc {
                element: expr.syntax().into(),
                range: expr.syntax().text_range(),
                doc: self.format_expr_doc(expr, indent),
            })
            .collect::<Vec<_>>()
    }

    fn array_item_list_items(&self, items: ArrayItemList, indent: usize) -> Vec<DelimitedItemDoc> {
        items
            .exprs()
            .map(|expr| DelimitedItemDoc {
                element: expr.syntax().into(),
                range: expr.syntax().text_range(),
                doc: self.format_expr_doc(expr, indent + 1),
            })
            .collect::<Vec<_>>()
    }

    fn format_array_item_list_doc(&self, items: ArrayItemList, indent: usize) -> Doc {
        let item_docs = self.array_item_list_items(items.clone(), indent);
        self.format_delimited_node_doc(
            DelimitedNodeSpec {
                node: &items.syntax(),
                open_kind: TokenKind::OpenBracket,
                close_kind: TokenKind::CloseBracket,
                open: "[",
                close: "]",
            },
            item_docs,
            None,
        )
    }

    fn format_delimited_node_doc(
        &self,
        spec: DelimitedNodeSpec,
        items: Vec<DelimitedItemDoc>,
        inline_limit: Option<usize>,
    ) -> Doc {
        let open_end = self
            .token_range(spec.node.clone(), spec.open_kind)
            .map(range_end)
            .unwrap_or_else(|| range_start(spec.node.text_range()));
        let close_start = self
            .token_range(spec.node.clone(), spec.close_kind)
            .map(range_start)
            .unwrap_or_else(|| range_end(spec.node.text_range()));
        let item_elements = items
            .iter()
            .map(|item| item.element.clone())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(open_end, close_start, &item_elements);

        if items.is_empty() {
            if !gap_requires_trivia_layout(&owned.leading) {
                return Doc::text(format!("{}{}", spec.open, spec.close));
            }

            return Doc::concat(vec![
                Doc::text(spec.open),
                Doc::indent(1, self.leading_delimited_gap_doc(&owned.leading, false)),
                Doc::hard_line(),
                Doc::text(spec.close),
            ]);
        }

        let leading_gap = owned.leading.clone();
        let mut requires_trivia_layout = gap_requires_trivia_layout(&leading_gap);
        let commas = self
            .tokens(spec.node.clone(), TokenKind::Comma)
            .collect::<Vec<_>>();
        let direct_boundary_layout = commas.len() + 1 == items.len();

        if direct_boundary_layout {
            requires_trivia_layout |=
                self.comma_sequence_requires_trivia_layout(spec.node.clone(), &items, &commas);
        } else {
            for gap in &owned.between {
                requires_trivia_layout |= gap_requires_trivia_layout(gap);
            }
        }

        let trailing_gap = owned.trailing.clone();
        requires_trivia_layout |= gap_requires_trivia_layout(&trailing_gap);

        if !requires_trivia_layout {
            let docs = items.into_iter().map(|item| item.doc).collect::<Vec<_>>();
            return self.format_delimited_doc_with_limit(spec.open, spec.close, docs, inline_limit);
        }

        let mut body_parts = vec![self.leading_delimited_gap_doc(&leading_gap, true)];
        body_parts.extend(self.comma_sequence_body_parts(
            spec.node.clone(),
            items,
            commas,
            direct_boundary_layout,
        ));

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }

        self.append_sequence_trailing_doc(&mut body_parts, &trailing_gap, true, 1);

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

        let item_elements = items
            .iter()
            .map(|item| item.element.clone())
            .collect::<Vec<_>>();
        let owned = self.owned_sequence_trivia(
            range_start(node.text_range()),
            range_end(node.text_range()),
            &item_elements,
        );
        let mut requires_trivia_layout = gap_requires_trivia_layout(&owned.trailing);
        let commas = self
            .tokens(node.clone(), TokenKind::Comma)
            .collect::<Vec<_>>();
        let direct_boundary_layout = commas.len() + 1 == items.len();
        if direct_boundary_layout {
            requires_trivia_layout |=
                self.comma_sequence_requires_trivia_layout(node.clone(), &items, &commas);
        } else {
            for gap in &owned.between {
                requires_trivia_layout |= gap_requires_trivia_layout(gap);
            }
        }
        let trailing_gap = owned.trailing.clone();
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
        body_parts.extend(self.comma_sequence_body_parts(
            node.clone(),
            items,
            commas,
            direct_boundary_layout,
        ));

        if self.options.trailing_commas {
            body_parts.push(Doc::text(","));
        }
        self.append_sequence_trailing_doc(&mut body_parts, &trailing_gap, true, 1);
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

    fn comma_sequence_requires_trivia_layout(
        &self,
        node: SyntaxNode,
        items: &[DelimitedItemDoc],
        commas: &[SyntaxToken],
    ) -> bool {
        let mut previous_item = &items[0];
        for (comma_token, item) in commas.iter().zip(items.iter().skip(1)) {
            let before_gap = self
                .boundary_trivia(
                    node.clone(),
                    boundary_from_element_to_token(&previous_item.element, comma_token.clone()),
                )
                .unwrap_or_default();
            let after_gap = self
                .boundary_trivia(
                    node.clone(),
                    boundary_from_token_to_element(comma_token.clone(), &item.element),
                )
                .unwrap_or_default();
            if gap_requires_trivia_layout(&before_gap) || gap_requires_trivia_layout(&after_gap) {
                return true;
            }
            previous_item = item;
        }
        false
    }

    fn comma_sequence_body_parts(
        &self,
        node: SyntaxNode,
        items: Vec<DelimitedItemDoc>,
        commas: Vec<SyntaxToken>,
        direct_boundary_layout: bool,
    ) -> Vec<Doc> {
        let mut body_parts = Vec::new();
        let mut items = items.into_iter();
        let first_item = items.next().expect("non-empty comma-separated items");
        let mut previous_element = first_item.element.clone();
        body_parts.push(first_item.doc);

        if direct_boundary_layout {
            for (comma_token, item) in commas.into_iter().zip(items) {
                let before_gap = self
                    .boundary_trivia(
                        node.clone(),
                        boundary_from_element_to_token(&previous_element, comma_token.clone()),
                    )
                    .unwrap_or_default();
                let after_gap = self
                    .boundary_trivia(
                        node.clone(),
                        boundary_from_token_to_element(comma_token.clone(), &item.element),
                    )
                    .unwrap_or_default();
                body_parts.push(Doc::text(","));
                body_parts.push(self.gap_separator_doc(
                    &merge_boundary_gaps(before_gap, after_gap),
                    1,
                    true,
                    true,
                ));
                body_parts.push(item.doc);
                previous_element = item.element;
            }
        } else {
            let mut previous_end = range_end(first_item.range);
            for item in items {
                let gap = self.comment_gap(previous_end, range_start(item.range), true, true);
                body_parts.push(Doc::text(","));
                body_parts.push(self.gap_separator_doc(&gap, 1, true, true));
                body_parts.push(item.doc);
                previous_end = range_end(item.range);
            }
        }

        body_parts
    }

    pub(crate) fn raw(&self, node: SyntaxNode) -> String {
        let start = u32::from(node.text_range().start()) as usize;
        let end = u32::from(node.text_range().end()) as usize;
        self.source.get(start..end).unwrap_or("").trim().to_owned()
    }

    fn doc_renders_single_line(&self, doc: &Doc, indent: usize) -> bool {
        !self.render_fragment(doc, indent).contains('\n')
    }

    fn while_requires_raw_fallback(&self, while_expr: WhileExpr) -> bool {
        let Some(condition) = while_expr.condition() else {
            return self.node_has_unowned_comments(while_expr.syntax());
        };
        let Some(body) = while_expr.body() else {
            return self.node_has_unowned_comments(while_expr.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            while_expr.syntax(),
            &[TriviaBoundary::NodeNode(condition.syntax(), body.syntax())],
        )
    }

    fn loop_requires_raw_fallback(&self, loop_expr: LoopExpr) -> bool {
        let Some(body) = loop_expr.body() else {
            return self.node_has_unowned_comments(loop_expr.syntax());
        };
        let Some(loop_kw) = self.token(loop_expr.syntax(), TokenKind::LoopKw) else {
            return self.node_has_unowned_comments(loop_expr.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            loop_expr.syntax(),
            &[TriviaBoundary::TokenNode(loop_kw, body.syntax())],
        )
    }

    fn if_requires_raw_fallback(&self, if_expr: IfExpr) -> bool {
        let Some(condition) = if_expr.condition() else {
            return self.node_has_unowned_comments(if_expr.syntax());
        };
        let Some(then_branch) = if_expr.then_branch() else {
            return self.node_has_unowned_comments(if_expr.syntax());
        };
        let mut allowed_boundaries = vec![TriviaBoundary::NodeNode(
            condition.syntax(),
            then_branch.syntax(),
        )];

        if let Some(else_branch) = if_expr.else_branch() {
            let Some(else_kw) = self.token(else_branch.syntax(), TokenKind::ElseKw) else {
                return self.node_has_unowned_comments(if_expr.syntax());
            };
            allowed_boundaries.push(TriviaBoundary::NodeToken(then_branch.syntax(), else_kw));

            if self.else_branch_requires_raw_fallback(else_branch) {
                return true;
            }
        }

        self.node_has_unowned_comments_outside_boundaries(if_expr.syntax(), &allowed_boundaries)
    }

    fn else_branch_requires_raw_fallback(&self, else_branch: rhai_syntax::ElseBranch) -> bool {
        let Some(body) = else_branch.body() else {
            return self.node_has_unowned_comments(else_branch.syntax());
        };
        let Some(else_kw) = self.token(else_branch.syntax(), TokenKind::ElseKw) else {
            return self.node_has_unowned_comments(else_branch.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            else_branch.syntax(),
            &[TriviaBoundary::TokenNode(else_kw, body.syntax())],
        )
    }

    fn for_requires_raw_fallback(&self, for_expr: ForExpr) -> bool {
        let Some(iterable) = for_expr.iterable() else {
            return self.node_has_unowned_comments(for_expr.syntax());
        };
        let Some(body) = for_expr.body() else {
            return self.node_has_unowned_comments(for_expr.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            for_expr.syntax(),
            &[TriviaBoundary::NodeNode(iterable.syntax(), body.syntax())],
        ) || for_expr
            .bindings()
            .is_some_and(|bindings| self.for_bindings_requires_raw_fallback(bindings))
    }

    fn do_requires_raw_fallback(&self, do_expr: DoExpr) -> bool {
        let Some(body) = do_expr.body() else {
            return self.node_has_unowned_comments(do_expr.syntax());
        };
        let Some(do_kw) = self.token(do_expr.syntax(), TokenKind::DoKw) else {
            return self.node_has_unowned_comments(do_expr.syntax());
        };
        let Some(condition) = do_expr.condition() else {
            return self.node_has_unowned_comments_outside_boundaries(
                do_expr.syntax(),
                &[TriviaBoundary::TokenNode(do_kw, body.syntax())],
            );
        };
        let Some(condition_kw) = condition.keyword_token() else {
            return self.node_has_unowned_comments(do_expr.syntax())
                || self.node_has_unowned_comments(condition.syntax());
        };
        let Some(condition_expr) = condition.expr() else {
            return self.node_has_unowned_comments(do_expr.syntax())
                || self.node_has_unowned_comments(condition.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            do_expr.syntax(),
            &[
                TriviaBoundary::TokenNode(do_kw, body.syntax()),
                TriviaBoundary::NodeNode(body.syntax(), condition.syntax()),
            ],
        ) || self.node_has_unowned_comments_outside_boundaries(
            condition.syntax(),
            &[TriviaBoundary::TokenNode(
                condition_kw,
                condition_expr.syntax(),
            )],
        )
    }

    fn path_requires_raw_fallback(&self, path: PathExpr) -> bool {
        let segments = path.segments().collect::<Vec<_>>();
        let separators = self
            .tokens(path.syntax(), TokenKind::ColonColon)
            .collect::<Vec<_>>();
        let mut allowed_boundaries = Vec::new();

        let (segment_start_index, mut previous_token) = if path.base().is_some() {
            if separators.len() != segments.len() {
                return self.node_has_unowned_comments(path.syntax());
            }

            (0, None)
        } else if let Some(first) = segments.first() {
            if separators.len() + 1 != segments.len() {
                return self.node_has_unowned_comments(path.syntax());
            }

            (1, Some(first.clone()))
        } else {
            return self.node_has_unowned_comments(path.syntax());
        };

        let mut previous_node = path.base().map(|base| base.syntax());
        for (separator_token, segment) in separators
            .into_iter()
            .zip(segments.into_iter().skip(segment_start_index))
        {
            if let Some(previous_node) = previous_node {
                allowed_boundaries.push(TriviaBoundary::NodeToken(
                    previous_node,
                    separator_token.clone(),
                ));
            } else if let Some(previous_token) = previous_token {
                allowed_boundaries.push(TriviaBoundary::TokenToken(
                    previous_token,
                    separator_token.clone(),
                ));
            }
            allowed_boundaries.push(TriviaBoundary::TokenToken(separator_token, segment.clone()));
            previous_node = None;
            previous_token = Some(segment);
        }

        self.node_has_unowned_comments_outside_boundaries(path.syntax(), &allowed_boundaries)
    }

    fn index_requires_raw_fallback(&self, index: IndexExpr) -> bool {
        let Some(receiver) = index.receiver() else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(inner) = index.index() else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(open_token) = self
            .token(index.syntax(), TokenKind::QuestionOpenBracket)
            .or_else(|| self.token(index.syntax(), TokenKind::OpenBracket))
        else {
            return self.node_has_unowned_comments(index.syntax());
        };
        let Some(close_token) = self.token(index.syntax(), TokenKind::CloseBracket) else {
            return self.node_has_unowned_comments(index.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            index.syntax(),
            &[
                TriviaBoundary::NodeToken(receiver.syntax(), open_token.clone()),
                TriviaBoundary::TokenNode(open_token, inner.syntax()),
                TriviaBoundary::NodeToken(inner.syntax(), close_token),
            ],
        )
    }

    fn closure_requires_raw_fallback(&self, closure: ClosureExpr) -> bool {
        let mut allowed_boundaries = Vec::new();
        if let Some(params) = closure.params() {
            if self.closure_params_requires_raw_fallback(params.clone()) {
                return true;
            }

            if let Some(body) = closure.body() {
                allowed_boundaries.push(TriviaBoundary::NodeNode(params.syntax(), body.syntax()));
            }
        }

        self.node_has_unowned_comments_outside_boundaries(closure.syntax(), &allowed_boundaries)
    }

    fn closure_params_requires_raw_fallback(&self, params: ClosureParamList) -> bool {
        if self
            .token_range(params.syntax(), TokenKind::PipePipe)
            .is_some()
        {
            return false;
        }

        let names = params.params().collect::<Vec<_>>();
        let Some(open_token) = self.tokens(params.syntax(), TokenKind::Pipe).next() else {
            return self.node_has_unowned_comments(params.syntax());
        };
        let Some(close_token) = self.tokens(params.syntax(), TokenKind::Pipe).last() else {
            return self.node_has_unowned_comments(params.syntax());
        };

        if names.is_empty() {
            return self.node_has_unowned_comments_outside_boundaries(
                params.syntax(),
                &[TriviaBoundary::TokenToken(open_token, close_token)],
            );
        }

        let commas = self
            .tokens(params.syntax(), TokenKind::Comma)
            .collect::<Vec<_>>();
        if commas.len() + 1 != names.len() {
            return self.node_has_unowned_comments(params.syntax());
        }

        let mut allowed_boundaries = vec![TriviaBoundary::TokenToken(open_token, names[0].clone())];
        let mut previous_name = names[0].clone();
        for (comma_token, next_name) in commas.into_iter().zip(names.iter().skip(1)) {
            allowed_boundaries.push(TriviaBoundary::TokenToken(
                previous_name.clone(),
                comma_token.clone(),
            ));
            allowed_boundaries.push(TriviaBoundary::TokenToken(comma_token, next_name.clone()));
            previous_name = next_name.clone();
        }
        allowed_boundaries.push(TriviaBoundary::TokenToken(previous_name, close_token));

        self.node_has_unowned_comments_outside_boundaries(params.syntax(), &allowed_boundaries)
    }

    fn for_bindings_requires_raw_fallback(&self, bindings: ForBindings) -> bool {
        let names = bindings.names().collect::<Vec<_>>();
        match names.as_slice() {
            [] | [_] => self.node_has_unowned_comments(bindings.syntax()),
            [first, second] => {
                let Some(open_token) = self.token(bindings.syntax(), TokenKind::OpenParen) else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };
                let Some(comma_token) = self.token(bindings.syntax(), TokenKind::Comma) else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };
                let Some(close_token) = self.token(bindings.syntax(), TokenKind::CloseParen) else {
                    return self.node_has_unowned_comments(bindings.syntax());
                };

                self.node_has_unowned_comments_outside_boundaries(
                    bindings.syntax(),
                    &[
                        TriviaBoundary::TokenToken(open_token, first.clone()),
                        TriviaBoundary::TokenToken(first.clone(), comma_token.clone()),
                        TriviaBoundary::TokenToken(comma_token, second.clone()),
                        TriviaBoundary::TokenToken(second.clone(), close_token),
                    ],
                )
            }
            _ => self.node_has_unowned_comments(bindings.syntax()),
        }
    }

    fn object_field_requires_raw_fallback(&self, field: ObjectField) -> bool {
        let Some(name_token) = field.name_token() else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(colon_token) = self.token(field.syntax(), TokenKind::Colon) else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(value) = field.value() else {
            return self.node_has_unowned_comments(field.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            field.syntax(),
            &[
                TriviaBoundary::TokenToken(name_token, colon_token.clone()),
                TriviaBoundary::TokenNode(colon_token, value.syntax()),
            ],
        )
    }

    fn switch_arm_requires_raw_fallback(&self, arm: SwitchArm) -> bool {
        let Some(patterns) = arm.patterns() else {
            return self.node_has_unowned_comments(arm.syntax());
        };
        let Some(value) = arm.value() else {
            return self.node_has_unowned_comments(arm.syntax());
        };
        let Some(arrow_token) = self.token(arm.syntax(), TokenKind::FatArrow) else {
            return self.node_has_unowned_comments(arm.syntax());
        };

        self.switch_patterns_requires_raw_fallback(patterns.clone())
            || self.node_has_unowned_comments_outside_boundaries(
                arm.syntax(),
                &[TriviaBoundary::NodeToken(
                    patterns.syntax(),
                    arrow_token.clone(),
                )],
            )
            || self.node_has_unowned_comments_outside_boundaries(
                arm.syntax(),
                &[TriviaBoundary::TokenNode(arrow_token, value.syntax())],
            )
    }

    pub(crate) fn format_switch_patterns_doc(
        &self,
        patterns: SwitchPatternList,
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
            .tokens(patterns.syntax(), TokenKind::Pipe)
            .collect::<Vec<_>>();
        if separators.len() + 1 != values.len() {
            return Doc::text(self.raw(patterns.syntax()));
        }

        let mut doc = self.format_expr_doc(values[0].clone(), indent);
        let mut previous_value = values[0].clone();

        for (separator_token, next_value) in separators.into_iter().zip(values.into_iter().skip(1))
        {
            let next_doc = self.format_expr_doc(next_value.clone(), indent);
            let before_gap = self
                .boundary_trivia(
                    patterns.syntax(),
                    TriviaBoundary::NodeToken(previous_value.syntax(), separator_token.clone()),
                )
                .unwrap_or_default();
            let after_gap = self
                .boundary_trivia(
                    patterns.syntax(),
                    TriviaBoundary::TokenNode(separator_token, next_value.syntax()),
                )
                .unwrap_or_default();
            if before_gap.has_comments() || after_gap.has_comments() {
                doc = Doc::concat(vec![
                    doc,
                    self.tight_comment_gap_from_gap(&before_gap, false),
                    Doc::text("|"),
                    self.space_or_tight_gap_from_gap(&after_gap),
                    next_doc,
                ]);
            } else {
                doc = Doc::group(Doc::concat(vec![
                    doc,
                    Doc::indent(
                        1,
                        Doc::concat(vec![Doc::soft_line(), Doc::text("| "), next_doc]),
                    ),
                ]));
            }

            previous_value = next_value;
        }

        doc
    }

    fn switch_patterns_requires_raw_fallback(&self, patterns: SwitchPatternList) -> bool {
        if patterns.wildcard_token().is_some() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let values = patterns.exprs().collect::<Vec<_>>();
        if values.is_empty() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let separators = self
            .tokens(patterns.syntax(), TokenKind::Pipe)
            .collect::<Vec<_>>();
        if separators.len() + 1 != values.len() {
            return self.node_has_unowned_comments(patterns.syntax());
        }

        let mut allowed_boundaries = Vec::new();
        let mut previous_value = values[0].clone();
        for (separator_token, next_value) in separators.into_iter().zip(values.iter().skip(1)) {
            allowed_boundaries.push(TriviaBoundary::NodeToken(
                previous_value.syntax(),
                separator_token.clone(),
            ));
            allowed_boundaries.push(TriviaBoundary::TokenNode(
                separator_token,
                next_value.syntax(),
            ));
            previous_value = next_value.clone();
        }

        self.node_has_unowned_comments_outside_boundaries(patterns.syntax(), &allowed_boundaries)
    }

    pub(crate) fn format_else_branch_doc(&self, else_branch: ElseBranch, indent: usize) -> Doc {
        let else_body = else_branch.body();
        let else_kw = self.token(else_branch.syntax(), TokenKind::ElseKw);
        let else_body_start = else_body
            .as_ref()
            .map(|body| u32::from(body.syntax().text_range().start()) as usize)
            .unwrap_or_else(|| u32::from(else_branch.syntax().text_range().end()) as usize);
        let else_kw_end = self
            .token_range(else_branch.syntax(), TokenKind::ElseKw)
            .map(range_end)
            .unwrap_or(else_body_start);

        Doc::concat(vec![
            Doc::text("else"),
            match (else_kw, else_body.clone()) {
                (Some(else_kw), Some(else_body)) => self.head_body_separator_for_boundary(
                    else_branch.syntax(),
                    TriviaBoundary::TokenNode(else_kw, else_body.syntax()),
                ),
                _ => self.head_body_separator_doc(else_kw_end, else_body_start),
            },
            match else_body {
                Some(Expr::If(nested_if)) => self.format_if_expr_doc(nested_if, indent),
                Some(Expr::Block(block)) => self.format_block_doc(block, indent),
                Some(other) => self.format_expr_doc(other, indent),
                None => Doc::text("{}"),
            },
        ])
    }

    fn field_requires_raw_fallback(&self, field: FieldExpr) -> bool {
        let Some(receiver) = field.receiver() else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(accessor_token) = self
            .token(field.syntax(), TokenKind::QuestionDot)
            .or_else(|| self.token(field.syntax(), TokenKind::Dot))
        else {
            return self.node_has_unowned_comments(field.syntax());
        };
        let Some(name_token) = field.name_token() else {
            return self.node_has_unowned_comments(field.syntax());
        };

        self.node_has_unowned_comments_outside_boundaries(
            field.syntax(),
            &[
                TriviaBoundary::NodeToken(receiver.syntax(), accessor_token.clone()),
                TriviaBoundary::TokenToken(accessor_token, name_token),
            ],
        )
    }

    fn collect_binary_chain(
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

    fn group_keyword_clause_doc(&self, keyword: &str, clause: Doc) -> Doc {
        Doc::group(Doc::concat(vec![
            Doc::text(keyword),
            Doc::indent(1, Doc::concat(vec![Doc::soft_line(), clause])),
        ]))
    }

    fn soft_or_tight_gap_from_gap(&self, gap: &GapTrivia) -> Doc {
        if gap.has_comments() {
            self.tight_comment_gap_from_gap(gap, true)
        } else {
            Doc::soft_line()
        }
    }
}

#[derive(Debug, Clone)]
struct DelimitedItemDoc {
    element: SyntaxElement,
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

fn boundary_from_element_to_token(element: &SyntaxElement, token: SyntaxToken) -> TriviaBoundary {
    match element {
        NodeOrToken::Node(node) => TriviaBoundary::NodeToken(node.clone(), token),
        NodeOrToken::Token(previous) => TriviaBoundary::TokenToken(previous.clone(), token),
    }
}

fn boundary_from_token_to_element(token: SyntaxToken, element: &SyntaxElement) -> TriviaBoundary {
    match element {
        NodeOrToken::Node(node) => TriviaBoundary::TokenNode(token, node.clone()),
        NodeOrToken::Token(next) => TriviaBoundary::TokenToken(token, next.clone()),
    }
}

fn merge_boundary_gaps(mut before: GapTrivia, after: GapTrivia) -> GapTrivia {
    before.leading_comments.extend(after.leading_comments);
    before.dangling_comments.extend(after.dangling_comments);
    before.trailing_blank_lines_before_next = before
        .trailing_blank_lines_before_next
        .max(after.trailing_blank_lines_before_next);
    before
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
