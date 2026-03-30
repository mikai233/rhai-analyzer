use crate::{FormatOptions, FormatResult, IndentStyle, RangeFormatResult};
use rhai_syntax::{
    AliasClause, ArgList, ArrayExpr, AstNode, BinaryExpr, BlockExpr, CallExpr, ClosureExpr,
    ClosureParamList, ConstStmt, DoExpr, Expr, ExprStmt, FieldExpr, FnItem, ForBindings, ForExpr,
    IfExpr, ImportStmt, IndexExpr, InterpolatedStringExpr, Item, LetStmt, LoopExpr, ObjectExpr,
    ObjectField, ParamList, ParenExpr, PathExpr, Root, Stmt, SwitchArm, SwitchExpr, SyntaxNode,
    TextRange, TextSize, TokenKind, WhileExpr, parse_text,
};

pub fn format_text(text: &str, options: &FormatOptions) -> FormatResult {
    let parse = parse_text(text);
    if !parse.errors().is_empty() {
        return FormatResult {
            text: text.to_owned(),
            changed: false,
        };
    }

    let Some(root) = Root::cast(parse.root()) else {
        return FormatResult {
            text: text.to_owned(),
            changed: false,
        };
    };

    let formatter = Formatter {
        source: text,
        options,
    };
    let formatted = formatter.format_root(root);

    FormatResult {
        changed: formatted != text,
        text: formatted,
    }
}

pub fn format_range(
    text: &str,
    requested_range: TextRange,
    options: &FormatOptions,
) -> Option<RangeFormatResult> {
    let formatted = format_text(text, options);
    if !formatted.changed {
        return None;
    }

    let (start, end, replacement) = minimal_changed_region(text, &formatted.text)?;
    let changed_range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    if !ranges_intersect(changed_range, requested_range) {
        return None;
    }

    Some(RangeFormatResult {
        range: changed_range,
        text: replacement.to_owned(),
        changed: true,
    })
}

struct Formatter<'a> {
    source: &'a str,
    options: &'a FormatOptions,
}

