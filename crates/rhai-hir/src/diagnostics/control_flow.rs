use crate::model::{
    BinaryOperator, BodyId, ControlFlowKind, DoConditionKind, ExprId, ExprKind, FileHir,
    SemanticDiagnostic, SemanticDiagnosticCode, SemanticDiagnosticKind, SymbolId, SymbolKind,
    UnaryOperator, ValueFlowKind,
};

impl FileHir {
    pub(crate) fn constant_condition_diagnostics(&self) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for if_expr in &self.if_exprs {
            let Some(condition) = if_expr.condition else {
                continue;
            };
            let Some(value) = self.static_bool_condition(condition, &mut Vec::new()) else {
                continue;
            };

            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::ConstantCondition,
                code: SemanticDiagnosticCode::ConstantCondition,
                range: self.expr(condition).range,
                message: if value {
                    "if condition is always true".to_owned()
                } else {
                    "if condition is always false".to_owned()
                },
                related_range: None,
            });
        }

        for while_expr in &self.while_exprs {
            let Some(condition) = while_expr.condition else {
                continue;
            };
            let Some(value) = self.static_bool_condition(condition, &mut Vec::new()) else {
                continue;
            };
            if value
                && while_expr
                    .body
                    .is_some_and(|body| self.loop_body_has_break(body))
            {
                continue;
            }

            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::ConstantCondition,
                code: SemanticDiagnosticCode::ConstantCondition,
                range: self.expr(condition).range,
                message: if value {
                    "while condition is always true".to_owned()
                } else {
                    "while condition is always false".to_owned()
                },
                related_range: None,
            });
        }

        for do_expr in &self.do_exprs {
            let Some(condition_kind) = do_expr.condition_kind else {
                continue;
            };
            let Some(condition) = do_expr.condition else {
                continue;
            };
            let Some(value) = self.static_bool_condition(condition, &mut Vec::new()) else {
                continue;
            };
            if condition_repeats_forever(condition_kind, value)
                && do_expr
                    .body
                    .is_some_and(|body| self.loop_body_has_break(body))
            {
                continue;
            }

            diagnostics.push(SemanticDiagnostic {
                kind: SemanticDiagnosticKind::ConstantCondition,
                code: SemanticDiagnosticCode::ConstantCondition,
                range: self.expr(condition).range,
                message: match condition_kind {
                    DoConditionKind::While if value => {
                        "do-while condition is always true".to_owned()
                    }
                    DoConditionKind::While => "do-while condition is always false".to_owned(),
                    DoConditionKind::Until if value => {
                        "do-until condition is always true".to_owned()
                    }
                    DoConditionKind::Until => "do-until condition is always false".to_owned(),
                },
                related_range: None,
            });
        }

        diagnostics
    }

    fn static_bool_condition(
        &self,
        expr: ExprId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<bool> {
        match self.expr(expr).kind {
            ExprKind::Literal => self.literal(expr).and_then(|literal| {
                (literal.kind == crate::LiteralKind::Bool)
                    .then_some(literal.text.as_deref())
                    .flatten()
                    .map(|text| text == "true")
            }),
            ExprKind::Name => self
                .reference_at(self.expr(expr).range)
                .and_then(|reference| self.definition_of(reference))
                .and_then(|symbol| self.static_bool_condition_for_symbol(symbol, visited_symbols)),
            ExprKind::Paren => largest_inner_expr(self, expr)
                .and_then(|inner| self.static_bool_condition(inner, visited_symbols)),
            ExprKind::Block => self.block_expr(expr).and_then(|block| {
                self.body_tail_value(block.body)
                    .and_then(|tail| self.static_bool_condition(tail, visited_symbols))
            }),
            ExprKind::Unary => self.unary_expr(expr).and_then(|unary| {
                (unary.operator == UnaryOperator::Not)
                    .then_some(unary.operand)
                    .flatten()
                    .and_then(|operand| self.static_bool_condition(operand, visited_symbols))
                    .map(|value| !value)
            }),
            ExprKind::Binary => self.binary_expr(expr).and_then(|binary| {
                let lhs = binary
                    .lhs
                    .and_then(|lhs| self.static_bool_condition(lhs, visited_symbols))?;
                let rhs = binary
                    .rhs
                    .and_then(|rhs| self.static_bool_condition(rhs, visited_symbols))?;

                match binary.operator {
                    BinaryOperator::AndAnd => Some(lhs && rhs),
                    BinaryOperator::OrOr => Some(lhs || rhs),
                    BinaryOperator::EqEq => Some(lhs == rhs),
                    BinaryOperator::NotEq => Some(lhs != rhs),
                    _ => None,
                }
            }),
            _ => None,
        }
    }

    fn static_bool_condition_for_symbol(
        &self,
        symbol: SymbolId,
        visited_symbols: &mut Vec<SymbolId>,
    ) -> Option<bool> {
        if visited_symbols.contains(&symbol) {
            return None;
        }
        visited_symbols.push(symbol);

        let result = match self.symbol(symbol).kind {
            SymbolKind::Variable | SymbolKind::Constant => {
                let initializers = self
                    .value_flows_into(symbol)
                    .filter(|flow| flow.kind == ValueFlowKind::Initializer)
                    .collect::<Vec<_>>();
                let has_assignments = self
                    .value_flows_into(symbol)
                    .any(|flow| flow.kind == ValueFlowKind::Assignment);

                (!has_assignments && initializers.len() == 1)
                    .then(|| self.static_bool_condition(initializers[0].expr, visited_symbols))
                    .flatten()
            }
            _ => None,
        };

        visited_symbols.pop();
        result
    }

    fn loop_body_has_break(&self, body: BodyId) -> bool {
        let loop_scope = self.body(body).scope;
        self.body_control_flow(body).any(|event| {
            event.kind == ControlFlowKind::Break && event.target_loop == Some(loop_scope)
        })
    }
}

fn condition_repeats_forever(kind: DoConditionKind, value: bool) -> bool {
    matches!(
        (kind, value),
        (DoConditionKind::While, true) | (DoConditionKind::Until, false)
    )
}

fn largest_inner_expr(hir: &FileHir, expr: ExprId) -> Option<ExprId> {
    let range = hir.expr(expr).range;
    hir.exprs
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            let candidate = ExprId(*index as u32);
            candidate != expr
                && node.range.start() >= range.start()
                && node.range.end() <= range.end()
                && node.range != range
        })
        .max_by_key(|(_, node)| node.range.len())
        .map(|(index, _)| ExprId(index as u32))
}
