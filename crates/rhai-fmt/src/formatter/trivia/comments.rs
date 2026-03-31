use rhai_syntax::{
    AttachedComment, CommentKind, GapTrivia, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange,
    TokenKind, TriviaBoundary,
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

    pub(crate) fn node_has_unowned_comments_outside(
        &self,
        node: SyntaxNode,
        allowed_ranges: &[(usize, usize)],
    ) -> bool {
        self.trivia
            .node_has_unowned_comments_outside(&node, allowed_ranges)
    }

    pub(crate) fn node_has_unowned_comments_outside_boundaries(
        &self,
        node: SyntaxNode,
        boundaries: &[TriviaBoundary],
    ) -> bool {
        self.trivia
            .node_has_unowned_comments_outside_boundaries(&node, boundaries)
    }

    pub(crate) fn range_has_comments(&self, start: usize, end: usize) -> bool {
        self.trivia.range_has_comments(start, end)
    }

    pub(crate) fn has_blank_line_between_nodes(
        &self,
        previous: SyntaxNode,
        next: SyntaxNode,
    ) -> bool {
        self.trivia
            .has_blank_line_after_node_before_node(&previous, &next)
    }

    pub(crate) fn is_whitespace_only_between_nodes(
        &self,
        previous: SyntaxNode,
        next: SyntaxNode,
    ) -> bool {
        self.trivia
            .is_whitespace_only_after_node_before_node(&previous, &next)
    }

    pub(crate) fn range_after_node_before_node(
        &self,
        previous: SyntaxNode,
        next: SyntaxNode,
    ) -> (usize, usize) {
        self.trivia.range_after_node_before_node(&previous, &next)
    }

    pub(crate) fn range_after_node_before_token(
        &self,
        previous: SyntaxNode,
        next: &SyntaxToken,
    ) -> (usize, usize) {
        self.trivia
            .range_after_node_before_token(&previous, next.clone())
    }

    pub(crate) fn range_after_token_before_node(
        &self,
        previous: &SyntaxToken,
        next: SyntaxNode,
    ) -> (usize, usize) {
        self.trivia
            .range_after_token_before_node(previous.clone(), &next)
    }

    pub(crate) fn range_after_token_before_token(
        &self,
        previous: &SyntaxToken,
        next: &SyntaxToken,
    ) -> (usize, usize) {
        self.trivia
            .range_after_token_before_token(previous.clone(), next.clone())
    }

    pub(crate) fn has_comments_after_node_before_token(
        &self,
        previous: SyntaxNode,
        next: &SyntaxToken,
    ) -> bool {
        self.trivia
            .has_comments_after_node_before_token(&previous, next.clone())
    }

    pub(crate) fn has_comments_after_token_before_node(
        &self,
        previous: &SyntaxToken,
        next: SyntaxNode,
    ) -> bool {
        self.trivia
            .has_comments_after_token_before_node(previous.clone(), &next)
    }

    pub(crate) fn has_comments_after_token_before_token(
        &self,
        previous: &SyntaxToken,
        next: &SyntaxToken,
    ) -> bool {
        self.trivia
            .has_comments_after_token_before_token(previous.clone(), next.clone())
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

    pub(crate) fn head_body_separator_doc(&self, start: usize, end: usize) -> Doc {
        let gap = self.comment_gap(start, end, true, true);
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

    pub(crate) fn inline_or_gap_separator_doc(
        &self,
        start: usize,
        end: usize,
        options: GapSeparatorOptions<'_>,
    ) -> Doc {
        let gap = self.comment_gap(start, end, options.has_previous, options.has_next);
        if !gap.has_comments() && gap.trailing_blank_lines_before_next == 0 {
            return Doc::text(options.inline_text);
        }

        self.gap_separator_doc(
            &gap,
            options.minimum_newlines,
            options.has_previous,
            options.include_terminal_newline,
        )
    }

    pub(crate) fn tight_comment_gap_doc(&self, start: usize, end: usize) -> Doc {
        let gap = self.comment_gap(start, end, true, true);
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
            } else {
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

    pub(crate) fn tight_comment_gap_after_node_before_token(
        &self,
        previous: SyntaxNode,
        next: &SyntaxToken,
    ) -> Doc {
        let (start, end) = self.range_after_node_before_token(previous, next);
        self.tight_comment_gap_doc(start, end)
    }

    pub(crate) fn tight_comment_gap_after_token_before_node(
        &self,
        previous: &SyntaxToken,
        next: SyntaxNode,
    ) -> Doc {
        let (start, end) = self.range_after_token_before_node(previous, next);
        self.tight_comment_gap_doc(start, end)
    }

    pub(crate) fn tight_comment_gap_after_token_before_token(
        &self,
        previous: &SyntaxToken,
        next: &SyntaxToken,
    ) -> Doc {
        let (start, end) = self.range_after_token_before_token(previous, next);
        self.tight_comment_gap_doc(start, end)
    }

    pub(crate) fn tight_comment_gap_doc_without_trailing_space(
        &self,
        start: usize,
        end: usize,
    ) -> Doc {
        let gap = self.comment_gap(start, end, true, true);
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
