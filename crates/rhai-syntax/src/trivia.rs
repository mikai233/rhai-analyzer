use crate::{SyntaxNode, SyntaxToken, TextRange, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaSlot {
    Leading,
    Between(usize),
    Trailing,
}

#[derive(Debug, Clone, Copy)]
pub enum TriviaBoundary<'a> {
    NodeNode(&'a SyntaxNode, &'a SyntaxNode),
    NodeToken(&'a SyntaxNode, SyntaxToken),
    TokenNode(SyntaxToken, &'a SyntaxNode),
    TokenToken(SyntaxToken, SyntaxToken),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    DocLine,
    Block,
    DocBlock,
    Shebang,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedComment {
    pub kind: CommentKind,
    pub range: TextRange,
    pub blank_lines_before: usize,
}

impl AttachedComment {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        let start = u32::from(self.range.start()) as usize;
        let end = u32::from(self.range.end()) as usize;
        &source[start..end]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GapTrivia {
    pub trailing_comments: Vec<AttachedComment>,
    pub leading_comments: Vec<AttachedComment>,
    pub dangling_comments: Vec<AttachedComment>,
    pub trailing_blank_lines_before_next: usize,
}

impl GapTrivia {
    pub fn has_vertical_comments(&self) -> bool {
        !self.leading_comments.is_empty() || !self.dangling_comments.is_empty()
    }

    pub fn has_comments(&self) -> bool {
        !self.trailing_comments.is_empty() || self.has_vertical_comments()
    }

    pub fn vertical_comments(&self) -> &[AttachedComment] {
        if !self.leading_comments.is_empty() {
            &self.leading_comments
        } else {
            &self.dangling_comments
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriviaStore {
    trivia_tokens: Vec<SyntaxToken>,
    comment_tokens: Vec<SyntaxToken>,
    line_starts: Vec<usize>,
}

impl TriviaStore {
    pub fn new(source: &str, tokens: &[SyntaxToken]) -> Self {
        Self {
            trivia_tokens: tokens
                .iter()
                .copied()
                .filter(|token| token.kind().is_trivia())
                .collect(),
            comment_tokens: tokens
                .iter()
                .copied()
                .filter(|token| comment_kind(token.kind()).is_some())
                .collect(),
            line_starts: collect_line_starts(source),
        }
    }

    pub fn node_has_unowned_comments(&self, node: &SyntaxNode) -> bool {
        self.node_has_unowned_comments_outside(node, &[])
    }

    pub fn node_has_unowned_comments_outside(
        &self,
        node: &SyntaxNode,
        allowed_ranges: &[(usize, usize)],
    ) -> bool {
        let child_ranges = node
            .children()
            .iter()
            .filter_map(|child| child.as_node())
            .map(|child| child.range())
            .collect::<Vec<_>>();

        let start = u32::from(node.range().start()) as usize;
        let end = u32::from(node.range().end()) as usize;

        self.comment_tokens
            .iter()
            .copied()
            .filter(|token| token_in_range(*token, start, end))
            .any(|token| {
                !child_ranges
                    .iter()
                    .any(|range| range_contains(*range, token.range()))
                    && !allowed_ranges.iter().any(|(allowed_start, allowed_end)| {
                        token_in_usize_range(token, *allowed_start, *allowed_end)
                    })
            })
    }

    pub fn node_has_unowned_comments_outside_slots(
        &self,
        node: &SyntaxNode,
        allowed_slots: &[TriviaSlot],
    ) -> bool {
        let allowed_ranges = allowed_slots
            .iter()
            .filter_map(|slot| self.slot_range(node, *slot))
            .collect::<Vec<_>>();
        self.node_has_unowned_comments_outside(node, &allowed_ranges)
    }

    pub fn node_has_unowned_comments_outside_boundaries(
        &self,
        node: &SyntaxNode,
        boundaries: &[TriviaBoundary<'_>],
    ) -> bool {
        let mut allowed_slots = Vec::with_capacity(boundaries.len());
        for boundary in boundaries {
            let Some(slot) = self.slot_for_boundary(node, *boundary) else {
                return true;
            };
            allowed_slots.push(slot);
        }
        self.node_has_unowned_comments_outside_slots(node, &allowed_slots)
    }

    pub fn range_has_comments(&self, start: usize, end: usize) -> bool {
        self.comment_tokens
            .iter()
            .copied()
            .find(|token| token_in_range(*token, start, end))
            .is_some()
    }

    pub fn range_is_whitespace_only(&self, start: usize, end: usize) -> bool {
        self.trivia_tokens
            .iter()
            .copied()
            .filter(|token| token_in_range(*token, start, end))
            .all(|token| token.kind() == TokenKind::Whitespace)
    }

    pub fn range_has_blank_line(&self, start: usize, end: usize) -> bool {
        if start >= end {
            return false;
        }

        self.line_index(end).saturating_sub(self.line_index(start)) >= 2
    }

    pub fn range_after_node_before_node(
        &self,
        previous: &SyntaxNode,
        next: &SyntaxNode,
    ) -> (usize, usize) {
        (offset_after(previous.range()), offset_before(next.range()))
    }

    pub fn range_after_node_before_token(
        &self,
        previous: &SyntaxNode,
        next: SyntaxToken,
    ) -> (usize, usize) {
        (offset_after(previous.range()), offset_before(next.range()))
    }

    pub fn range_after_token_before_node(
        &self,
        previous: SyntaxToken,
        next: &SyntaxNode,
    ) -> (usize, usize) {
        (offset_after(previous.range()), offset_before(next.range()))
    }

    pub fn range_after_token_before_token(
        &self,
        previous: SyntaxToken,
        next: SyntaxToken,
    ) -> (usize, usize) {
        (offset_after(previous.range()), offset_before(next.range()))
    }

    pub fn has_comments_after_node_before_node(
        &self,
        previous: &SyntaxNode,
        next: &SyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_node_before_token(
        &self,
        previous: &SyntaxNode,
        next: SyntaxToken,
    ) -> bool {
        let (start, end) = self.range_after_node_before_token(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_token_before_node(
        &self,
        previous: SyntaxToken,
        next: &SyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_token_before_node(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_token_before_token(
        &self,
        previous: SyntaxToken,
        next: SyntaxToken,
    ) -> bool {
        let (start, end) = self.range_after_token_before_token(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn is_whitespace_only_after_node_before_node(
        &self,
        previous: &SyntaxNode,
        next: &SyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_is_whitespace_only(start, end)
    }

    pub fn has_blank_line_after_node_before_node(
        &self,
        previous: &SyntaxNode,
        next: &SyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_has_blank_line(start, end)
    }

    pub fn slot_after_node_before_node(
        &self,
        owner: &SyntaxNode,
        previous: &SyntaxNode,
        next: &SyntaxNode,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, TriviaBoundary::NodeNode(previous, next))
    }

    pub fn slot_after_node_before_token(
        &self,
        owner: &SyntaxNode,
        previous: &SyntaxNode,
        next: SyntaxToken,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, TriviaBoundary::NodeToken(previous, next))
    }

    pub fn slot_after_token_before_node(
        &self,
        owner: &SyntaxNode,
        previous: SyntaxToken,
        next: &SyntaxNode,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, TriviaBoundary::TokenNode(previous, next))
    }

    pub fn slot_after_token_before_token(
        &self,
        owner: &SyntaxNode,
        previous: SyntaxToken,
        next: SyntaxToken,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, TriviaBoundary::TokenToken(previous, next))
    }

    pub fn comment_gap(
        &self,
        start: usize,
        end: usize,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        if start >= end {
            return GapTrivia::default();
        }

        let start_line = self.line_index(start);
        let mut cursor_line = start_line;
        let mut trailing_comments = Vec::new();
        let mut leading_comments = Vec::new();
        let mut dangling_comments = Vec::new();

        for token in self
            .comment_tokens
            .iter()
            .copied()
            .filter(|token| token_in_range(*token, start, end))
        {
            let Some(kind) = comment_kind(token.kind()) else {
                continue;
            };

            let comment_start_line = self.line_index(u32::from(token.range().start()) as usize);
            let comment_end_line = self.line_index_for_end(u32::from(token.range().end()) as usize);

            if has_previous
                && comment_start_line == start_line
                && leading_comments.is_empty()
                && dangling_comments.is_empty()
            {
                trailing_comments.push(AttachedComment {
                    kind,
                    range: token.range(),
                    blank_lines_before: 0,
                });
                cursor_line = comment_end_line;
                continue;
            }

            let attached_comment = AttachedComment {
                kind,
                range: token.range(),
                blank_lines_before: comment_start_line.saturating_sub(cursor_line + 1),
            };
            if has_next {
                leading_comments.push(attached_comment);
            } else {
                dangling_comments.push(attached_comment);
            }
            cursor_line = comment_end_line;
        }

        let next_line = self.line_index(end);
        let trailing_blank_lines_before_next = next_line.saturating_sub(cursor_line + 1);

        GapTrivia {
            trailing_comments,
            leading_comments,
            dangling_comments,
            trailing_blank_lines_before_next,
        }
    }

    pub fn comment_gap_after_node_before_node(
        &self,
        previous: &SyntaxNode,
        next: &SyntaxNode,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_node_before_token(
        &self,
        previous: &SyntaxNode,
        next: SyntaxToken,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_node_before_token(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_token_before_node(
        &self,
        previous: SyntaxToken,
        next: &SyntaxNode,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_token_before_node(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_token_before_token(
        &self,
        previous: SyntaxToken,
        next: SyntaxToken,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_token_before_token(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    fn line_index(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1)
    }

    fn line_index_for_end(&self, offset: usize) -> usize {
        self.line_index(offset.saturating_sub(1))
    }

    fn slot_for_boundary(
        &self,
        owner: &SyntaxNode,
        boundary: TriviaBoundary<'_>,
    ) -> Option<TriviaSlot> {
        let elements = owner.significant_children().collect::<Vec<_>>();
        for (index, pair) in elements.windows(2).enumerate() {
            let [left, right] = pair else {
                continue;
            };
            if boundary_matches(left, right, boundary) {
                return Some(TriviaSlot::Between(index));
            }
        }

        None
    }

    fn slot_range(&self, owner: &SyntaxNode, slot: TriviaSlot) -> Option<(usize, usize)> {
        let elements = owner.significant_children().collect::<Vec<_>>();
        match slot {
            TriviaSlot::Leading => {
                let first = elements.first()?;
                Some((
                    u32::from(owner.range().start()) as usize,
                    u32::from(first.range().start()) as usize,
                ))
            }
            TriviaSlot::Between(index) => {
                let left = elements.get(index)?;
                let right = elements.get(index + 1)?;
                Some((
                    u32::from(left.range().end()) as usize,
                    u32::from(right.range().start()) as usize,
                ))
            }
            TriviaSlot::Trailing => {
                let last = elements.last()?;
                Some((
                    u32::from(last.range().end()) as usize,
                    u32::from(owner.structural_range().end()) as usize,
                ))
            }
        }
    }
}

fn comment_kind(kind: TokenKind) -> Option<CommentKind> {
    match kind {
        TokenKind::LineComment => Some(CommentKind::Line),
        TokenKind::DocLineComment => Some(CommentKind::DocLine),
        TokenKind::BlockComment => Some(CommentKind::Block),
        TokenKind::DocBlockComment => Some(CommentKind::DocBlock),
        TokenKind::Shebang => Some(CommentKind::Shebang),
        _ => None,
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

fn token_in_range(token: SyntaxToken, start: usize, end: usize) -> bool {
    let token_start = u32::from(token.range().start()) as usize;
    let token_end = u32::from(token.range().end()) as usize;
    token_start >= start && token_end <= end
}

fn token_in_usize_range(token: SyntaxToken, start: usize, end: usize) -> bool {
    let token_start = u32::from(token.range().start()) as usize;
    let token_end = u32::from(token.range().end()) as usize;
    start <= token_start && token_end <= end
}

fn offset_before(range: TextRange) -> usize {
    u32::from(range.start()) as usize
}

fn offset_after(range: TextRange) -> usize {
    u32::from(range.end()) as usize
}

fn range_contains(container: TextRange, candidate: TextRange) -> bool {
    u32::from(container.start()) <= u32::from(candidate.start())
        && u32::from(candidate.end()) <= u32::from(container.end())
}

fn boundary_matches(
    left: &crate::SyntaxElement,
    right: &crate::SyntaxElement,
    boundary: TriviaBoundary<'_>,
) -> bool {
    match boundary {
        TriviaBoundary::NodeNode(previous, next) => {
            left.as_node()
                .is_some_and(|node| std::ptr::eq(node, previous))
                && right.as_node().is_some_and(|node| std::ptr::eq(node, next))
        }
        TriviaBoundary::NodeToken(previous, next) => {
            left.as_node()
                .is_some_and(|node| std::ptr::eq(node, previous))
                && right.as_token().is_some_and(|token| token == next)
        }
        TriviaBoundary::TokenNode(previous, next) => {
            left.as_token().is_some_and(|token| token == previous)
                && right.as_node().is_some_and(|node| std::ptr::eq(node, next))
        }
        TriviaBoundary::TokenToken(previous, next) => {
            left.as_token().is_some_and(|token| token == previous)
                && right.as_token().is_some_and(|token| token == next)
        }
    }
}
