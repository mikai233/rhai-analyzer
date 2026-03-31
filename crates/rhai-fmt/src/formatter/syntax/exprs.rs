use rhai_syntax::{
    ArrayExpr, AstNode, BinaryExpr, CallExpr, ClosureExpr, ClosureParamList, DoExpr, Expr,
    FieldExpr, ForBindings, ForExpr, IfExpr, IndexExpr, InterpolatedStringExpr, LoopExpr,
    ObjectExpr, ObjectField, ParamList, ParenExpr, PathExpr, StringPart, SwitchArm, SwitchExpr,
    SyntaxNode, TextRange, TokenKind, WhileExpr,
};

use crate::ContainerLayoutStyle;
use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};
use crate::formatter::trivia::comments::GapTrivia;

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
            Expr::Unary(unary) => {
                let operator = unary
                    .operator_token()
                    .map(|token| token.text(self.source))
                    .unwrap_or("");
                let inner = unary
                    .expr()
                    .map(|expr| self.format_expr_doc(expr, indent))
                    .unwrap_or_else(|| Doc::text(self.raw(expr.syntax())));
                if self.doc_renders_single_line(&inner, indent) {
                    self.group_tight_suffix_doc(Doc::text(operator), inner)
                } else {
                    Doc::concat(vec![Doc::text(operator), inner])
                }
            }
            Expr::Binary(binary) => self.format_binary_doc(binary, indent),
            Expr::Assign(assign) => {
                let lhs = assign
                    .lhs()
                    .map(|lhs| self.format_expr_doc(lhs, indent))
                    .unwrap_or_else(|| Doc::text(""));
                let rhs = assign
                    .rhs()
                    .map(|rhs| self.format_expr_doc(rhs, indent))
                    .unwrap_or_else(|| Doc::text(""));
                let operator = assign
                    .operator_token()
                    .map(|token| token.text(self.source))
                    .unwrap_or("=");
                if self.doc_renders_single_line(&lhs, indent)
                    && self.doc_renders_single_line(&rhs, indent)
                {
                    self.group_spaced_suffix_doc(lhs, format!("{operator} "), rhs)
                } else {
                    Doc::concat(vec![lhs, Doc::text(format!(" {operator} ")), rhs])
                }
            }
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
            Expr::If(if_expr) => {
                self.node_has_unowned_comments(expr.syntax())
                    || if_expr
                        .else_branch()
                        .is_some_and(|branch| self.node_has_unowned_comments(branch.syntax()))
            }
            Expr::Switch(_) => false,
            Expr::While(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Loop(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::For(for_expr) => {
                self.node_has_unowned_comments(expr.syntax())
                    || for_expr
                        .bindings()
                        .is_some_and(|bindings| self.node_has_comments(bindings.syntax()))
            }
            Expr::Do(do_expr) => {
                self.node_has_unowned_comments(expr.syntax())
                    || do_expr
                        .condition()
                        .is_some_and(|condition| self.node_has_unowned_comments(condition.syntax()))
            }
            Expr::Path(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Closure(closure) => {
                self.node_has_unowned_comments(expr.syntax())
                    || closure
                        .params()
                        .is_some_and(|params| self.node_has_comments(params.syntax()))
            }
            Expr::InterpolatedString(string) => {
                self.node_has_unowned_comments(expr.syntax())
                    || string.parts().any(|part| match part {
                        StringPart::Segment(_) => false,
                        StringPart::Interpolation(interpolation) => interpolation
                            .body()
                            .is_some_and(|body| self.node_has_unowned_comments(body.syntax())),
                    })
            }
            Expr::Unary(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Binary(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Assign(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Paren(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Call(call) => self.call_requires_raw_fallback(call),
            Expr::Index(_) => self.node_has_unowned_comments(expr.syntax()),
            Expr::Field(_) => self.node_has_unowned_comments(expr.syntax()),
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
        let Some(args_open_start) = self
            .token_range(call.syntax(), TokenKind::OpenParen)
            .map(range_start)
        else {
            return self.node_has_unowned_comments(call.syntax());
        };

        let callee_end = u32::from(callee.syntax().range().end()) as usize;
        self.range_has_comments(callee_end, args_open_start)
    }

    fn format_path_doc(&self, path: PathExpr<'_>, indent: usize) -> Doc {
        let mut segments = path.segments();
        let mut doc = if let Some(base) = path.base() {
            self.format_expr_doc(base, indent)
        } else if let Some(first) = segments.next() {
            Doc::text(first.text(self.source))
        } else {
            Doc::text(self.raw(path.syntax()))
        };

        for segment in segments {
            doc = self
                .group_tight_suffix_doc(doc, Doc::text(format!("::{}", segment.text(self.source))));
        }

        doc
    }

    fn format_binary_doc(&self, binary: BinaryExpr<'_>, indent: usize) -> Doc {
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

    fn format_paren_doc(&self, paren: ParenExpr<'_>, indent: usize) -> Doc {
        let inner = paren
            .expr()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        Doc::concat(vec![Doc::text("("), inner, Doc::text(")")])
    }

    fn format_call_doc(&self, call: CallExpr<'_>, indent: usize) -> Doc {
        let callee = call
            .callee()
            .map(|callee| self.format_expr_doc(callee, indent))
            .unwrap_or_else(|| Doc::text(""));
        let bang = if call.uses_caller_scope() { "!" } else { "" };
        let args = self.format_arg_list_doc(call, indent);
        Doc::concat(vec![callee, Doc::text(bang), args])
    }

    fn format_arg_list_doc(&self, call: CallExpr<'_>, indent: usize) -> Doc {
        let values = call
            .args()
            .map(|args| {
                args.args()
                    .map(|expr| DelimitedItemDoc {
                        range: expr.syntax().range(),
                        doc: self.format_expr_doc(expr, indent),
                    })
                    .collect::<Vec<_>>()
            })
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

    fn format_array_doc(&self, array: ArrayExpr<'_>, indent: usize) -> Doc {
        let items = array
            .items()
            .map(|items| {
                items
                    .exprs()
                    .map(|expr| DelimitedItemDoc {
                        range: expr.syntax().range(),
                        doc: self.format_expr_doc(expr, indent + 1),
                    })
                    .collect::<Vec<_>>()
            })
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
        let receiver = index
            .receiver()
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let inner = index
            .index()
            .map(|inner| self.format_expr_doc(inner, indent))
            .unwrap_or_else(|| Doc::text(""));
        let open = if self
            .token_range(index.syntax(), TokenKind::QuestionOpenBracket)
            .is_some()
        {
            "?["
        } else {
            "["
        };
        self.group_tight_suffix_doc(
            receiver,
            Doc::group(Doc::concat(vec![
                Doc::text(open),
                Doc::indent(1, Doc::concat(vec![Doc::line(), inner])),
                Doc::line(),
                Doc::text("]"),
            ])),
        )
    }

    fn format_field_doc(&self, field: FieldExpr<'_>, indent: usize) -> Doc {
        let receiver = field
            .receiver()
            .map(|receiver| self.format_expr_doc(receiver, indent))
            .unwrap_or_else(|| Doc::text(""));
        let name = field
            .name_token()
            .map(|name| name.text(self.source).to_owned())
            .unwrap_or_default();
        let accessor = if self
            .token_range(field.syntax(), TokenKind::QuestionDot)
            .is_some()
        {
            "?."
        } else {
            "."
        };
        self.group_tight_suffix_doc(receiver, Doc::text(format!("{accessor}{name}")))
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
        if self.node_has_unowned_comments(field.syntax()) {
            return Doc::text(self.raw(field.syntax()));
        }

        let name = field
            .name_token()
            .map(|token| token.text(self.source).to_owned())
            .unwrap_or_default();
        let value = field
            .value()
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        Doc::concat(vec![Doc::text(format!("{name}: ")), value])
    }

    fn format_if_expr_doc(&self, if_expr: IfExpr<'_>, indent: usize) -> Doc {
        let condition = if_expr
            .condition()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let then_branch = if_expr
            .then_branch()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let mut parts = vec![Doc::text("if "), condition, Doc::text(" "), then_branch];

        if let Some(else_body) = if_expr.else_branch().and_then(|branch| branch.body()) {
            parts.push(Doc::text(" else "));
            parts.push(match else_body {
                Expr::If(nested_if) => self.format_if_expr_doc(nested_if, indent),
                Expr::Block(block) => self.format_block_doc(block, indent),
                other => self.format_expr_doc(other, indent),
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
        let leading_gap = self.comment_gap(open_brace_end, first_arm_start, false);

        if arms.is_empty() && leading_gap.line_comments.is_empty() {
            return Doc::concat(vec![Doc::text("switch "), scrutinee, Doc::text(" {}")]);
        }

        let mut body_parts = Vec::new();
        if !leading_gap.line_comments.is_empty() {
            body_parts.push(self.render_line_comments_doc(&leading_gap.line_comments));

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
                let gap = self.comment_gap(cursor, arm_start, index > 0 || !body_parts.is_empty());
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

        let trailing_gap = self.comment_gap(cursor, close_brace_start, !arms.is_empty());
        if !arms.is_empty()
            && (!trailing_gap.trailing_comments.is_empty()
                || !trailing_gap.line_comments.is_empty())
        {
            body_parts.push(self.gap_separator_doc(&trailing_gap, 1, true, false));
        } else if !trailing_gap.line_comments.is_empty() {
            body_parts.push(self.render_line_comments_doc(&trailing_gap.line_comments));
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
        if self.node_has_unowned_comments(arm.syntax()) {
            return Doc::text(self.raw(arm.syntax()));
        }

        let patterns = arm
            .patterns()
            .map(|patterns| {
                let values = patterns
                    .exprs()
                    .map(|expr| self.format_expr_doc(expr, indent))
                    .collect::<Vec<_>>();
                if patterns.wildcard_token().is_some() {
                    Doc::text("_")
                } else {
                    Doc::join(values, Doc::text(" | "))
                }
            })
            .unwrap_or_else(|| Doc::text("_"));
        let value = arm
            .value()
            .map(|value| self.format_expr_doc(value, indent))
            .unwrap_or_else(|| Doc::text(""));
        Doc::concat(vec![patterns, Doc::text(" => "), value])
    }

    fn format_while_expr_doc(&self, while_expr: WhileExpr<'_>, indent: usize) -> Doc {
        let condition = while_expr
            .condition()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body = while_expr
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        Doc::concat(vec![Doc::text("while "), condition, Doc::text(" "), body])
    }

    fn format_loop_expr_doc(&self, loop_expr: LoopExpr<'_>, indent: usize) -> Doc {
        let body = loop_expr
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        Doc::concat(vec![Doc::text("loop "), body])
    }

    fn format_for_expr_doc(&self, for_expr: ForExpr<'_>, indent: usize) -> Doc {
        let bindings = self.format_for_bindings_doc(for_expr.bindings());
        let iterable = for_expr
            .iterable()
            .map(|expr| self.format_expr_doc(expr, indent))
            .unwrap_or_else(|| Doc::text(""));
        let body = for_expr
            .body()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        Doc::concat(vec![
            Doc::text("for "),
            bindings,
            Doc::text(" in "),
            iterable,
            Doc::text(" "),
            body,
        ])
    }

    fn format_for_bindings_doc(&self, bindings: Option<ForBindings<'_>>) -> Doc {
        let names = bindings
            .map(|bindings| {
                bindings
                    .names()
                    .map(|name| Doc::text(name.text(self.source)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match names.len() {
            0 => Doc::text("_"),
            1 => names.into_iter().next().unwrap_or_else(|| Doc::text("_")),
            _ => Doc::concat(vec![
                Doc::text("("),
                Doc::join(names, Doc::text(", ")),
                Doc::text(")"),
            ]),
        }
    }

    fn format_do_expr_doc(&self, do_expr: DoExpr<'_>, indent: usize) -> Doc {
        let body = do_expr
            .body()
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
        Doc::concat(vec![
            Doc::text("do "),
            body,
            Doc::text(format!(" {keyword} ")),
            expr,
        ])
    }

    fn format_closure_expr_doc(&self, closure: ClosureExpr<'_>, indent: usize) -> Doc {
        let params = self.format_closure_params_doc(closure.params());
        let body = closure
            .body()
            .map(|body| self.format_expr_doc(body, indent))
            .unwrap_or_else(|| Doc::text(""));
        Doc::concat(vec![params, Doc::text(" "), body])
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

    fn format_closure_params_doc(&self, params: Option<ClosureParamList<'_>>) -> Doc {
        let names = params
            .map(|params| {
                params
                    .params()
                    .map(|param| Doc::text(param.text(self.source)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Doc::concat(vec![
            Doc::text("|"),
            Doc::join(names, Doc::text(", ")),
            Doc::text("|"),
        ])
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
            let gap = self.comment_gap(open_end, close_start, false);
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

        let leading_gap = self.comment_gap(open_end, range_start(items[0].range), false);
        let mut requires_trivia_layout = gap_requires_trivia_layout(&leading_gap);
        let mut cursor = range_end(items[0].range);

        for item in items.iter().skip(1) {
            let gap = self.comment_gap(cursor, range_start(item.range), true);
            requires_trivia_layout |= gap_requires_trivia_layout(&gap);
            cursor = range_end(item.range);
        }

        let trailing_gap = self.comment_gap(cursor, close_start, true);
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
            let gap = self.comment_gap(previous_end, range_start(item.range), true);
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

    fn leading_delimited_gap_doc(&self, gap: &GapTrivia, include_terminal_newline: bool) -> Doc {
        if !gap.line_comments.is_empty() {
            let mut parts = vec![hard_lines(gap.line_comments[0].blank_lines_before + 1)];
            parts.push(self.render_line_comments_doc(&gap.line_comments));

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
        || !gap.line_comments.is_empty()
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
