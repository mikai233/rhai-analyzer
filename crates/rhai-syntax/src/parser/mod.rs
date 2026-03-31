use std::sync::Arc;

use crate::lexer::lex_text;
use crate::syntax::{
    Parse, SyntaxError, SyntaxKind, SyntaxToken, TextRange, TextSize, text_size_of,
};
use rowan::NodeOrToken;

mod build;
mod expr;
mod item;
mod recovery;
mod stmt;

pub(crate) use self::build::{
    BuildElement, BuildNode, build_element_range, node_element, range_for_children, token_element,
};
pub(crate) use crate::parser::recovery::infix_binding_power;

pub fn parse_text(text: &str) -> Parse {
    let lexed = lex_text(text);
    let (tokens, mut errors) = lexed.into_parts();
    let text_len = text_size_of(text);
    let mut parser = Parser::new(tokens.clone(), text_len, text);
    let root = parser.parse_root();
    errors.extend(parser.finish_errors());

    Parse::new(Arc::<str>::from(text), root.into_green(), errors)
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
        children: Vec<BuildElement>,
        fallback_offset: TextSize,
    ) -> BuildNode {
        self.finish_node_with_range(
            kind,
            range_for_children(&children, fallback_offset),
            children,
        )
    }

    fn finish_node_with_range(
        &self,
        kind: SyntaxKind,
        range: TextRange,
        children: Vec<BuildElement>,
    ) -> BuildNode {
        let mut cursor = self
            .tokens
            .partition_point(|token| token.range().end() <= range.start());
        let mut attached_children = Vec::new();

        for child in children {
            let child_start = build_element_range(&child).start();
            while let Some(token) = self.tokens.get(cursor).copied() {
                if token.range().start() >= child_start || token.range().end() > range.end() {
                    break;
                }
                if token.kind().is_trivia() {
                    attached_children.push(token_element(token, self.source));
                }
                cursor += 1;
            }

            match child {
                BuildElement::Node(node) => {
                    let node_end = node.range().end();
                    attached_children.push(node_element(node));
                    while let Some(token) = self.tokens.get(cursor) {
                        if token.range().end() <= node_end {
                            cursor += 1;
                        } else {
                            break;
                        }
                    }
                }
                BuildElement::Token(token) => {
                    let token_kind = token.kind();
                    let token_range = token.range();
                    attached_children.push(NodeOrToken::Token(token));
                    if self.tokens.get(cursor).is_some_and(|current| {
                        current.kind() == token_kind && current.range() == token_range
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
                attached_children.push(token_element(token, self.source));
            }
            cursor += 1;
        }

        BuildNode::with_range(kind, range, attached_children)
    }

    fn parse_root(&mut self) -> BuildNode {
        self.parse_root_with_range(TextRange::new(TextSize::from(0), self.text_len))
    }

    fn parse_root_with_range(&mut self, range: TextRange) -> BuildNode {
        let children = vec![node_element(self.parse_statement_list_with_range(
            SyntaxKind::RootItemList,
            range.start(),
            None,
            false,
        ))];

        self.finish_node_with_range(SyntaxKind::Root, range, children)
    }

    fn parse_statement_list_with_range(
        &mut self,
        kind: SyntaxKind,
        start: TextSize,
        stop_at: Option<crate::syntax::TokenKind>,
        nested_statement_scope: bool,
    ) -> BuildNode {
        let mut item_children = Vec::new();

        if nested_statement_scope {
            self.statement_depth += 1;
        }

        while !self.is_eof() && stop_at.is_none_or(|kind| !self.at(kind)) {
            item_children.push(node_element(self.parse_stmt()));
        }

        if nested_statement_scope {
            self.statement_depth = self.statement_depth.saturating_sub(1);
        }

        let range = TextRange::new(start, self.next_significant_offset());
        self.finish_node_with_range(kind, range, item_children)
    }
}