impl Formatter<'_> {
    fn format_root(&self, root: Root<'_>) -> String {
        let items = root.items().collect::<Vec<_>>();
        let mut out = String::new();

        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                let previous_is_fn = matches!(items[index - 1], Item::Fn(_));
                let current_is_fn = matches!(item, Item::Fn(_));
                out.push_str(if previous_is_fn || current_is_fn {
                    "\n\n"
                } else {
                    "\n"
                });
            }
            out.push_str(&self.format_item(*item, 0));
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    fn format_item(&self, item: Item<'_>, indent: usize) -> String {
        match item {
            Item::Fn(function) => self.format_function(function, indent),
            Item::Stmt(stmt) => self.format_stmt(stmt, indent),
        }
    }

    fn format_function(&self, function: FnItem<'_>, indent: usize) -> String {
        let mut out = self.indent(indent);
        if function.is_private() {
            out.push_str("private ");
        }
        out.push_str("fn ");

        if let Some(receiver) = function.this_type_token() {
            out.push_str(receiver.text(self.source));
            out.push('.');
        }

        if let Some(name) = function.name_token() {
            out.push_str(name.text(self.source));
        }

        out.push_str(&self.format_params(function.params(), indent));
        out.push(' ');
        out.push_str(
            &function
                .body()
                .map(|body| self.format_block(body, indent))
                .unwrap_or_else(|| "{}".to_owned()),
        );
        out
    }

    fn format_stmt(&self, stmt: Stmt<'_>, indent: usize) -> String {
        match stmt {
            Stmt::Let(let_stmt) => self.format_let_stmt(let_stmt, indent),
            Stmt::Const(const_stmt) => self.format_const_stmt(const_stmt, indent),
            Stmt::Import(import_stmt) => self.format_import_stmt(import_stmt, indent),
            Stmt::Export(export_stmt) => {
                let mut out = self.indent(indent);
                out.push_str("export ");
                if let Some(declaration) = export_stmt.declaration() {
                    out.push_str(self.format_stmt_inline(declaration, indent).trim_start());
                } else if let Some(target) = export_stmt.target() {
                    out.push_str(&self.format_expr(target, indent));
                    if let Some(alias) =
                        export_stmt.alias().and_then(|alias| self.alias_name(alias))
                    {
                        out.push_str(" as ");
                        out.push_str(alias);
                    }
                    out.push(';');
                } else {
                    out.push(';');
                }
                out
            }
            Stmt::Break(break_stmt) => {
                let mut out = self.indent(indent);
                out.push_str("break");
                if let Some(value) = break_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                out
            }
            Stmt::Continue(_) => format!("{}continue;", self.indent(indent)),
            Stmt::Return(return_stmt) => {
                let mut out = self.indent(indent);
                out.push_str("return");
                if let Some(value) = return_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                out
            }
            Stmt::Throw(throw_stmt) => {
                let mut out = self.indent(indent);
                out.push_str("throw");
                if let Some(value) = throw_stmt.value() {
                    out.push(' ');
                    out.push_str(&self.format_expr(value, indent));
                }
                out.push(';');
                out
            }
            Stmt::Try(try_stmt) => {
                let mut out = self.indent(indent);
                out.push_str("try ");
                out.push_str(
                    &try_stmt
                        .body()
                        .map(|body| self.format_block(body, indent))
                        .unwrap_or_else(|| "{}".to_owned()),
                );
                if let Some(catch_clause) = try_stmt.catch_clause() {
                    out.push_str(" catch");
                    if let Some(binding) = catch_clause.binding_token() {
                        out.push(' ');
                        out.push_str(binding.text(self.source));
                    }
                    out.push(' ');
                    out.push_str(
                        &catch_clause
                            .body()
                            .map(|body| self.format_block(body, indent))
                            .unwrap_or_else(|| "{}".to_owned()),
                    );
                }
                out
            }
            Stmt::Expr(expr_stmt) => self.format_expr_stmt(expr_stmt, indent),
        }
    }

    fn format_stmt_inline(&self, stmt: Stmt<'_>, indent: usize) -> String {
        self.format_stmt(stmt, indent)
    }

    fn format_let_stmt(&self, let_stmt: LetStmt<'_>, indent: usize) -> String {
        let mut out = self.indent(indent);
        out.push_str("let ");
        if let Some(name) = let_stmt.name_token() {
            out.push_str(name.text(self.source));
        }
        if let Some(initializer) = let_stmt.initializer() {
            out.push_str(" = ");
            out.push_str(&self.format_expr(initializer, indent));
        }
        out.push(';');
        out
    }

    fn format_const_stmt(&self, const_stmt: ConstStmt<'_>, indent: usize) -> String {
        let mut out = self.indent(indent);
        out.push_str("const ");
        if let Some(name) = const_stmt.name_token() {
            out.push_str(name.text(self.source));
        }
        if let Some(value) = const_stmt.value() {
            out.push_str(" = ");
            out.push_str(&self.format_expr(value, indent));
        }
        out.push(';');
        out
    }

    fn format_import_stmt(&self, import_stmt: ImportStmt<'_>, indent: usize) -> String {
        let mut out = self.indent(indent);
        out.push_str("import ");
        if let Some(module) = import_stmt.module() {
            out.push_str(&self.format_expr(module, indent));
        }
        if let Some(alias) = import_stmt.alias().and_then(|alias| self.alias_name(alias)) {
            out.push_str(" as ");
            out.push_str(alias);
        }
        out.push(';');
        out
    }

    fn format_expr_stmt(&self, expr_stmt: ExprStmt<'_>, indent: usize) -> String {
        let mut out = self.indent(indent);
        if let Some(expr) = expr_stmt.expr() {
            out.push_str(&self.format_expr(expr, indent));
        }
        if expr_stmt.has_semicolon() {
            out.push(';');
        }
        out
    }

    fn format_block(&self, block: BlockExpr<'_>, indent: usize) -> String {
        let items = block.items().collect::<Vec<_>>();
        if items.is_empty() {
            return "{}".to_owned();
        }

        let mut out = String::from("{\n");
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&self.format_item(*item, indent + 1));
        }
        out.push('\n');
        out.push_str(&self.indent(indent));
        out.push('}');
        out
    }

    fn format_expr(&self, expr: Expr<'_>, indent: usize) -> String {
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
        self.format_delimited("(", ")", &values, indent)
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
        self.format_delimited("[", "]", &items, indent)
    }

    fn format_object(&self, object: ObjectExpr<'_>, indent: usize) -> String {
        let fields = object
            .fields()
            .map(|field| self.format_object_field(field, indent + 1))
            .collect::<Vec<_>>();
        self.format_delimited_with_limit("#{", "}", &fields, indent, Some(60))
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

    fn format_delimited(&self, open: &str, close: &str, items: &[String], indent: usize) -> String {
        self.format_delimited_with_limit(open, close, items, indent, None)
    }

    fn format_delimited_with_limit(
        &self,
        open: &str,
        close: &str,
        items: &[String],
        indent: usize,
        inline_limit: Option<usize>,
    ) -> String {
        if items.is_empty() {
            return format!("{open}{close}");
        }

        let inline = format!("{open}{}{close}", items.join(", "));
        let max_inline_width = inline_limit.unwrap_or_else(|| {
            self.options
                .max_line_length
                .saturating_sub(indent * self.options.indent_width)
        });
        if items.iter().all(|item| !item.contains('\n'))
            && inline.chars().count() <= max_inline_width
        {
            return inline;
        }

        let mut out = String::new();
        out.push_str(open);
        out.push('\n');
        for (index, item) in items.iter().enumerate() {
            out.push_str(&self.indent(indent + 1));
            out.push_str(item);
            if index + 1 < items.len() || self.options.trailing_commas {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&self.indent(indent));
        out.push_str(close);
        out
    }

    fn format_params(&self, params: Option<ParamList<'_>>, indent: usize) -> String {
        let names = params
            .map(|params| {
                params
                    .params()
                    .map(|param| param.text(self.source).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.format_delimited("(", ")", &names, indent)
    }

    fn raw(&self, node: &SyntaxNode) -> String {
        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;
        self.source[start..end].trim().to_owned()
    }

    fn alias_name(&self, alias: AliasClause<'_>) -> Option<&str> {
        alias.alias_token().map(|token| token.text(self.source))
    }

    fn indent(&self, level: usize) -> String {
        match self.options.indent_style {
            IndentStyle::Spaces => " ".repeat(level * self.options.indent_width),
            IndentStyle::Tabs => "\t".repeat(level),
        }
    }
}

fn contains_token(node: &SyntaxNode, kind: TokenKind) -> bool {
    node.children().iter().any(|child| {
        child.as_token().is_some_and(|token| token.kind() == kind)
            || child
                .as_node()
                .is_some_and(|child_node| contains_token(child_node, kind))
    })
}

fn minimal_changed_region<'a>(original: &'a str, formatted: &'a str) -> Option<(usize, usize, &'a str)> {
    if original == formatted {
        return None;
    }

    let prefix = common_prefix_len(original, formatted);
    let original_suffix = &original[prefix..];
    let formatted_suffix = &formatted[prefix..];
    let suffix = common_suffix_len(original_suffix, formatted_suffix);

    let original_end = original.len().saturating_sub(suffix);
    let formatted_end = formatted.len().saturating_sub(suffix);
    Some((prefix, original_end, &formatted[prefix..formatted_end]))
}

fn common_prefix_len(left: &str, right: &str) -> usize {
    let mut left_iter = left.char_indices();
    let mut right_iter = right.char_indices();
    let mut len = 0;

    loop {
        match (left_iter.next(), right_iter.next()) {
            (Some((left_index, left_char)), Some((right_index, right_char)))
                if left_char == right_char && left_index == right_index =>
            {
                len = left_index + left_char.len_utf8();
            }
            _ => break,
        }
    }

    len
}

fn common_suffix_len(left: &str, right: &str) -> usize {
    let mut left_iter = left.chars().rev();
    let mut right_iter = right.chars().rev();
    let mut len = 0;

    loop {
        match (left_iter.next(), right_iter.next()) {
            (Some(left_char), Some(right_char)) if left_char == right_char => {
                len += left_char.len_utf8();
            }
            _ => break,
        }
    }

    len.min(left.len()).min(right.len())
}

fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    let left_start = u32::from(left.start());
    let left_end = u32::from(left.end());
    let right_start = u32::from(right.start());
    let right_end = u32::from(right.end());

    left_start < right_end && right_start < left_end
}
