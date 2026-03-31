use crate::syntax::{
    RowanGreenNode, RowanGreenToken, SyntaxKind, SyntaxToken, TextRange, TextSize, TokenKind,
};
use rowan::NodeOrToken;

pub(crate) type BuildElement = NodeOrToken<BuildNode, BuildToken>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildToken {
    kind: TokenKind,
    range: TextRange,
    green: RowanGreenToken,
}

impl BuildToken {
    pub(crate) fn new(token: SyntaxToken, source: &str) -> Self {
        Self {
            kind: token.kind(),
            range: token.range(),
            green: RowanGreenToken::new(token.kind().to_rowan(), token.text(source)),
        }
    }

    pub(crate) fn kind(&self) -> TokenKind {
        self.kind
    }

    pub(crate) fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildNode {
    range: TextRange,
    green: RowanGreenNode,
}

impl BuildNode {
    pub(crate) fn new(
        kind: SyntaxKind,
        children: Vec<BuildElement>,
        fallback_offset: TextSize,
    ) -> Self {
        let range = range_for_children(&children, fallback_offset);
        Self::with_range(kind, range, children)
    }

    pub(crate) fn with_range(
        kind: SyntaxKind,
        range: TextRange,
        children: Vec<BuildElement>,
    ) -> Self {
        let green =
            RowanGreenNode::new(kind.to_rowan(), children.iter().map(build_element_to_green));
        Self { range, green }
    }

    pub(crate) fn kind(&self) -> SyntaxKind {
        SyntaxKind::from_rowan(self.green.kind()).expect("expected syntax node kind in build node")
    }

    pub(crate) fn range(&self) -> TextRange {
        self.range
    }

    pub(crate) fn into_green(self) -> RowanGreenNode {
        self.green
    }
}

pub(crate) fn node_element(node: BuildNode) -> BuildElement {
    NodeOrToken::Node(node)
}

pub(crate) fn token_element(token: SyntaxToken, source: &str) -> BuildElement {
    NodeOrToken::Token(BuildToken::new(token, source))
}

pub(crate) fn build_element_range(element: &BuildElement) -> TextRange {
    match element {
        NodeOrToken::Node(node) => node.range(),
        NodeOrToken::Token(token) => token.range(),
    }
}

pub(crate) fn range_for_children(
    children: &[BuildElement],
    fallback_offset: TextSize,
) -> TextRange {
    match (children.first(), children.last()) {
        (Some(first), Some(last)) => TextRange::new(
            build_element_range(first).start(),
            build_element_range(last).end(),
        ),
        _ => TextRange::new(fallback_offset, fallback_offset),
    }
}

fn build_element_to_green(element: &BuildElement) -> NodeOrToken<RowanGreenNode, RowanGreenToken> {
    match element {
        NodeOrToken::Node(node) => NodeOrToken::Node(node.green.clone()),
        NodeOrToken::Token(token) => NodeOrToken::Token(token.green.clone()),
    }
}
