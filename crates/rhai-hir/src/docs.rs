use rhai_syntax::{SyntaxToken, TextRange, TextSize, TokenKind};

use crate::ty::{TypeRef, parse_type_ref};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocBlockId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocBlock {
    pub range: TextRange,
    pub text: String,
    pub lines: Vec<String>,
    pub tags: Vec<DocTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocTag {
    Type(TypeRef),
    Param { name: String, ty: TypeRef },
    Return(TypeRef),
    Field { name: String, ty: TypeRef },
    Unknown { name: String, value: String },
}

pub fn collect_doc_block(
    tokens: &[SyntaxToken],
    source: &str,
    item_start: TextSize,
) -> Option<DocBlock> {
    let pivot = tokens.partition_point(|token| token.range().end() <= item_start);
    if pivot == 0 {
        return None;
    }

    let mut doc_tokens = Vec::new();
    let mut cursor = pivot;
    while cursor > 0 {
        cursor -= 1;
        let token = tokens[cursor];

        match token.kind() {
            TokenKind::DocLineComment | TokenKind::DocBlockComment => {
                doc_tokens.push(token);
            }
            TokenKind::Whitespace => {
                if token.text(source).matches('\n').count() > 1 {
                    break;
                }
            }
            _ if doc_tokens.is_empty() => continue,
            _ => break,
        }
    }

    if doc_tokens.is_empty() {
        return None;
    }

    doc_tokens.reverse();
    let start = doc_tokens.first()?.range().start();
    let end = doc_tokens.last()?.range().end();
    let range = TextRange::new(start, end);

    let lines: Vec<_> = doc_tokens
        .iter()
        .flat_map(|token| normalize_doc_comment(token, source))
        .collect();
    let text = lines.join("\n");
    let tags = lines
        .iter()
        .filter_map(|line| parse_doc_tag(line))
        .collect();

    Some(DocBlock {
        range,
        text,
        lines,
        tags,
    })
}

fn normalize_doc_comment(token: &SyntaxToken, source: &str) -> Vec<String> {
    let text = token.text(source);
    match token.kind() {
        TokenKind::DocLineComment => vec![
            text.trim_start_matches("///")
                .trim_start_matches("//!")
                .trim()
                .to_owned(),
        ],
        TokenKind::DocBlockComment => {
            let inner = text
                .trim_start_matches("/**")
                .trim_start_matches("/*!")
                .trim_end_matches("*/");

            inner
                .lines()
                .map(|line| line.trim().trim_start_matches('*').trim().to_owned())
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parse_doc_tag(line: &str) -> Option<DocTag> {
    let line = line.trim();
    let tag = line.strip_prefix('@')?;

    if let Some(rest) = tag.strip_prefix("type ") {
        return parse_type_ref(rest.trim()).map(DocTag::Type);
    }

    if let Some(rest) = tag.strip_prefix("param ") {
        let (name, ty) = split_name_and_type(rest)?;
        return parse_type_ref(ty).map(|ty| DocTag::Param {
            name: name.to_owned(),
            ty,
        });
    }

    if let Some(rest) = tag.strip_prefix("return ") {
        return parse_type_ref(rest.trim()).map(DocTag::Return);
    }

    if let Some(rest) = tag.strip_prefix("field ") {
        let (name, ty) = split_name_and_type(rest)?;
        return parse_type_ref(ty).map(|ty| DocTag::Field {
            name: name.to_owned(),
            ty,
        });
    }

    let mut parts = tag.splitn(2, char::is_whitespace);
    let name = parts.next()?.to_owned();
    let value = parts.next().unwrap_or_default().trim().to_owned();
    Some(DocTag::Unknown { name, value })
}

fn split_name_and_type(input: &str) -> Option<(&str, &str)> {
    let input = input.trim();
    let split = input.find(char::is_whitespace)?;
    let (name, rest) = input.split_at(split);
    Some((name.trim(), rest.trim()))
}

#[cfg(test)]
mod tests {
    use rhai_syntax::{TextSize, lex_text};

    use crate::ty::TypeRef;

    use super::{DocTag, collect_doc_block};

    #[test]
    fn collects_leading_doc_block_and_tags() {
        let source = r#"
/// Adds values.
/// @param x int
/// @param y int
/// @return int
fn add(x, y) { x + y }
"#;
        let lexed = lex_text(source);
        let fn_offset = TextSize::from(u32::try_from(source.find("fn").unwrap()).unwrap());
        let docs = collect_doc_block(lexed.tokens(), source, fn_offset).expect("expected docs");

        assert_eq!(docs.lines[0], "Adds values.");
        assert_eq!(
            docs.tags,
            vec![
                DocTag::Param {
                    name: "x".to_owned(),
                    ty: TypeRef::Int,
                },
                DocTag::Param {
                    name: "y".to_owned(),
                    ty: TypeRef::Int,
                },
                DocTag::Return(TypeRef::Int),
            ]
        );
    }

    #[test]
    fn stops_doc_attachment_at_blank_lines() {
        let source = "/// first\n\nfn value() {}";
        let lexed = lex_text(source);
        let fn_offset = TextSize::from(u32::try_from(source.find("fn").unwrap()).unwrap());

        assert!(collect_doc_block(lexed.tokens(), source, fn_offset).is_none());
    }
}
