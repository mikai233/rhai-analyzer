use crate::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TriviaToken {
    kind: TokenKind,
    range: TextRange,
}

impl TriviaToken {
    fn from_rowan(token: &SyntaxToken) -> Option<Self> {
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
    NodeNode(SyntaxNode, SyntaxNode),
    NodeToken(SyntaxNode, SyntaxToken),
    TokenNode(SyntaxToken, SyntaxNode),
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
        source.get(start..end).unwrap_or("")
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedTrivia {
    pub leading: GapTrivia,
    pub between: Vec<GapTrivia>,
    pub trailing: GapTrivia,
}

impl OwnedTrivia {
    pub fn slot(&self, slot: TriviaSlot) -> Option<&GapTrivia> {
        match slot {
            TriviaSlot::Leading => Some(&self.leading),
            TriviaSlot::Between(index) => self.between.get(index),
            TriviaSlot::Trailing => Some(&self.trailing),
        }
    }

    pub fn has_unowned_comments_outside_slots(&self, allowed_slots: &[TriviaSlot]) -> bool {
        if !allowed_slots.contains(&TriviaSlot::Leading) && self.leading.has_comments() {
            return true;
        }

        if !allowed_slots.contains(&TriviaSlot::Trailing) && self.trailing.has_comments() {
            return true;
        }

        self.between.iter().enumerate().any(|(index, gap)| {
            gap.has_comments() && !allowed_slots.contains(&TriviaSlot::Between(index))
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriviaStore {
    trivia_tokens: Vec<TriviaToken>,
    comment_tokens: Vec<TriviaToken>,
    line_starts: Vec<usize>,
}

impl TriviaStore {
    pub fn new(source: &str, root: &SyntaxNode) -> Self {
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

    pub fn node_has_unowned_comments(&self, node: &SyntaxNode) -> bool {
        self.owned_trivia(node)
            .has_unowned_comments_outside_slots(&[])
    }

    pub fn node_has_unowned_comments_outside_slots(
        &self,
        node: &SyntaxNode,
        allowed_slots: &[TriviaSlot],
    ) -> bool {
        self.owned_trivia(node)
            .has_unowned_comments_outside_slots(allowed_slots)
    }

    pub fn node_has_unowned_comments_outside_boundaries(
        &self,
        node: &SyntaxNode,
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

    pub fn boundary_range(&self, boundary: &TriviaBoundary) -> (usize, usize) {
        match boundary {
            TriviaBoundary::NodeNode(previous, next) => (
                offset_after(previous.text_range()),
                offset_before(next.text_range()),
            ),
            TriviaBoundary::NodeToken(previous, next) => (
                offset_after(previous.text_range()),
                offset_before(next.text_range()),
            ),
            TriviaBoundary::TokenNode(previous, next) => (
                offset_after(previous.text_range()),
                offset_before(next.text_range()),
            ),
            TriviaBoundary::TokenToken(previous, next) => (
                offset_after(previous.text_range()),
                offset_before(next.text_range()),
            ),
        }
    }

    pub fn boundary_has_comments(&self, boundary: &TriviaBoundary) -> bool {
        let (start, end) = self.boundary_range(boundary);
        self.range_has_comments(start, end)
    }

    pub fn boundary_is_whitespace_only(&self, boundary: &TriviaBoundary) -> bool {
        let (start, end) = self.boundary_range(boundary);
        self.range_is_whitespace_only(start, end)
    }

    pub fn boundary_has_blank_line(&self, boundary: &TriviaBoundary) -> bool {
        let (start, end) = self.boundary_range(boundary);
        self.range_has_blank_line(start, end)
    }

    pub fn boundary_slot(
        &self,
        owner: &SyntaxNode,
        boundary: &TriviaBoundary,
    ) -> Option<TriviaSlot> {
        self.slot_for_boundary(owner, boundary)
    }

    pub fn trivia_for_slot(&self, owner: &SyntaxNode, slot: TriviaSlot) -> Option<GapTrivia> {
        let (start, end) = self.slot_range(owner, slot)?;
        Some(self.comment_gap(
            start,
            end,
            !matches!(slot, TriviaSlot::Leading),
            !matches!(slot, TriviaSlot::Trailing),
        ))
    }

    pub fn trivia_for_boundary(
        &self,
        owner: &SyntaxNode,
        boundary: &TriviaBoundary,
    ) -> Option<GapTrivia> {
        let slot = self.boundary_slot(owner, boundary)?;
        self.trivia_for_slot(owner, slot)
    }

    pub fn owned_trivia(&self, owner: &SyntaxNode) -> OwnedTrivia {
        let slot_count = significant_elements(owner).len().saturating_sub(1);
        OwnedTrivia {
            leading: self
                .trivia_for_slot(owner, TriviaSlot::Leading)
                .unwrap_or_default(),
            between: (0..slot_count)
                .map(|index| {
                    self.trivia_for_slot(owner, TriviaSlot::Between(index))
                        .unwrap_or_default()
                })
                .collect(),
            trailing: self
                .trivia_for_slot(owner, TriviaSlot::Trailing)
                .unwrap_or_default(),
        }
    }

    pub fn owned_trivia_for_sequence(
        &self,
        start: usize,
        end: usize,
        elements: &[SyntaxElement],
    ) -> OwnedTrivia {
        let ranges = elements
            .iter()
            .map(|element| {
                let range = element_range(element);
                (
                    u32::from(range.start()) as usize,
                    u32::from(range.end()) as usize,
                )
            })
            .collect::<Vec<_>>();
        self.owned_trivia_for_ranges(start, end, &ranges)
    }

    pub fn owned_trivia_for_ranges(
        &self,
        start: usize,
        end: usize,
        ranges: &[(usize, usize)],
    ) -> OwnedTrivia {
        if ranges.is_empty() {
            return OwnedTrivia {
                leading: self.comment_gap(start, end, false, false),
                between: Vec::new(),
                trailing: GapTrivia::default(),
            };
        }

        let leading = self.comment_gap(start, ranges[0].0, false, true);
        let between = ranges
            .windows(2)
            .map(|pair| {
                let [left, right] = pair else {
                    unreachable!("windows(2) always yields pairs");
                };
                self.comment_gap(left.1, right.0, true, true)
            })
            .collect();
        let trailing = self.comment_gap(
            ranges.last().expect("non-empty checked above").1,
            end,
            true,
            false,
        );

        OwnedTrivia {
            leading,
            between,
            trailing,
        }
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

    pub fn comment_gap_for_boundary(
        &self,
        boundary: &TriviaBoundary,
        has_previous: bool,
        has_next: bool,
    ) -> GapTrivia {
        let (start, end) = self.boundary_range(boundary);
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

    fn slot_range(&self, owner: &SyntaxNode, slot: TriviaSlot) -> Option<(usize, usize)> {
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

fn significant_elements(owner: &SyntaxNode) -> Vec<SyntaxElement> {
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

fn element_range(element: &SyntaxElement) -> TextRange {
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
    left: &SyntaxElement,
    right: &SyntaxElement,
    boundary: &TriviaBoundary,
) -> bool {
    match boundary {
        TriviaBoundary::NodeNode(previous, next) => {
            owner_element_matches_node(left, previous) && owner_element_matches_node(right, next)
        }
        TriviaBoundary::NodeToken(previous, next) => {
            owner_element_matches_node(left, previous) && owner_element_matches_token(right, next)
        }
        TriviaBoundary::TokenNode(previous, next) => {
            owner_element_matches_token(left, previous) && owner_element_matches_node(right, next)
        }
        TriviaBoundary::TokenToken(previous, next) => {
            owner_element_matches_token(left, previous) && owner_element_matches_token(right, next)
        }
    }
}

fn owner_element_matches_node(element: &SyntaxElement, node: &SyntaxNode) -> bool {
    match element {
        rowan::NodeOrToken::Node(owner_node) => {
            owner_node == node || range_contains(owner_node.text_range(), node.text_range())
        }
        rowan::NodeOrToken::Token(_) => false,
    }
}

fn owner_element_matches_token(element: &SyntaxElement, token: &SyntaxToken) -> bool {
    match element {
        rowan::NodeOrToken::Node(owner_node) => {
            range_contains(owner_node.text_range(), token.text_range())
        }
        rowan::NodeOrToken::Token(owner_token) => owner_token == token,
    }
}
