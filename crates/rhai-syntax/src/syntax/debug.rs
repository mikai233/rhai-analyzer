use crate::syntax::{NodeOrToken, Parse, SyntaxNode, SyntaxToken, TextRange};

impl Parse {
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        self.write_syntax_node(&mut out, &self.root(), 0);
        out
    }

    pub fn debug_tree_compact(&self) -> String {
        let mut out = String::new();
        self.write_compact_syntax_node(&mut out, &self.root(), 0);
        out
    }

    fn write_syntax_node(&self, out: &mut String, node: &SyntaxNode, indent: usize) {
        let Some(kind) = node.kind().syntax_kind() else {
            return;
        };
        push_indent(out, indent);
        push_range(out, node.text_range());
        out.push_str(&format!("{kind:?}\n"));

        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(node) => self.write_syntax_node(out, &node, indent + 2),
                NodeOrToken::Token(token) => {
                    let Some(kind) = token.kind().token_kind() else {
                        continue;
                    };
                    if kind.is_trivia() {
                        continue;
                    }
                    push_indent(out, indent + 2);
                    push_range(out, token.text_range());
                    out.push_str(&format!("{kind:?} {:?}\n", token.text()));
                }
            }
        }
    }

    fn write_compact_syntax_node(&self, out: &mut String, node: &SyntaxNode, indent: usize) {
        let Some(kind) = node.kind().syntax_kind() else {
            return;
        };
        push_indent(out, indent);
        out.push_str(&format!("{kind:?}\n"));

        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(node) => self.write_compact_syntax_node(out, &node, indent + 2),
                NodeOrToken::Token(token) => write_compact_token(out, &token, indent + 2),
            }
        }
    }
}

fn write_compact_token(out: &mut String, token: &SyntaxToken, indent: usize) {
    let Some(kind) = token.kind().token_kind() else {
        return;
    };
    if kind.is_trivia() {
        return;
    }
    push_indent(out, indent);
    out.push_str(&format!("{kind:?} {:?}\n", token.text()));
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

fn push_range(out: &mut String, range: TextRange) {
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    out.push_str(&format!("{start}..{end} "));
}
