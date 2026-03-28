pub use text_size::{TextRange, TextSize};

use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    Root,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxNode {
    kind: SyntaxKind,
    range: TextRange,
}

impl SyntaxNode {
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct SyntaxError {
    message: String,
    range: TextRange,
}

impl SyntaxError {
    pub fn new(message: impl Into<String>, range: TextRange) -> Self {
        Self {
            message: message.into(),
            range,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Debug, Clone)]
pub struct Parse {
    text: Arc<str>,
    root: SyntaxNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    pub fn root(&self) -> SyntaxNode {
        self.root
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }
}

pub fn parse_text(text: &str) -> Parse {
    let len = u32::try_from(text.len()).unwrap_or(u32::MAX);

    Parse {
        text: Arc::<str>::from(text),
        root: SyntaxNode {
            kind: SyntaxKind::Root,
            range: TextRange::new(TextSize::from(0), TextSize::from(len)),
        },
        errors: Vec::new(),
    }
}
