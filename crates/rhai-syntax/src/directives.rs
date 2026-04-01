use crate::syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDirective {
    pub range: TextRange,
    pub kind: CommentDirectiveKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentDirectiveKind {
    Extern { name: String, ty: Option<String> },
    Module { name: String },
    AllowUnresolved { name: String },
    AllowUnresolvedImport { name: String },
    FormatSkip,
}

pub fn collect_comment_directives(root: &SyntaxNode, source: &str) -> Vec<CommentDirective> {
    root.raw_tokens()
        .filter_map(|token| {
            let kind = token.kind().token_kind()?;
            if !matches!(
                kind,
                TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::DocLineComment
                    | TokenKind::DocBlockComment
            ) {
                return None;
            }
            let start = u32::from(token.text_range().start()) as usize;
            let end = u32::from(token.text_range().end()) as usize;
            let text = source.get(start..end)?;
            Some(parse_comment_directives(text, token.text_range()))
        })
        .flatten()
        .collect()
}

fn parse_comment_directives(text: &str, range: TextRange) -> Vec<CommentDirective> {
    comment_payload_lines(text)
        .filter_map(|line| parse_directive_line(line, range))
        .collect()
}

fn parse_directive_line(line: &str, range: TextRange) -> Option<CommentDirective> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("rhai-fmt:") {
        let command = rest.trim();
        if command == "skip" {
            return Some(CommentDirective {
                range,
                kind: CommentDirectiveKind::FormatSkip,
            });
        }
        return None;
    }

    let rest = line.strip_prefix("rhai:")?.trim();

    if let Some(rest) = rest.strip_prefix("extern ") {
        let rest = rest.trim();
        let (name, ty) = rest
            .split_once(": ")
            .map_or((rest, None), |(name, ty)| (name.trim(), Some(ty.trim())));
        if name.is_empty() {
            return None;
        }
        let ty = ty.filter(|ty| !ty.is_empty()).map(str::to_owned);
        return Some(CommentDirective {
            range,
            kind: CommentDirectiveKind::Extern {
                name: name.to_owned(),
                ty,
            },
        });
    }

    if let Some(rest) = rest.strip_prefix("module ") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        return Some(CommentDirective {
            range,
            kind: CommentDirectiveKind::Module {
                name: name.to_owned(),
            },
        });
    }

    if let Some(rest) = rest.strip_prefix("allow unresolved-import ") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        return Some(CommentDirective {
            range,
            kind: CommentDirectiveKind::AllowUnresolvedImport {
                name: name.to_owned(),
            },
        });
    }

    if let Some(rest) = rest.strip_prefix("allow unresolved ") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        return Some(CommentDirective {
            range,
            kind: CommentDirectiveKind::AllowUnresolved {
                name: name.to_owned(),
            },
        });
    }

    None
}

fn comment_payload_lines(text: &str) -> impl Iterator<Item = &str> {
    let text = text
        .strip_prefix("///")
        .or_else(|| text.strip_prefix("//!"))
        .or_else(|| text.strip_prefix("//"))
        .or_else(|| text.strip_prefix("/**"))
        .or_else(|| text.strip_prefix("/*!"))
        .or_else(|| text.strip_prefix("/*"))
        .unwrap_or(text);
    let text = text.strip_suffix("*/").unwrap_or(text);
    text.lines().map(strip_comment_line_prefix)
}

fn strip_comment_line_prefix(line: &str) -> &str {
    line.trim_start()
        .strip_prefix('*')
        .map_or(line.trim_start(), str::trim_start)
}

#[cfg(test)]
mod tests {
    use super::{CommentDirectiveKind, collect_comment_directives};
    use crate::parse_text;

    #[test]
    fn collects_analysis_and_fmt_directives_from_comments() {
        let source = r#"
// rhai: extern injected_value: int
// rhai: allow unresolved unknown_name
// rhai: allow unresolved-import env
/* rhai: module env */
// rhai-fmt: skip
let value = 1;
"#;
        let parse = parse_text(source);
        let directives = collect_comment_directives(&parse.root(), source);
        assert!(directives.iter().any(|directive| matches!(
            &directive.kind,
            CommentDirectiveKind::Extern { name, ty }
                if name == "injected_value" && ty.as_deref() == Some("int")
        )));
        assert!(directives.iter().any(|directive| matches!(
            &directive.kind,
            CommentDirectiveKind::AllowUnresolved { name } if name == "unknown_name"
        )));
        assert!(directives.iter().any(|directive| matches!(
            &directive.kind,
            CommentDirectiveKind::AllowUnresolvedImport { name } if name == "env"
        )));
        assert!(directives.iter().any(|directive| matches!(
            &directive.kind,
            CommentDirectiveKind::Module { name } if name == "env"
        )));
        assert!(
            directives
                .iter()
                .any(|directive| matches!(directive.kind, CommentDirectiveKind::FormatSkip))
        );
    }
}
