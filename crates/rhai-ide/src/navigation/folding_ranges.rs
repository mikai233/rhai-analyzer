use rhai_db::DatabaseSnapshot;
use rhai_syntax::{
    ArrayExpr, AstNode, BlockExpr, CatchClause, Expr, ForExpr, IfExpr, Item, ObjectExpr, Root,
    RowanSyntaxNode, Stmt, SwitchExpr, TextRange, TokenKind, TryStmt, WhileExpr,
};
use rhai_vfs::FileId;

use crate::{FoldingRange, FoldingRangeKind};

pub(crate) fn folding_ranges(snapshot: &DatabaseSnapshot, file_id: FileId) -> Vec<FoldingRange> {
    let Some(parse) = snapshot.parse(file_id) else {
        return Vec::new();
    };

    let root_syntax = parse.root();
    let mut ranges = comment_folding_ranges(&root_syntax, parse.text());
    let Some(root) = Root::cast(root_syntax) else {
        return ranges;
    };

    if let Some(items) = root.item_list() {
        for item in items.items() {
            collect_item_folding_ranges(item, &mut ranges);
        }
    }

    ranges.sort_by_key(|range| (u32::from(range.range.start()), u32::from(range.range.end())));
    ranges.dedup_by_key(|range| (range.range.start(), range.range.end(), range.kind));
    ranges
}

fn comment_folding_ranges(root: &RowanSyntaxNode, _source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut run_start = None;
    let mut run_end = None;
    let mut pending_gap_allows_continuation = false;

    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        let Some(kind) = token.kind().token_kind() else {
            continue;
        };
        match kind {
            TokenKind::LineComment | TokenKind::DocLineComment => {
                if run_start.is_some() && !pending_gap_allows_continuation {
                    maybe_push_comment_run(&mut ranges, run_start, run_end);
                    run_start = None;
                }
                run_start.get_or_insert(token.text_range().start());
                run_end = Some(token.text_range().end());
                pending_gap_allows_continuation = false;
            }
            TokenKind::Whitespace if run_start.is_some() => {
                pending_gap_allows_continuation = !contains_blank_line(token.text());
            }
            _ => {
                maybe_push_comment_run(&mut ranges, run_start, run_end);
                run_start = None;
                run_end = None;
                pending_gap_allows_continuation = false;
            }
        }
    }

    maybe_push_comment_run(&mut ranges, run_start, run_end);
    ranges
}

fn contains_blank_line(text: &str) -> bool {
    let newline_count = text.chars().filter(|ch| *ch == '\n').count();
    newline_count > 1
}

fn maybe_push_comment_run(
    ranges: &mut Vec<FoldingRange>,
    start: Option<rhai_syntax::TextSize>,
    end: Option<rhai_syntax::TextSize>,
) {
    if let (Some(start), Some(end)) = (start, end) {
        let range = TextRange::new(start, end);
        if is_multiline(range) {
            ranges.push(FoldingRange {
                range,
                kind: FoldingRangeKind::Comment,
            });
        }
    }
}

fn collect_item_folding_ranges(item: Item, ranges: &mut Vec<FoldingRange>) {
    match item {
        Item::Fn(function) => {
            push_region_range(ranges, function.syntax().text_range());
            if let Some(body) = function.body() {
                collect_block_folding_ranges(body, ranges);
            }
        }
        Item::Stmt(stmt) => collect_stmt_folding_ranges(stmt, ranges),
    }
}

