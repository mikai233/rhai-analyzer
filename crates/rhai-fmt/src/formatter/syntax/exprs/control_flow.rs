use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::syntax::exprs::{DelimitedItemDoc, range_end, range_start};
use crate::formatter::trivia::comments::GapSeparatorOptions;

impl Formatter<'_> {
    pub(crate) fn format_switch_arm_list_body_doc(
        &self,
        arms: SwitchArmList,
        indent: usize,
    ) -> Doc {
        let item_docs = self.switch_arm_list_items(arms.clone(), indent);
        self.format_comma_separated_body_doc(&arms.syntax(), item_docs)
    }

    pub(crate) fn switch_arm_list_items(
        &self,
        arms: SwitchArmList,
        indent: usize,
    ) -> Vec<DelimitedItemDoc> {
        arms.arms()
            .map(|arm| DelimitedItemDoc {
                element: arm.syntax().into(),
                range: arm.syntax().text_range(),
                doc: self.format_switch_arm_doc(arm, indent),
            })
            .collect()
    }

    pub(crate) fn format_if_expr_doc(&self, if_expr: IfExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_switch_expr_doc(&self, switch_expr: SwitchExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_switch_arm_doc(&self, arm: SwitchArm, indent: usize) -> Doc {
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

    pub(crate) fn format_while_expr_doc(&self, while_expr: WhileExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_loop_expr_doc(&self, loop_expr: LoopExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_for_expr_doc(&self, for_expr: ForExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_do_expr_doc(&self, do_expr: DoExpr, indent: usize) -> Doc {
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

    pub(crate) fn format_closure_expr_doc(&self, closure: ClosureExpr, indent: usize) -> Doc {
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
}
