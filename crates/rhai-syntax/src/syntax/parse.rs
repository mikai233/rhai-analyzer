use std::sync::Arc;

use crate::{TriviaStore, syntax::GreenNode, syntax::SyntaxError, syntax::SyntaxNode};

#[derive(Debug, Clone)]
pub struct Parse {
    text: Arc<str>,
    trivia: TriviaStore,
    green: GreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    pub(crate) fn new(text: Arc<str>, green: GreenNode, errors: Vec<SyntaxError>) -> Self {
        let root = SyntaxNode::new_root(green.clone());
        let trivia = TriviaStore::new(&text, &root);
        Self {
            text,
            trivia,
            green,
            errors,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn root(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn trivia(&self) -> &TriviaStore {
        &self.trivia
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }
}
