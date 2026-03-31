use std::sync::Arc;

use crate::lexer::lex_text;
use crate::syntax::{
    Parse, SyntaxError, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, text_size_of,
    token_element,
};

mod expr;
mod item;
mod recovery;
mod stmt;

pub(crate) use crate::parser::recovery::{
    InterpolatedPart, find_interpolation_end, infix_binding_power, make_absolute_range,
    next_char_at, shift_element, shift_error,
};

pub fn parse_text(text: &str) -> Parse {
    let lexed = lex_text(text);
    let (tokens, mut errors) = lexed.into_parts();
    let text_len = text_size_of(text);
    let mut parser = Parser::new(tokens.clone(), text_len, text);
    let root = parser.parse_root();
    errors.extend(parser.finish_errors());

    Parse::new(Arc::<str>::from(text), tokens, root, errors)
}

struct Parser<'a> {
    tokens: Vec<SyntaxToken>,
    cursor: usize,
    text_len: TextSize,
    errors: Vec<SyntaxError>,
    source: &'a str,
    statement_depth: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<SyntaxToken>, text_len: TextSize, source: &'a str) -> Self {
        Self {
            tokens,
            cursor: 0,
            text_len,
            errors: Vec::new(),
            source,
            statement_depth: 0,
        }
    }

    fn finish_errors(self) -> Vec<SyntaxError> {
        self.errors
    }

    fn finish_node(
        &self,
        kind: SyntaxKind,
        children: Vec<crate::SyntaxElement>,
        fallback_offset: TextSize,
    ) -> SyntaxNode {
        let base = SyntaxNode::new(kind, children, fallback_offset);
        self.finish_node_with_range(kind, base.range(), base.raw_children().to_vec())
    }

    fn finish_node_with_range(
        &self,
        kind: SyntaxKind,
        range: TextRange,
        children: Vec<crate::SyntaxElement>,
    ) -> SyntaxNode {
        let mut cursor = self
            .tokens
            .partition_point(|token| token.range().end() <= range.start());
        let mut attached_children = Vec::new();

        for child in children {
            let child_start = child.range().start();
            while let Some(token) = self.tokens.get(cursor).copied() {
                if token.range().start() >= child_start || token.range().end() > range.end() {
                    break;
                }
                if token.kind().is_trivia() {
                    attached_children.push(token_element(token));
                }
                cursor += 1;
            }

            match child {
                crate::SyntaxElement::Node(node) => {
                    let node_end = node.range().end();
                    attached_children.push(crate::syntax::node_element(*node));
                    while let Some(token) = self.tokens.get(cursor) {
                        if token.range().end() <= node_end {
                            cursor += 1;
                        } else {
                            break;
                        }
                    }
                }
                crate::SyntaxElement::Token(token) => {
                    attached_children.push(token_element(token));
                    if self.tokens.get(cursor).is_some_and(|current| {
                        current.kind() == token.kind() && current.range() == token.range()
                    }) {
                        cursor += 1;
                    }
                }
            }
        }

        while let Some(token) = self.tokens.get(cursor).copied() {
            if token.range().end() > range.end() {
                break;
            }
            if token.kind().is_trivia() {
                attached_children.push(token_element(token));
            }
            cursor += 1;
        }

        SyntaxNode::with_range(kind, range, attached_children)
    }

    fn parse_root(&mut self) -> SyntaxNode {
        let mut item_children = Vec::new();

        while !self.is_eof() {
            item_children.push(crate::syntax::node_element(self.parse_stmt()));
        }

        let children = vec![crate::syntax::node_element(self.finish_node_with_range(
            SyntaxKind::RootItemList,
            TextRange::new(TextSize::from(0), self.text_len),
            item_children,
        ))];

        self.finish_node_with_range(
            SyntaxKind::Root,
            TextRange::new(TextSize::from(0), self.text_len),
            children,
        )
    }
}
