use rhai_syntax::{AliasClause, SyntaxNode, TextRange, TokenKind};

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentKind {
    Line,
    DocLine,
    Block,
    DocBlock,
    Shebang,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttachedComment {
    pub(crate) kind: CommentKind,
    pub(crate) text: String,
    pub(crate) blank_lines_before: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GapTrivia {
    pub(crate) trailing_comments: Vec<AttachedComment>,
    pub(crate) line_comments: Vec<AttachedComment>,
    pub(crate) trailing_blank_lines_before_next: usize,
}

impl Formatter<'_> {
    pub(crate) fn alias_name(&self, alias: AliasClause<'_>) -> Option<&str> {
        alias.alias_token().map(|token| token.text(self.source))
    }

    pub(crate) fn node_has_comments(&self, node: &SyntaxNode) -> bool {
        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;

        self.range_has_comments(start, end)
    }

    pub(crate) fn node_has_unowned_comments(&self, node: &SyntaxNode) -> bool {
        let child_ranges = node
            .children()
            .iter()
            .filter_map(|child| child.as_node())
            .map(|child| child.range())
            .collect::<Vec<_>>();

        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;

        self.tokens
            .iter()
            .copied()
            .filter(|token| self.token_in_range(*token, start, end))
            .any(|token| {
                self.comment_kind(token.kind()).is_some()
                    && !child_ranges
                        .iter()
                        .any(|range| range_contains(*range, token.range()))
            })
    }

    pub(crate) fn range_has_comments(&self, start: usize, end: usize) -> bool {
        self.tokens
            .iter()
            .copied()
            .filter(|token| self.token_in_range(*token, start, end))
            .any(|token| self.comment_kind(token.kind()).is_some())
    }

    pub(crate) fn comment_gap(&self, start: usize, end: usize, has_previous: bool) -> GapTrivia {
        if start >= end || end > self.source.len() {
            return GapTrivia::default();
        }

        let start_line = self.line_index(start);
        let mut cursor_line = start_line;
        let mut trailing_comments = Vec::new();
        let mut line_comments = Vec::new();

        for token in self
            .tokens
            .iter()
            .copied()
            .filter(|token| self.token_in_range(*token, start, end))
        {
            let Some(kind) = self.comment_kind(token.kind()) else {
                continue;
            };

            let comment_start_line = self.line_index(u32::from(token.range().start()) as usize);
            let comment_end_line = self.line_index_for_end(u32::from(token.range().end()) as usize);
            let text = token.text(self.source).to_owned();

            if has_previous && comment_start_line == start_line && line_comments.is_empty() {
                trailing_comments.push(AttachedComment {
                    kind,
                    text,
                    blank_lines_before: 0,
                });
                cursor_line = comment_end_line;
                continue;
            }

            let blank_lines_before = comment_start_line.saturating_sub(cursor_line + 1);
            line_comments.push(AttachedComment {
                kind,
                text,
                blank_lines_before,
            });
            cursor_line = comment_end_line;
        }

        let next_line = self.line_index(end);
        let trailing_blank_lines_before_next = next_line.saturating_sub(cursor_line + 1);

        GapTrivia {
            trailing_comments,
            line_comments,
            trailing_blank_lines_before_next,
        }
    }

    pub(crate) fn token_range(&self, node: &SyntaxNode, kind: TokenKind) -> Option<TextRange> {
        node.children()
            .iter()
            .filter_map(|child| child.as_token())
            .find(|token| token.kind() == kind)
            .map(|token| token.range())
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

        if !gap.line_comments.is_empty() {
            let prefix_newlines = if has_previous {
                minimum_newlines.max(gap.line_comments[0].blank_lines_before + 1)
            } else {
                gap.line_comments[0].blank_lines_before
            };
            parts.push(hard_lines(prefix_newlines));
            parts.push(self.render_line_comments_doc(&gap.line_comments));

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

    pub(crate) fn render_line_comments_doc(&self, comments: &[AttachedComment]) -> Doc {
        let mut parts = Vec::new();

        for (index, comment) in comments.iter().enumerate() {
            if index > 0 {
                parts.push(hard_lines(comment.blank_lines_before + 1));
            }
            parts.push(Doc::text(render_comment_text(comment)));
        }

        Doc::concat(parts)
    }

    fn render_trailing_comments_doc(&self, comments: &[AttachedComment]) -> Doc {
        let parts = comments
            .iter()
            .flat_map(|comment| [Doc::text(" "), Doc::text(render_comment_text(comment))])
            .collect::<Vec<_>>();
        Doc::concat(parts)
    }

    fn comment_kind(&self, kind: TokenKind) -> Option<CommentKind> {
        match kind {
            TokenKind::LineComment => Some(CommentKind::Line),
            TokenKind::DocLineComment => Some(CommentKind::DocLine),
            TokenKind::BlockComment => Some(CommentKind::Block),
            TokenKind::DocBlockComment => Some(CommentKind::DocBlock),
            TokenKind::Shebang => Some(CommentKind::Shebang),
            _ => None,
        }
    }

    fn token_in_range(&self, token: rhai_syntax::SyntaxToken, start: usize, end: usize) -> bool {
        let token_start = u32::from(token.range().start()) as usize;
        let token_end = u32::from(token.range().end()) as usize;
        token_start >= start && token_end <= end
    }

    fn line_index(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1)
    }

    fn line_index_for_end(&self, offset: usize) -> usize {
        self.line_index(offset.saturating_sub(1))
    }
}

fn render_comment_text(comment: &AttachedComment) -> String {
    match comment.kind {
        CommentKind::Block | CommentKind::DocBlock => comment.text.clone(),
        CommentKind::Line | CommentKind::DocLine | CommentKind::Shebang => {
            comment.text.trim().to_owned()
        }
    }
}

fn hard_lines(count: usize) -> Doc {
    Doc::concat(vec![Doc::hard_line(); count])
}

fn range_contains(container: TextRange, candidate: TextRange) -> bool {
    u32::from(container.start()) <= u32::from(candidate.start())
        && u32::from(candidate.end()) <= u32::from(container.end())
}
