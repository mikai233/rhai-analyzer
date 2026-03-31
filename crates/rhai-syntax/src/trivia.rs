use crate::{RowanSyntaxElement, RowanSyntaxNode, RowanSyntaxToken, TextRange, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TriviaToken {
    kind: TokenKind,
    range: TextRange,
}

impl TriviaToken {
    fn from_rowan(token: &RowanSyntaxToken) -> Option<Self> {
        Some(Self {
            kind: token.kind().token_kind()?,
            range: token.text_range(),
        })
    }

    fn kind(self) -> TokenKind {
        self.kind
    }

    fn range(self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaSlot {
    Leading,
    Between(usize),
    Trailing,
}

#[derive(Debug, Clone)]
pub enum TriviaBoundary {
    NodeNode(RowanSyntaxNode, RowanSyntaxNode),
    NodeToken(RowanSyntaxNode, RowanSyntaxToken),
    TokenNode(RowanSyntaxToken, RowanSyntaxNode),
    TokenToken(RowanSyntaxToken, RowanSyntaxToken),
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
    trivia_tokens: Vec<TriviaToken>,
    comment_tokens: Vec<TriviaToken>,
    line_starts: Vec<usize>,
}

impl TriviaStore {
    pub fn new(source: &str, root: &RowanSyntaxNode) -> Self {
        let mut trivia_tokens = Vec::new();
        let mut comment_tokens = Vec::new();

        for token in root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
        {
            let Some(token) = TriviaToken::from_rowan(&token) else {
                continue;
            };
            if token.kind().is_trivia() {
                trivia_tokens.push(token);
                if comment_kind(token.kind()).is_some() {
                    comment_tokens.push(token);
                }
            }
        }

        Self {
            trivia_tokens,
            comment_tokens,
            line_starts: collect_line_starts(source),
        }
    }

    pub fn node_has_unowned_comments(&self, node: &RowanSyntaxNode) -> bool {
        self.node_has_unowned_comments_outside(node, &[])
    }

    pub fn node_has_unowned_comments_outside(
        &self,
        node: &RowanSyntaxNode,
        allowed_ranges: &[(usize, usize)],
    ) -> bool {
        let child_ranges = node
            .children()
            .map(|child| child.text_range())
            .collect::<Vec<_>>();
        let start = u32::from(node.text_range().start()) as usize;
        let end = u32::from(node.text_range().end()) as usize;

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
        node: &RowanSyntaxNode,
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
        node: &RowanSyntaxNode,
        boundaries: &[TriviaBoundary],
    ) -> bool {
        let mut allowed_slots = Vec::with_capacity(boundaries.len());
        for boundary in boundaries {
            let Some(slot) = self.slot_for_boundary(node, boundary) else {
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
            .any(|token| token_in_range(token, start, end))
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
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
    ) -> (usize, usize) {
        (
            offset_after(previous.text_range()),
            offset_before(next.text_range()),
        )
    }

    pub fn range_after_node_before_token(
        &self,
        previous: &RowanSyntaxNode,
        next: RowanSyntaxToken,
    ) -> (usize, usize) {
        (
            offset_after(previous.text_range()),
            offset_before(next.text_range()),
        )
    }

    pub fn range_after_token_before_node(
        &self,
        previous: RowanSyntaxToken,
        next: &RowanSyntaxNode,
    ) -> (usize, usize) {
        (
            offset_after(previous.text_range()),
            offset_before(next.text_range()),
        )
    }

    pub fn range_after_token_before_token(
        &self,
        previous: RowanSyntaxToken,
        next: RowanSyntaxToken,
    ) -> (usize, usize) {
        (
            offset_after(previous.text_range()),
            offset_before(next.text_range()),
        )
    }

    pub fn has_comments_after_node_before_node(
        &self,
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_node_before_token(
        &self,
        previous: &RowanSyntaxNode,
        next: RowanSyntaxToken,
    ) -> bool {
        let (start, end) = self.range_after_node_before_token(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_token_before_node(
        &self,
        previous: RowanSyntaxToken,
        next: &RowanSyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_token_before_node(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn has_comments_after_token_before_token(
        &self,
        previous: RowanSyntaxToken,
        next: RowanSyntaxToken,
    ) -> bool {
        let (start, end) = self.range_after_token_before_token(previous, next);
        self.range_has_comments(start, end)
    }

    pub fn is_whitespace_only_after_node_before_node(
        &self,
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_is_whitespace_only(start, end)
    }

    pub fn has_blank_line_after_node_before_node(
        &self,
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
    ) -> bool {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.range_has_blank_line(start, end)
    }

    pub fn slot_after_node_before_node(
        &self,
        owner: &RowanSyntaxNode,
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(
            owner,
            &TriviaBoundary::NodeNode(previous.clone(), next.clone()),
        )
    }

    pub fn slot_after_node_before_token(
        &self,
        owner: &RowanSyntaxNode,
        previous: &RowanSyntaxNode,
        next: RowanSyntaxToken,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, &TriviaBoundary::NodeToken(previous.clone(), next))
    }

    pub fn slot_after_token_before_node(
        &self,
        owner: &RowanSyntaxNode,
        previous: RowanSyntaxToken,
        next: &RowanSyntaxNode,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, &TriviaBoundary::TokenNode(previous, next.clone()))
    }

    pub fn slot_after_token_before_token(
        &self,
        owner: &RowanSyntaxNode,
        previous: RowanSyntaxToken,
        next: RowanSyntaxToken,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, &TriviaBoundary::TokenToken(previous, next))
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
        previous: &RowanSyntaxNode,
        next: &RowanSyntaxNode,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_node_before_node(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_node_before_token(
        &self,
        previous: &RowanSyntaxNode,
        next: RowanSyntaxToken,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_node_before_token(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_token_before_node(
        &self,
        previous: RowanSyntaxToken,
        next: &RowanSyntaxNode,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.range_after_token_before_node(previous, next);
        self.comment_gap(start, end, has_previous, has_next)
    }

    pub fn comment_gap_after_token_before_token(
        &self,
        previous: RowanSyntaxToken,
        next: RowanSyntaxToken,
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
        owner: &RowanSyntaxNode,
        boundary: &TriviaBoundary,
    ) -> Option<TriviaSlot> {
        let elements = significant_elements(owner);
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

    fn slot_range(&self, owner: &RowanSyntaxNode, slot: TriviaSlot) -> Option<(usize, usize)> {
        let elements = significant_elements(owner);
        match slot {
            TriviaSlot::Leading => {
                let first = elements.first()?;
                Some((
                    u32::from(owner.text_range().start()) as usize,
                    u32::from(element_range(first).start()) as usize,
                ))
            }
            TriviaSlot::Between(index) => {
                let left = elements.get(index)?;
                let right = elements.get(index + 1)?;
                Some((
                    u32::from(element_range(left).end()) as usize,
                    u32::from(element_range(right).start()) as usize,
                ))
            }
            TriviaSlot::Trailing => {
                let last = elements.last()?;
                Some((
                    u32::from(element_range(last).end()) as usize,
                    u32::from(owner.text_range().end()) as usize,
                ))
            }
        }
    }
}

fn significant_elements(owner: &RowanSyntaxNode) -> Vec<RowanSyntaxElement> {
    owner
        .children_with_tokens()
        .filter(|element| match element {
            rowan::NodeOrToken::Node(_) => true,
            rowan::NodeOrToken::Token(token) => token
                .kind()
                .token_kind()
                .is_some_and(|kind| !kind.is_trivia()),
        })
        .collect()
}

fn element_range(element: &RowanSyntaxElement) -> TextRange {
    match element {
        rowan::NodeOrToken::Node(node) => node.text_range(),
        rowan::NodeOrToken::Token(token) => token.text_range(),
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

fn token_in_range(token: TriviaToken, start: usize, end: usize) -> bool {
    let token_start = u32::from(token.range().start()) as usize;
    let token_end = u32::from(token.range().end()) as usize;
    token_start >= start && token_end <= end
}

fn token_in_usize_range(token: TriviaToken, start: usize, end: usize) -> bool {
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
    left: &RowanSyntaxElement,
    right: &RowanSyntaxElement,
    boundary: &TriviaBoundary,
) -> bool {
    match boundary {
        TriviaBoundary::NodeNode(previous, next) => matches!(
            (left, right),
            (rowan::NodeOrToken::Node(left_node), rowan::NodeOrToken::Node(right_node))
                if left_node == previous && right_node == next
        ),
        TriviaBoundary::NodeToken(previous, next) => matches!(
            (left, right),
            (rowan::NodeOrToken::Node(left_node), rowan::NodeOrToken::Token(right_token))
                if left_node == previous && right_token == next
        ),
        TriviaBoundary::TokenNode(previous, next) => matches!(
            (left, right),
            (rowan::NodeOrToken::Token(left_token), rowan::NodeOrToken::Node(right_node))
                if left_token == previous && right_node == next
        ),
        TriviaBoundary::TokenToken(previous, next) => matches!(
            (left, right),
            (rowan::NodeOrToken::Token(left_token), rowan::NodeOrToken::Token(right_token))
                if left_token == previous && right_token == next
        ),
    }
}
