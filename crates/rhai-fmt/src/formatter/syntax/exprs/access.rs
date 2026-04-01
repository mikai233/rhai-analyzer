use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::syntax::exprs::DelimitedNodeSpec;

impl Formatter<'_> {
    pub(crate) fn format_path_doc(&self, path: PathExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_call_doc(&self, call: CallExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_arg_list_doc(&self, args: ArgList, indent: usize) -> Doc {
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

    pub(crate) fn format_index_doc(&self, index: IndexExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_field_doc(&self, field: FieldExpr, indent: usize) -> Doc {
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

}
