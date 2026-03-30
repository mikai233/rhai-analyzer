use rhai_syntax::{
    ArgList, ArrayExpr, AstNode, BinaryExpr, CallExpr, ClosureExpr, ClosureParamList, DoExpr, Expr,
    FieldExpr, ForBindings, ForExpr, IfExpr, IndexExpr, InterpolatedStringExpr, LoopExpr,
    ObjectExpr, ObjectField, ParamList, ParenExpr, PathExpr, SwitchArm, SwitchExpr, SyntaxNode,
    TokenKind, WhileExpr,
};

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;
use crate::formatter::support::utils::contains_token;

impl Formatter<'_> {
    pub(crate) fn format_expr(&self, expr: Expr<'_>, indent: usize) -> String {
        match expr {
            Expr::Name(name) => name
                .token()
                .map(|token| token.text(self.source).to_owned())
                .unwrap_or_else(|| self.raw(expr.syntax())),
            Expr::Literal(literal) => literal
                .token()
                .map(|token| token.text(self.source).to_owned())
                .unwrap_or_else(|| self.raw(expr.syntax())),
            Expr::Array(array) => self.format_array(array, indent),
            Expr::Object(object) => self.format_object(object, indent),
            Expr::If(if_expr) => self.format_if_expr(if_expr, indent),
            Expr::Switch(switch_expr) => self.format_switch_expr(switch_expr, indent),
            Expr::While(while_expr) => self.format_while_expr(while_expr, indent),
            Expr::Loop(loop_expr) => self.format_loop_expr(loop_expr, indent),
            Expr::For(for_expr) => self.format_for_expr(for_expr, indent),
            Expr::Do(do_expr) => self.format_do_expr(do_expr, indent),
            Expr::Path(path) => self.format_path(path, indent),
            Expr::Closure(closure) => self.format_closure_expr(closure, indent),
            Expr::InterpolatedString(string) => self.format_interpolated_string(string),
            Expr::Unary(unary) => {
                let operator = unary
                    .operator_token()
                    .map(|token| token.text(self.source))
                    .unwrap_or("");
                let inner = unary
                    .expr()
                    .map(|expr| self.format_expr(expr, indent))
                    .unwrap_or_else(|| self.raw(expr.syntax()));
                format!("{operator}{inner}")
            }
            Expr::Binary(binary) => self.format_binary(binary, indent),
            Expr::Assign(assign) => {
                let lhs = assign
                    .lhs()
                    .map(|lhs| self.format_expr(lhs, indent))
                    .unwrap_or_default();
                let rhs = assign
                    .rhs()
                    .map(|rhs| self.format_expr(rhs, indent))
                    .unwrap_or_default();
                let operator = assign
                    .operator_token()
                    .map(|token| token.text(self.source))
                    .unwrap_or("=");
                format!("{lhs} {operator} {rhs}")
            }
            Expr::Paren(paren) => self.format_paren(paren, indent),
            Expr::Call(call) => self.format_call(call, indent),
            Expr::Index(index) => self.format_index(index, indent),
            Expr::Field(field) => self.format_field(field, indent),
            Expr::Block(block) => self.format_block(block, indent),
            _ => self.raw(expr.syntax()),
        }
    }

    fn format_path(&self, path: PathExpr<'_>, indent: usize) -> String {
        let mut parts = Vec::<String>::new();
        if let Some(base) = path.base() {
            parts.push(self.format_expr(base, indent));
        }
        parts.extend(
            path.segments()
                .map(|segment| segment.text(self.source).to_owned()),
        );
        if parts.is_empty() {
            self.raw(path.syntax())
        } else {
            parts.join("::")
        }
    }

    fn format_binary(&self, binary: BinaryExpr<'_>, indent: usize) -> String {
        let lhs = binary
            .lhs()
            .map(|lhs| self.format_expr(lhs, indent))
            .unwrap_or_default();
        let rhs = binary
            .rhs()
            .map(|rhs| self.format_expr(rhs, indent))
            .unwrap_or_default();
        let operator = binary
            .operator_token()
            .map(|token| token.text(self.source))
            .unwrap_or("");
        format!("{lhs} {operator} {rhs}")
    }

    fn format_paren(&self, paren: ParenExpr<'_>, indent: usize) -> String {
        let inner = paren
            .expr()
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        format!("({inner})")
    }

    fn format_call(&self, call: CallExpr<'_>, indent: usize) -> String {
        let callee = call
            .callee()
            .map(|callee| self.format_expr(callee, indent))
            .unwrap_or_default();
        let bang = if call.uses_caller_scope() { "!" } else { "" };
        let args = self.format_arg_list(call.args(), indent);
        format!("{callee}{bang}{args}")
    }

    fn format_arg_list(&self, args: Option<ArgList<'_>>, indent: usize) -> String {
        let Some(args) = args else {
            return "()".to_owned();
        };
        let values = args
            .args()
            .map(|expr| self.format_expr(expr, indent))
            .collect::<Vec<_>>();
        self.render_fragment(&self.format_delimited_doc("(", ")", values), indent)
    }

    fn format_index(&self, index: IndexExpr<'_>, indent: usize) -> String {
        if contains_token(index.syntax(), TokenKind::QuestionOpenBracket) {
            return self.raw(index.syntax());
        }

        let receiver = index
            .receiver()
            .map(|receiver| self.format_expr(receiver, indent))
            .unwrap_or_default();
        let inner = index
            .index()
            .map(|inner| self.format_expr(inner, indent))
            .unwrap_or_default();
        format!("{receiver}[{inner}]")
    }

    fn format_field(&self, field: FieldExpr<'_>, indent: usize) -> String {
        if contains_token(field.syntax(), TokenKind::QuestionDot) {
            return self.raw(field.syntax());
        }

        let receiver = field
            .receiver()
            .map(|receiver| self.format_expr(receiver, indent))
            .unwrap_or_default();
        let name = field
            .name_token()
            .map(|name| name.text(self.source).to_owned())
            .unwrap_or_default();
        format!("{receiver}.{name}")
    }

    fn format_array(&self, array: ArrayExpr<'_>, indent: usize) -> String {
        let items = array
            .items()
            .map(|items| {
                items
                    .exprs()
                    .map(|expr| self.format_expr(expr, indent + 1))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.render_fragment(&self.format_delimited_doc("[", "]", items), indent)
    }

    fn format_object(&self, object: ObjectExpr<'_>, indent: usize) -> String {
        let fields = object
            .fields()
            .map(|field| self.format_object_field(field, indent + 1))
            .collect::<Vec<_>>();
        self.render_fragment(
            &self.format_delimited_doc_with_limit("#{", "}", fields, Some(60)),
            indent,
        )
    }

    fn format_object_field(&self, field: ObjectField<'_>, indent: usize) -> String {
        let name = field
            .name_token()
            .map(|token| token.text(self.source).to_owned())
            .unwrap_or_default();
        let value = field
            .value()
            .map(|value| self.format_expr(value, indent))
            .unwrap_or_default();
        format!("{name}: {value}")
    }

    fn format_if_expr(&self, if_expr: IfExpr<'_>, indent: usize) -> String {
        let condition = if_expr
            .condition()
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        let then_branch = if_expr
            .then_branch()
            .map(|body| self.format_block(body, indent))
            .unwrap_or_else(|| "{}".to_owned());
        let mut out = format!("if {condition} {then_branch}");

        if let Some(else_body) = if_expr.else_branch().and_then(|branch| branch.body()) {
            match else_body {
                Expr::If(nested_if) => {
                    out.push_str(" else ");
                    out.push_str(&self.format_if_expr(nested_if, indent));
                }
                Expr::Block(block) => {
                    out.push_str(" else ");
                    out.push_str(&self.format_block(block, indent));
                }
                other => {
                    out.push_str(" else ");
                    out.push_str(&self.format_expr(other, indent));
                }
            }
        }

        out
    }

    fn format_switch_expr(&self, switch_expr: SwitchExpr<'_>, indent: usize) -> String {
        let scrutinee = switch_expr
            .scrutinee()
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        let arms = switch_expr.arms().collect::<Vec<_>>();
        if arms.is_empty() {
            return format!("switch {scrutinee} {{}}");
        }

        let mut out = format!("switch {scrutinee} {{\n");
        for (index, arm) in arms.iter().enumerate() {
            out.push_str(&self.indent(indent + 1));
            out.push_str(&self.format_switch_arm(*arm, indent + 1));
            if index + 1 < arms.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&self.indent(indent));
        out.push('}');
        out
    }

    fn format_switch_arm(&self, arm: SwitchArm<'_>, indent: usize) -> String {
        let patterns = arm
            .patterns()
            .map(|patterns| {
                let values = patterns
                    .exprs()
                    .map(|expr| self.format_expr(expr, indent))
                    .collect::<Vec<_>>();
                if patterns.wildcard_token().is_some() {
                    "_".to_owned()
                } else {
                    values.join(" | ")
                }
            })
            .unwrap_or_else(|| "_".to_owned());
        let value = arm
            .value()
            .map(|value| self.format_expr(value, indent))
            .unwrap_or_default();
        format!("{patterns} => {value}")
    }

    fn format_while_expr(&self, while_expr: WhileExpr<'_>, indent: usize) -> String {
        let condition = while_expr
            .condition()
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        let body = while_expr
            .body()
            .map(|body| self.format_block(body, indent))
            .unwrap_or_else(|| "{}".to_owned());
        format!("while {condition} {body}")
    }

    fn format_loop_expr(&self, loop_expr: LoopExpr<'_>, indent: usize) -> String {
        let body = loop_expr
            .body()
            .map(|body| self.format_block(body, indent))
            .unwrap_or_else(|| "{}".to_owned());
        format!("loop {body}")
    }

    fn format_for_expr(&self, for_expr: ForExpr<'_>, indent: usize) -> String {
        let bindings = self.format_for_bindings(for_expr.bindings());
        let iterable = for_expr
            .iterable()
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        let body = for_expr
            .body()
            .map(|body| self.format_block(body, indent))
            .unwrap_or_else(|| "{}".to_owned());
        format!("for {bindings} in {iterable} {body}")
    }

    fn format_for_bindings(&self, bindings: Option<ForBindings<'_>>) -> String {
        let names = bindings
            .map(|bindings| {
                bindings
                    .names()
                    .map(|name| name.text(self.source).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match names.len() {
            0 => "_".to_owned(),
            1 => names[0].clone(),
            _ => format!("({})", names.join(", ")),
        }
    }

    fn format_do_expr(&self, do_expr: DoExpr<'_>, indent: usize) -> String {
        let body = do_expr
            .body()
            .map(|body| self.format_block(body, indent))
            .unwrap_or_else(|| "{}".to_owned());
        let condition = do_expr.condition();
        let keyword = condition
            .and_then(|condition| condition.keyword_token())
            .map(|token| token.text(self.source))
            .unwrap_or("while");
        let expr = condition
            .and_then(|condition| condition.expr())
            .map(|expr| self.format_expr(expr, indent))
            .unwrap_or_default();
        format!("do {body} {keyword} {expr}")
    }

    fn format_closure_expr(&self, closure: ClosureExpr<'_>, indent: usize) -> String {
        let params = self.format_closure_params(closure.params());
        let body = closure
            .body()
            .map(|body| self.format_expr(body, indent))
            .unwrap_or_default();
        format!("{params} {body}")
    }

    fn format_closure_params(&self, params: Option<ClosureParamList<'_>>) -> String {
        let names = params
            .map(|params| {
                params
                    .params()
                    .map(|param| param.text(self.source).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        format!("|{}|", names.join(", "))
    }

    fn format_interpolated_string(&self, string: InterpolatedStringExpr<'_>) -> String {
        self.raw(string.syntax())
    }

    fn format_delimited_doc(&self, open: &str, close: &str, items: Vec<String>) -> Doc {
        self.format_delimited_doc_with_limit(open, close, items, None)
    }

    fn format_delimited_doc_with_limit(
        &self,
        open: &str,
        close: &str,
        items: Vec<String>,
        inline_limit: Option<usize>,
    ) -> Doc {
        if items.is_empty() {
            return Doc::text(format!("{open}{close}"));
        }

        let inline = format!("{open}{}{close}", items.join(", "));
        let max_inline_width = inline_limit.unwrap_or(self.options.max_line_length);
        let should_inline = items.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= max_inline_width;
        if should_inline {
            return Doc::text(inline);
        }

        let mut item_docs = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                item_docs.push(Doc::text(","));
                item_docs.push(Doc::soft_line());
            }
            item_docs.push(Doc::text(item));
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

        if inline_limit.is_some() {
            Doc::concat(parts)
        } else {
            Doc::group(Doc::concat(parts))
        }
    }

    pub(crate) fn format_params(&self, params: Option<ParamList<'_>>, indent: usize) -> String {
        let names = params
            .map(|params| {
                params
                    .params()
                    .map(|param| param.text(self.source).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.render_fragment(&self.format_delimited_doc("(", ")", names), indent)
    }

    pub(crate) fn raw(&self, node: &SyntaxNode) -> String {
        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;
        self.source[start..end].trim().to_owned()
    }
}
