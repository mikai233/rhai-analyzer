use rhai_syntax::*;

use crate::formatter::Formatter;

impl Formatter<'_> {
    pub(crate) fn call_requires_raw_fallback(&self, call: CallExpr) -> bool {
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

    pub(crate) fn unary_requires_raw_fallback(&self, unary: rhai_syntax::UnaryExpr) -> bool {
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

    pub(crate) fn binary_requires_raw_fallback(&self, binary: BinaryExpr) -> bool {
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

    pub(crate) fn assign_requires_raw_fallback(&self, assign: rhai_syntax::AssignExpr) -> bool {
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

    pub(crate) fn paren_requires_raw_fallback(&self, paren: ParenExpr) -> bool {
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

    pub(crate) fn while_requires_raw_fallback(&self, while_expr: WhileExpr) -> bool {
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

    pub(crate) fn loop_requires_raw_fallback(&self, loop_expr: LoopExpr) -> bool {
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

    pub(crate) fn if_requires_raw_fallback(&self, if_expr: IfExpr) -> bool {
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

    pub(crate) fn else_branch_requires_raw_fallback(
        &self,
        else_branch: rhai_syntax::ElseBranch,
    ) -> bool {
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

    pub(crate) fn for_requires_raw_fallback(&self, for_expr: ForExpr) -> bool {
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

    pub(crate) fn do_requires_raw_fallback(&self, do_expr: DoExpr) -> bool {
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

    pub(crate) fn path_requires_raw_fallback(&self, path: PathExpr) -> bool {
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

    pub(crate) fn index_requires_raw_fallback(&self, index: IndexExpr) -> bool {
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

    pub(crate) fn closure_requires_raw_fallback(&self, closure: ClosureExpr) -> bool {
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

    pub(crate) fn closure_params_requires_raw_fallback(&self, params: ClosureParamList) -> bool {
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

    pub(crate) fn for_bindings_requires_raw_fallback(&self, bindings: ForBindings) -> bool {
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

    pub(crate) fn object_field_requires_raw_fallback(&self, field: ObjectField) -> bool {
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

    pub(crate) fn switch_arm_requires_raw_fallback(&self, arm: SwitchArm) -> bool {
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

    pub(crate) fn switch_patterns_requires_raw_fallback(
        &self,
        patterns: SwitchPatternList,
    ) -> bool {
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

    pub(crate) fn field_requires_raw_fallback(&self, field: FieldExpr) -> bool {
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
}