fn collect_stmt_folding_ranges(stmt: Stmt, ranges: &mut Vec<FoldingRange>) {
    match stmt {
        Stmt::Try(try_stmt) => collect_try_folding_ranges(try_stmt, ranges),
        Stmt::Expr(expr_stmt) => {
            if let Some(expr) = expr_stmt.expr() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Let(let_stmt) => {
            if let Some(expr) = let_stmt.initializer() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Const(const_stmt) => {
            if let Some(expr) = const_stmt.value() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Import(import_stmt) => {
            if let Some(expr) = import_stmt.module() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Export(export_stmt) => {
            if let Some(expr) = export_stmt.target() {
                collect_expr_folding_ranges(expr, ranges);
            }
            if let Some(declaration) = export_stmt.declaration() {
                collect_stmt_folding_ranges(declaration, ranges);
            }
        }
        Stmt::Break(break_stmt) => {
            if let Some(expr) = break_stmt.value() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Return(return_stmt) => {
            if let Some(expr) = return_stmt.value() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Throw(throw_stmt) => {
            if let Some(expr) = throw_stmt.value() {
                collect_expr_folding_ranges(expr, ranges);
            }
        }
        Stmt::Continue(_) => {}
    }
}

fn collect_try_folding_ranges(try_stmt: TryStmt, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, try_stmt.syntax().text_range());

    if let Some(body) = try_stmt.body() {
        collect_block_folding_ranges(body, ranges);
    }

    if let Some(catch_clause) = try_stmt.catch_clause() {
        collect_catch_folding_ranges(catch_clause, ranges);
    }
}

fn collect_catch_folding_ranges(catch_clause: CatchClause, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, catch_clause.syntax().text_range());

    if let Some(body) = catch_clause.body() {
        collect_block_folding_ranges(body, ranges);
    }
}

fn collect_expr_folding_ranges(expr: Expr, ranges: &mut Vec<FoldingRange>) {
    match expr {
        Expr::Array(array) => collect_array_folding_ranges(array, ranges),
        Expr::Object(object) => collect_object_folding_ranges(object, ranges),
        Expr::If(if_expr) => collect_if_folding_ranges(if_expr, ranges),
        Expr::Switch(switch_expr) => collect_switch_folding_ranges(switch_expr, ranges),
        Expr::While(while_expr) => collect_while_folding_ranges(while_expr, ranges),
        Expr::Loop(loop_expr) => {
            push_region_range(ranges, loop_expr.syntax().text_range());
            if let Some(body) = loop_expr.body() {
                collect_block_folding_ranges(body, ranges);
            }
        }
        Expr::For(for_expr) => collect_for_folding_ranges(for_expr, ranges),
        Expr::Do(do_expr) => {
            push_region_range(ranges, do_expr.syntax().text_range());
            if let Some(body) = do_expr.body() {
                collect_block_folding_ranges(body, ranges);
            }
            if let Some(condition) = do_expr.condition().and_then(|condition| condition.expr()) {
                collect_expr_folding_ranges(condition, ranges);
            }
        }
        Expr::Closure(closure) => {
            push_region_range(ranges, closure.syntax().text_range());
            if let Some(body) = closure.body() {
                collect_expr_folding_ranges(body, ranges);
            }
        }
        Expr::InterpolatedString(string) => {
            push_region_range(ranges, string.syntax().text_range());
            if let Some(parts) = string.part_list() {
                for part in parts.parts() {
                    if let rhai_syntax::StringPart::Interpolation(interpolation) = part
                        && let Some(body) = interpolation.body()
                    {
                        push_region_range(ranges, body.syntax().text_range());
                        if let Some(items) = body.item_list() {
                            for item in items.items() {
                                collect_item_folding_ranges(item, ranges);
                            }
                        }
                    }
                }
            }
        }
        Expr::Unary(unary) => {
            if let Some(inner) = unary.expr() {
                collect_expr_folding_ranges(inner, ranges);
            }
        }
        Expr::Binary(binary) => {
            if let Some(lhs) = binary.lhs() {
                collect_expr_folding_ranges(lhs, ranges);
            }
            if let Some(rhs) = binary.rhs() {
                collect_expr_folding_ranges(rhs, ranges);
            }
        }
        Expr::Assign(assign) => {
            if let Some(lhs) = assign.lhs() {
                collect_expr_folding_ranges(lhs, ranges);
            }
            if let Some(rhs) = assign.rhs() {
                collect_expr_folding_ranges(rhs, ranges);
            }
        }
        Expr::Paren(paren) => {
            if let Some(inner) = paren.expr() {
                collect_expr_folding_ranges(inner, ranges);
            }
        }
        Expr::Call(call) => {
            push_region_range(ranges, call.syntax().text_range());
            if let Some(callee) = call.callee() {
                collect_expr_folding_ranges(callee, ranges);
            }
            if let Some(args) = call.args() {
                for arg in args.args() {
                    collect_expr_folding_ranges(arg, ranges);
                }
            }
        }
        Expr::Index(index) => {
            push_region_range(ranges, index.syntax().text_range());
            if let Some(receiver) = index.receiver() {
                collect_expr_folding_ranges(receiver, ranges);
            }
            if let Some(inner) = index.index() {
                collect_expr_folding_ranges(inner, ranges);
            }
        }
        Expr::Field(field) => {
            if let Some(receiver) = field.receiver() {
                collect_expr_folding_ranges(receiver, ranges);
            }
        }
        Expr::Block(block) => collect_block_folding_ranges(block, ranges),
        Expr::Path(path) => {
            if let Some(base) = path.base() {
                collect_expr_folding_ranges(base, ranges);
            }
        }
        Expr::Name(_) | Expr::Literal(_) | Expr::Error(_) => {}
    }
}

fn collect_array_folding_ranges(array: ArrayExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, array.syntax().text_range());

    if let Some(items) = array.items() {
        for expr in items.exprs() {
            collect_expr_folding_ranges(expr, ranges);
        }
    }
}

fn collect_object_folding_ranges(object: ObjectExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, object.syntax().text_range());

    if let Some(fields) = object.field_list() {
        for field in fields.fields() {
            if let Some(value) = field.value() {
                collect_expr_folding_ranges(value, ranges);
            }
        }
    }
}

fn collect_if_folding_ranges(if_expr: IfExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, if_expr.syntax().text_range());

    if let Some(condition) = if_expr.condition() {
        collect_expr_folding_ranges(condition, ranges);
    }
    if let Some(then_branch) = if_expr.then_branch() {
        collect_block_folding_ranges(then_branch, ranges);
    }
    if let Some(else_body) = if_expr.else_branch().and_then(|branch| branch.body()) {
        collect_expr_folding_ranges(else_body, ranges);
    }
}

fn collect_switch_folding_ranges(switch_expr: SwitchExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, switch_expr.syntax().text_range());

    if let Some(scrutinee) = switch_expr.scrutinee() {
        collect_expr_folding_ranges(scrutinee, ranges);
    }

    if let Some(arm_list) = switch_expr.arm_list() {
        for arm in arm_list.arms() {
            push_region_range(ranges, arm.syntax().text_range());
            if let Some(patterns) = arm.patterns() {
                for pattern in patterns.exprs() {
                    collect_expr_folding_ranges(pattern, ranges);
                }
            }
            if let Some(value) = arm.value() {
                collect_expr_folding_ranges(value, ranges);
            }
        }
    }
}

