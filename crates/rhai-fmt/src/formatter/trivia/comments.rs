use rhai_syntax::{
    AttachedComment, CommentKind, GapTrivia, OwnedTrivia, SyntaxElement, SyntaxNode, SyntaxNodeExt,
    SyntaxToken, TextRange, TokenKind, TriviaBoundary,
};

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

#[derive(Debug, Clone, Copy)]
pub(crate) struct GapSeparatorOptions<'a> {
    pub(crate) inline_text: &'a str,
    pub(crate) minimum_newlines: usize,
    pub(crate) has_previous: bool,
    pub(crate) has_next: bool,
    pub(crate) include_terminal_newline: bool,
}

impl Formatter<'_> {
    pub(crate) fn node_has_unowned_comments(&self, node: SyntaxNode) -> bool {
        self.trivia.node_has_unowned_comments(&node)
    }

    pub(crate) fn node_has_unowned_comments_outside_boundaries(
        &self,
        node: SyntaxNode,
        boundaries: &[TriviaBoundary],
    ) -> bool {
        self.trivia
            .node_has_unowned_comments_outside_boundaries(&node, boundaries)
    }

    pub(crate) fn boundary_trivia(
        &self,
        owner: SyntaxNode,
        boundary: TriviaBoundary,
    ) -> Option<GapTrivia> {
        self.trivia.trivia_for_boundary(&owner, &boundary)
    }

    pub(crate) fn owned_sequence_trivia(
        &self,
        start: usize,
        end: usize,
        elements: &[SyntaxElement],
    ) -> OwnedTrivia {
        self.trivia.owned_trivia_for_sequence(start, end, elements)
    }

    pub(crate) fn owned_range_sequence_trivia(
        &self,
        start: usize,
        end: usize,
        ranges: &[(usize, usize)],
    ) -> OwnedTrivia {
        self.trivia.owned_trivia_for_ranges(start, end, ranges)
    }

    pub(crate) fn boundary_gap(
        &self,
        boundary: TriviaBoundary,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        self.trivia
            .comment_gap_for_boundary(&boundary, has_previous, has_next)
    }

    pub(crate) fn tight_comment_gap_for_boundary(
        &self,
        owner: SyntaxNode,
        boundary: TriviaBoundary,
        allow_trailing_space: bool,
    ) -> Doc {
        self.boundary_trivia(owner, boundary)
            .map(|gap| self.tight_comment_gap_from_gap(&gap, allow_trailing_space))
            .unwrap_or_else(Doc::nil)
    }

    pub(crate) fn space_or_tight_gap_from_gap(&self, gap: &GapTrivia) -> Doc {
        if gap.has_comments() {
            self.tight_comment_gap_from_gap(gap, true)
        } else {
            Doc::text(" ")
        }
    }

    pub(crate) fn has_blank_line_between_nodes(
        &self,
        previous: SyntaxNode,
        next: SyntaxNode,
    ) -> bool {
        self.trivia
            .boundary_has_blank_line(&TriviaBoundary::NodeNode(previous, next))
    }

    pub(crate) fn is_whitespace_only_between_nodes(
        &self,
        previous: SyntaxNode,
        next: SyntaxNode,
    ) -> bool {
        self.trivia
            .boundary_is_whitespace_only(&TriviaBoundary::NodeNode(previous, next))
    }

    pub(crate) fn comment_gap(
        &self,
        start: usize,
        end: usize,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        self.trivia.comment_gap(start, end, has_previous, has_next)
    }

    pub(crate) fn token_range(&self, node: SyntaxNode, kind: TokenKind) -> Option<TextRange> {
        self.token(node, kind).map(|token| token.text_range())
    }

    pub(crate) fn token(&self, node: SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
        node.direct_significant_tokens()
            .find(|token| token.kind().token_kind() == Some(kind))
    }

    pub(crate) fn tokens(
        &self,
        node: SyntaxNode,
        kind: TokenKind,
    ) -> impl Iterator<Item = SyntaxToken> {
        node.direct_significant_tokens()
            .filter(move |token| token.kind().token_kind() == Some(kind))
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub(crate) fn gap_separator_doc(
        &self,
        gap: &GapTrivia,
        minimum_newlines: usize,
        has_previous: bool,
        include_terminal_newline: bool,
    ) -> Doc {
        let mut parts = Vec::new();

        if !gap.trailing_comments.is_empty() {
            parts.push(self.render_trailing_comments_doc(&gap.trailing_comments));
        }

        let vertical_comments = gap.vertical_comments();
        if !vertical_comments.is_empty() {
            let prefix_newlines = if has_previous {
                minimum_newlines.max(vertical_comments[0].blank_lines_before + 1)
            } else {
                vertical_comments[0].blank_lines_before
            };
            parts.push(hard_lines(prefix_newlines));
            parts.push(self.render_line_comments_doc(vertical_comments));

            let suffix_newlines = if include_terminal_newline {
                gap.trailing_blank_lines_before_next + 1
            } else {
                gap.trailing_blank_lines_before_next
            };
            if suffix_newlines > 0 {
                parts.push(hard_lines(suffix_newlines));
            }
        } else if has_previous {
            let suffix_newlines = if include_terminal_newline {
                minimum_newlines.max(gap.trailing_blank_lines_before_next + 1)
            } else {
                gap.trailing_blank_lines_before_next
            };
            if suffix_newlines > 0 {
                parts.push(hard_lines(suffix_newlines));
            }
        }

        Doc::concat(parts)
    }

    pub(crate) fn format_sequence_body_doc<F>(
        &self,
        docs: Vec<Doc>,
        owned: &OwnedTrivia,
        min_newlines: F,
    ) -> Vec<Doc>
    where
        F: Fn(usize) -> usize,
    {
        let mut parts = Vec::new();
        for (index, doc) in docs.into_iter().enumerate() {
            let gap = if index == 0 {
                owned.leading.clone()
            } else {
                owned.between.get(index - 1).cloned().unwrap_or_default()
            };
            parts.push(self.gap_separator_doc(&gap, min_newlines(index), index > 0, true));
            parts.push(doc);
        }
        parts
    }

    pub(crate) fn format_comma_sequence_body_doc(
        &self,
        docs: Vec<Doc>,
        owned: &OwnedTrivia,
    ) -> Vec<Doc> {
        let mut parts = Vec::new();
        let len = docs.len();
        for (index, doc) in docs.into_iter().enumerate() {
            let gap = if index == 0 {
                owned.leading.clone()
            } else {
                owned.between.get(index - 1).cloned().unwrap_or_default()
            };
            parts.push(self.gap_separator_doc(&gap, 1, index > 0, true));
            parts.push(doc);
            if index + 1 < len {
                parts.push(Doc::text(","));
            }
        }
        parts
    }

    pub(crate) fn append_sequence_trailing_doc(
        &self,
        parts: &mut Vec<Doc>,
        trailing_gap: &GapTrivia,
        has_items: bool,
        min_newlines: usize,
    ) {
        if has_items && trailing_gap.has_comments() {
            parts.push(self.gap_separator_doc(trailing_gap, min_newlines, true, false));
        } else if trailing_gap.has_vertical_comments() {
            parts.push(self.render_line_comments_doc(trailing_gap.vertical_comments()));
        }
    }

    pub(crate) fn format_empty_sequence_body_doc(&self, leading_gap: &GapTrivia) -> Doc {
        if leading_gap.has_vertical_comments() {
            let mut parts = vec![self.render_line_comments_doc(leading_gap.vertical_comments())];
            if leading_gap.trailing_blank_lines_before_next > 0 {
                parts.push(hard_lines(leading_gap.trailing_blank_lines_before_next));
            }
            Doc::concat(parts)
        } else {
            self.gap_separator_doc(leading_gap, 1, false, false)
        }
    }

    pub(crate) fn head_body_separator_doc(&self, start: usize, end: usize) -> Doc {
        let gap = self.comment_gap(start, end, true, true);
        self.head_body_separator_from_gap(&gap)
    }

    pub(crate) fn head_body_separator_for_boundary(
        &self,
        owner: SyntaxNode,
        boundary: TriviaBoundary,
    ) -> Doc {
        let gap = self
            .boundary_trivia(owner, boundary.clone())
            .unwrap_or_else(|| self.boundary_gap(boundary, true, true));
        self.head_body_separator_from_gap(&gap)
    }

    fn head_body_separator_from_gap(&self, gap: &GapTrivia) -> Doc {
        if !gap.has_comments() && gap.trailing_blank_lines_before_next == 0 {
            return Doc::text(" ");
        }

        let mut parts = Vec::new();
        if !gap.trailing_comments.is_empty() {
            parts.push(self.render_trailing_comments_doc(&gap.trailing_comments));
        }

        if gap.has_vertical_comments() {
            let vertical_comments = gap.vertical_comments();
            parts.push(hard_lines(vertical_comments[0].blank_lines_before + 1));
            parts.push(self.render_line_comments_doc(vertical_comments));
            parts.push(hard_lines(gap.trailing_blank_lines_before_next + 1));
        } else {
            parts.push(hard_lines(gap.trailing_blank_lines_before_next + 1));
        }

        Doc::concat(parts)
    }

    pub(crate) fn inline_or_boundary_separator_doc(
        &self,
        owner: SyntaxNode,
        boundary: TriviaBoundary,
        options: GapSeparatorOptions<'_>,
    ) -> Doc {
        let gap = self
            .boundary_trivia(owner, boundary.clone())
            .unwrap_or_else(|| self.boundary_gap(boundary, options.has_previous, options.has_next));
        self.inline_or_gap_separator_from_gap(&gap, options)
    }

    pub(crate) fn inline_or_gap_separator_doc(
        &self,
        start: usize,
        end: usize,
        options: GapSeparatorOptions<'_>,
    ) -> Doc {
        let gap = self.comment_gap(start, end, options.has_previous, options.has_next);
        self.inline_or_gap_separator_from_gap(&gap, options)
    }

    fn inline_or_gap_separator_from_gap(
        &self,
        gap: &GapTrivia,
        options: GapSeparatorOptions<'_>,
    ) -> Doc {
        if !gap.has_comments() && gap.trailing_blank_lines_before_next == 0 {
            return Doc::text(options.inline_text);
        }

        self.gap_separator_doc(
            gap,
            options.minimum_newlines,
            options.has_previous,
            options.include_terminal_newline,
        )
    }

    pub(crate) fn tight_comment_gap_from_gap(
        &self,
        gap: &GapTrivia,
        allow_trailing_space: bool,
    ) -> Doc {
        if !gap.has_comments() {
            return Doc::nil();
        }

        let mut parts = Vec::new();
        let vertical_comments = gap.vertical_comments();

        if !gap.trailing_comments.is_empty() {
            parts.push(self.render_trailing_comments_doc(&gap.trailing_comments));

            if !vertical_comments.is_empty() {
                parts.push(hard_lines(vertical_comments[0].blank_lines_before + 1));
            } else if gap.trailing_comments.iter().any(|comment| {
                matches!(
                    comment.kind,
                    CommentKind::Line | CommentKind::DocLine | CommentKind::Shebang
                )
            }) || gap.trailing_blank_lines_before_next > 0
            {
                parts.push(hard_lines(gap.trailing_blank_lines_before_next + 1));
            } else if allow_trailing_space {
                parts.push(Doc::text(" "));
            }
        } else if !vertical_comments.is_empty() {
            parts.push(hard_lines(vertical_comments[0].blank_lines_before + 1));
        }

        if !vertical_comments.is_empty() {
            parts.push(self.render_line_comments_doc(vertical_comments));
            parts.push(hard_lines(gap.trailing_blank_lines_before_next + 1));
        }

        Doc::concat(parts)
    }

    pub(crate) fn render_line_comments_doc(&self, comments: &[AttachedComment]) -> Doc {
        let mut parts = Vec::new();

        for (index, comment) in comments.iter().enumerate() {
            if index > 0 {
                parts.push(hard_lines(comment.blank_lines_before + 1));
            }
            parts.push(Doc::text(render_comment_text(comment, self.source)));
        }

        Doc::concat(parts)
    }

    fn render_trailing_comments_doc(&self, comments: &[AttachedComment]) -> Doc {
        let parts = comments
            .iter()
            .flat_map(|comment| {
                [
                    Doc::text(" "),
                    Doc::text(render_comment_text(comment, self.source)),
                ]
            })
            .collect::<Vec<_>>();
        Doc::concat(parts)
    }
}

fn render_comment_text(comment: &AttachedComment, source: &str) -> String {
    match comment.kind {
        CommentKind::Block | CommentKind::DocBlock => comment.text(source).to_owned(),
        CommentKind::Line | CommentKind::DocLine | CommentKind::Shebang => {
            comment.text(source).trim().to_owned()
        }
    }
}

fn hard_lines(count: usize) -> Doc {
    Doc::concat(vec![Doc::hard_line(); count])
}
