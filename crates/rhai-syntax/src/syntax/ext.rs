use crate::syntax::{SyntaxNode, SyntaxToken, TextRange};

pub trait SyntaxNodeExt {
    fn child_nodes(&self) -> impl Iterator<Item = SyntaxNode>;
    fn direct_raw_tokens(&self) -> impl Iterator<Item = SyntaxToken>;
    fn direct_significant_tokens(&self) -> impl Iterator<Item = SyntaxToken>;
    fn raw_tokens(&self) -> impl Iterator<Item = SyntaxToken>;
    fn significant_tokens(&self) -> impl Iterator<Item = SyntaxToken>;
    fn first_significant_token(&self) -> Option<SyntaxToken>;
    fn last_significant_token(&self) -> Option<SyntaxToken>;
    fn significant_range(&self) -> TextRange;
    fn structural_range(&self) -> TextRange;
}

impl SyntaxNodeExt for SyntaxNode {
    fn child_nodes(&self) -> impl Iterator<Item = SyntaxNode> {
        self.children()
    }

    fn direct_raw_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.children_with_tokens()
            .filter_map(|element| element.into_token())
    }

    fn direct_significant_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.direct_raw_tokens().filter(is_significant_rowan_token)
    }

    fn raw_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.descendants_with_tokens()
            .filter_map(|element| element.into_token())
    }

    fn significant_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.raw_tokens().filter(is_significant_rowan_token)
    }

    fn first_significant_token(&self) -> Option<SyntaxToken> {
        self.significant_tokens().next()
    }

    fn last_significant_token(&self) -> Option<SyntaxToken> {
        self.significant_tokens().last()
    }

    fn significant_range(&self) -> TextRange {
        match (
            self.first_significant_token(),
            self.last_significant_token(),
        ) {
            (Some(first), Some(last)) => {
                TextRange::new(first.text_range().start(), last.text_range().end())
            }
            _ => self.text_range(),
        }
    }

    fn structural_range(&self) -> TextRange {
        let end = self
            .last_significant_token()
            .map(|token| token.text_range().end())
            .unwrap_or_else(|| self.text_range().end());
        TextRange::new(
            self.text_range().start(),
            end.max(self.text_range().start()),
        )
    }
}

fn is_significant_rowan_token(token: &SyntaxToken) -> bool {
    token
        .kind()
        .token_kind()
        .is_some_and(|kind| !kind.is_trivia())
}