fn collect_while_folding_ranges(while_expr: WhileExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, while_expr.syntax().text_range());

    if let Some(condition) = while_expr.condition() {
        collect_expr_folding_ranges(condition, ranges);
    }
    if let Some(body) = while_expr.body() {
        collect_block_folding_ranges(body, ranges);
    }
}

fn collect_for_folding_ranges(for_expr: ForExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, for_expr.syntax().text_range());

    if let Some(iterable) = for_expr.iterable() {
        collect_expr_folding_ranges(iterable, ranges);
    }
    if let Some(body) = for_expr.body() {
        collect_block_folding_ranges(body, ranges);
    }
}

fn collect_block_folding_ranges(block: BlockExpr, ranges: &mut Vec<FoldingRange>) {
    push_region_range(ranges, block.syntax().text_range());

    if let Some(items) = block.item_list() {
        for item in items.items() {
            collect_item_folding_ranges(item, ranges);
        }
    }
}

fn push_region_range(ranges: &mut Vec<FoldingRange>, range: TextRange) {
    if is_multiline(range) {
        ranges.push(FoldingRange {
            range,
            kind: FoldingRangeKind::Region,
        });
    }
}

fn is_multiline(range: TextRange) -> bool {
    u32::from(range.start()) < u32::from(range.end())
}
