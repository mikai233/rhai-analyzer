use std::sync::Arc;

use crate::lexer::lex_text;
use crate::syntax::{
    Parse, SyntaxError, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, text_size_of,
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
    let significant_tokens: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| !token.kind().is_trivia())
        .collect();

    let mut parser = Parser::new(significant_tokens, text_len, text);
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

    fn parse_root(&mut self) -> SyntaxNode {
        let mut children = Vec::new();

        while !self.is_eof() {
            children.push(crate::syntax::node_element(self.parse_stmt()));
        }

        SyntaxNode::with_range(
            SyntaxKind::Root,
            TextRange::new(TextSize::from(0), self.text_len),
            children,
        )
    }
}
