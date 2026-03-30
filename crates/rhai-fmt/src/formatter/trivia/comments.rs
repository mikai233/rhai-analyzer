use crate::{FormatOptions, IndentStyle};
use rhai_syntax::{AliasClause, SyntaxNode, TextRange, TokenKind};

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn alias_name(&self, alias: AliasClause<'_>) -> Option<&str> {
        alias.alias_token().map(|token| token.text(self.source))
    }

    pub(crate) fn format_comment_region(
        &self,
        start: usize,
        end: usize,
        indent: usize,
    ) -> Option<String> {
        if start >= end || end > self.source.len() {
            return None;
        }

        let slice = &self.source[start..end];
        let mut lines = Vec::<String>::new();
        let mut in_block_comment = false;
        let mut pending_blank = false;

        for raw_line in slice.lines() {
            let trimmed = raw_line.trim();
            let trimmed_start = raw_line.trim_start();
            let is_comment_line = in_block_comment
                || trimmed_start.starts_with("//")
                || trimmed_start.starts_with("#!")
                || trimmed_start.starts_with("/*");

            if is_comment_line {
                if pending_blank && !lines.is_empty() {
                    lines.push(String::new());
                    pending_blank = false;
                }
                lines.push(format!(
                    "{}{}",
                    indent_text(self.options, indent),
                    trimmed_start.trim_end()
                ));
            } else if trimmed.is_empty() && !lines.is_empty() {
                pending_blank = true;
            }

            if in_block_comment {
                if trimmed_start.contains("*/") {
                    in_block_comment = false;
                }
            } else if trimmed_start.starts_with("/*") && !trimmed_start.contains("*/") {
                in_block_comment = true;
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    pub(crate) fn token_range(&self, node: &SyntaxNode, kind: TokenKind) -> Option<TextRange> {
        node.children()
            .iter()
            .filter_map(|child| child.as_token())
            .find(|token| token.kind() == kind)
            .map(|token| token.range())
    }

    pub(crate) fn indent(&self, level: usize) -> String {
        indent_text(self.options, level)
    }

    pub(crate) fn separator_doc(&self, minimum_newlines: usize, start: usize, end: usize) -> Doc {
        Doc::concat(vec![
            Doc::hard_line();
            self.separator_count(minimum_newlines, start, end)
        ])
    }

    fn separator_count(&self, minimum_newlines: usize, start: usize, end: usize) -> usize {
        minimum_newlines.max(self.extra_blank_line_count(start, end) + 1)
    }

    fn extra_blank_line_count(&self, start: usize, end: usize) -> usize {
        if start >= end || end > self.source.len() {
            return 0;
        }

        let slice = &self.source[start..end];
        let mut lines = slice.lines().collect::<Vec<_>>();
        if lines.is_empty() {
            return 0;
        }

        lines.remove(0);
        if !slice.ends_with('\n') && lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }

        lines
            .into_iter()
            .filter(|line| line.trim().is_empty())
            .count()
    }
}

fn indent_text(options: &FormatOptions, level: usize) -> String {
    match options.indent_style {
        IndentStyle::Spaces => " ".repeat(level * options.indent_width),
        IndentStyle::Tabs => "\t".repeat(level),
    }
}
