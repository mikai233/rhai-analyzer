pub(crate) mod layout;
pub(crate) mod support;
pub(crate) mod syntax;
pub(crate) mod trivia;

use crate::{FormatOptions, FormatResult, RangeFormatResult};
use rhai_syntax::{
    AliasClause, ArgList, ArrayItemList, AstNode, BlockExpr, CatchClause, ClosureParamList,
    DoCondition, ElseBranch, Expr, ForBindings, Item, ParamList, Root, SwitchPatternList,
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, parse_text,
};

use crate::formatter::layout::doc::Doc;
use crate::formatter::layout::render::{render_doc, render_doc_with_indent};
use crate::formatter::support::coverage::{FormatSupportLevel, expr_support};
use crate::formatter::support::utils::{minimal_changed_region, ranges_intersect};

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
        tokens: parse.tokens(),
        line_starts: collect_line_starts(text),
        options,
    };
    let formatted =
        normalize_document_output(render_doc(&formatter.format_root(root), options), options);

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
    let parse = parse_text(text);
    if !parse.errors().is_empty() {
        return None;
    }

    let root = Root::cast(parse.root())?;
    let structural_range = intersect_ranges(root.syntax().range(), requested_range)?;
    if !ranges_intersect(structural_range, requested_range) {
        return None;
    }

    let formatter = Formatter {
        source: text,
        tokens: parse.tokens(),
        line_starts: collect_line_starts(text),
        options,
    };
    let owner = select_range_owner(root, structural_range)?;
    let replacement = match owner.kind {
        RangeOwnerKind::Root(root) => {
            normalize_document_output(render_doc(&formatter.format_root(root), options), options)
        }
        RangeOwnerKind::Item(item) => formatter.render_fragment(
            &formatter.format_item(item, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::Block(block) => formatter.render_fragment(
            &formatter.format_block_doc(block, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::Expr(expr) => formatter.render_fragment(
            &formatter.format_expr_doc(expr, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::ParamList(params) => formatter.render_fragment(
            &formatter.format_params_doc(Some(params), owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::ClosureParamList(params) => formatter.render_fragment(
            &formatter.format_closure_params_doc(Some(params)),
            owner.base_indent,
        ),
        RangeOwnerKind::ArgList(args) => formatter.render_fragment(
            &formatter.format_arg_list_body_doc(args, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::ArrayItemList(items) => formatter.render_fragment(
            &formatter.format_array_item_list_body_doc(items, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::SwitchPatternList(patterns) => formatter.render_fragment(
            &formatter.format_switch_patterns_doc(patterns, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::ForBindings(bindings) => formatter.render_fragment(
            &formatter.format_for_bindings_doc(Some(bindings)),
            owner.base_indent,
        ),
        RangeOwnerKind::DoCondition(condition) => formatter.render_fragment(
            &formatter.format_do_condition_doc(condition, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::CatchClause(catch_clause) => formatter.render_fragment(
            &formatter.format_catch_clause_doc(catch_clause, owner.base_indent),
            owner.base_indent,
        ),
        RangeOwnerKind::AliasClause(alias) => {
            formatter.render_fragment(&formatter.format_alias_clause_doc(alias), owner.base_indent)
        }
        RangeOwnerKind::ElseBranch(else_branch) => formatter.render_fragment(
            &formatter.format_else_branch_doc(else_branch, owner.base_indent),
            owner.base_indent,
        ),
    };
    let start = u32::from(owner.range.start()) as usize;
    let end = u32::from(owner.range.end()) as usize;
    let original = &text[start..end];
    let (local_start, local_end, replacement) = minimal_changed_region(original, &replacement)?;

    let local_range = TextRange::new(
        TextSize::from(local_start as u32),
        TextSize::from(local_end as u32),
    );
    let absolute_range = TextRange::new(
        owner.range.start() + local_range.start(),
        owner.range.start() + local_range.end(),
    );
    if !ranges_intersect(absolute_range, requested_range) {
        return None;
    }

    Some(RangeFormatResult {
        range: absolute_range,
        text: replacement.to_owned(),
        changed: true,
    })
}

pub(crate) struct Formatter<'a> {
    source: &'a str,
    tokens: &'a [SyntaxToken],
    line_starts: Vec<usize>,
    options: &'a FormatOptions,
}

impl Formatter<'_> {
    pub(crate) fn render_fragment(&self, doc: &Doc, base_indent: usize) -> String {
        render_doc_with_indent(doc, self.options, base_indent)
    }
}

fn collect_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

#[derive(Clone, Copy)]
struct RangeOwner<'a> {
    range: TextRange,
    base_indent: usize,
    kind: RangeOwnerKind<'a>,
}

#[derive(Clone, Copy)]
enum RangeOwnerKind<'a> {
    Root(Root<'a>),
    Item(Item<'a>),
    Block(BlockExpr<'a>),
    Expr(Expr<'a>),
    ParamList(ParamList<'a>),
    ClosureParamList(ClosureParamList<'a>),
    ArgList(ArgList<'a>),
    ArrayItemList(ArrayItemList<'a>),
    SwitchPatternList(SwitchPatternList<'a>),
    ForBindings(ForBindings<'a>),
    DoCondition(DoCondition<'a>),
    CatchClause(CatchClause<'a>),
    AliasClause(AliasClause<'a>),
    ElseBranch(ElseBranch<'a>),
}

fn select_range_owner<'a>(root: Root<'a>, requested_range: TextRange) -> Option<RangeOwner<'a>> {
    let mut owner = RangeOwner {
        range: root.syntax().range(),
        base_indent: 0,
        kind: RangeOwnerKind::Root(root),
    };

    if let Some(nested_owner) = find_nested_range_owner(root.syntax(), requested_range, 0) {
        owner = nested_owner;
    }

    Some(owner)
}

fn find_nested_range_owner<'a>(
    node: &'a SyntaxNode,
    requested_range: TextRange,
    block_depth: usize,
) -> Option<RangeOwner<'a>> {
    if !range_contains(node.range(), requested_range) {
        return None;
    }

    let mut best = range_owner_for_node(node, block_depth);
    let child_block_depth = if node.kind() == SyntaxKind::Block {
        block_depth + 1
    } else {
        block_depth
    };

    for child in node.children().iter().filter_map(|child| child.as_node()) {
        if let Some(child_owner) =
            find_nested_range_owner(child, requested_range, child_block_depth)
        {
            best = Some(child_owner);
        }
    }

    best
}

fn range_owner_for_node<'a>(node: &'a SyntaxNode, block_depth: usize) -> Option<RangeOwner<'a>> {
    if let Some(item) = Item::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::Item(item),
        });
    }

    if let Some(block) = BlockExpr::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::Block(block),
        });
    }

    if let Some(params) = ParamList::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ParamList(params),
        });
    }

    if let Some(params) = ClosureParamList::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ClosureParamList(params),
        });
    }

    if let Some(args) = ArgList::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ArgList(args),
        });
    }

    if let Some(items) = ArrayItemList::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ArrayItemList(items),
        });
    }

    if let Some(patterns) = SwitchPatternList::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::SwitchPatternList(patterns),
        });
    }

    if let Some(bindings) = ForBindings::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ForBindings(bindings),
        });
    }

    if let Some(condition) = DoCondition::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::DoCondition(condition),
        });
    }

    if let Some(catch_clause) = CatchClause::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::CatchClause(catch_clause),
        });
    }

    if let Some(alias) = AliasClause::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::AliasClause(alias),
        });
    }

    if let Some(else_branch) = ElseBranch::cast(node) {
        return Some(RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::ElseBranch(else_branch),
        });
    }

    Expr::cast(node)
        .filter(|expr| range_owner_supports_expr(*expr))
        .map(|expr| RangeOwner {
            range: node.range(),
            base_indent: block_depth,
            kind: RangeOwnerKind::Expr(expr),
        })
}

fn range_owner_supports_expr(expr: Expr<'_>) -> bool {
    !matches!(
        expr,
        Expr::Name(_) | Expr::Literal(_) | Expr::Block(_) | Expr::Error(_)
    ) && !matches!(expr_support(expr).level, FormatSupportLevel::RawFallback)
}

fn intersect_ranges(left: TextRange, right: TextRange) -> Option<TextRange> {
    let start = u32::from(left.start()).max(u32::from(right.start()));
    let end = u32::from(left.end()).min(u32::from(right.end()));
    if start >= end {
        return None;
    }

    Some(TextRange::new(TextSize::from(start), TextSize::from(end)))
}

fn range_contains(container: TextRange, candidate: TextRange) -> bool {
    u32::from(container.start()) <= u32::from(candidate.start())
        && u32::from(candidate.end()) <= u32::from(container.end())
}

fn normalize_document_output(mut text: String, options: &FormatOptions) -> String {
    if !options.final_newline && text.ends_with('\n') {
        text.pop();
    }

    text
}
