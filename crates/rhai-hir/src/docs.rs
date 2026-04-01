use rhai_syntax::{SyntaxNode, TextRange, TextSize, TokenKind};

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
    Param {
        name: String,
        ty: TypeRef,
    },
    Return(TypeRef),
    Field {
        name: String,
        ty: TypeRef,
        docs: Option<String>,
    },
    Unknown {
        name: String,
        value: String,
    },
}

pub fn collect_doc_block(root: &SyntaxNode, item_start: TextSize) -> Option<DocBlock> {
    let tokens: Vec<_> = root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter_map(|token| {
            token
                .kind()
                .token_kind()
                .map(|kind| (kind, token.text_range(), token.text().to_string()))
        })
        .collect();
    let pivot = tokens.partition_point(|(_, range, _)| range.end() <= item_start);
    if pivot == 0 {
        return None;
    }

    let mut doc_tokens = Vec::new();
    let mut cursor = pivot;
    while cursor > 0 {
        cursor -= 1;
        let token = &tokens[cursor];

        match token.0 {
            TokenKind::DocLineComment | TokenKind::DocBlockComment => {
                doc_tokens.push(token);
            }
            TokenKind::Whitespace => {
                if token.2.matches('\n').count() > 1 {
                    break;
                }
            }
            _ if doc_tokens.is_empty() => return None,
            _ => break,
        }
    }

    if doc_tokens.is_empty() {
        return None;
    }

    doc_tokens.reverse();
    let start = doc_tokens.first()?.1.start();
    let end = doc_tokens.last()?.1.end();
    let range = TextRange::new(start, end);

    let lines: Vec<_> = doc_tokens.iter().flat_map(normalize_doc_comment).collect();
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

fn normalize_doc_comment(token: &&(TokenKind, TextRange, String)) -> Vec<String> {
    let text = token.2.as_str();
    match token.0 {
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
        let (name, ty, docs) = split_name_type_and_optional_docs(rest)?;
        return Some(DocTag::Field {
            name: name.to_owned(),
            ty,
            docs,
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

fn split_name_type_and_optional_docs(input: &str) -> Option<(&str, TypeRef, Option<String>)> {
    let input = input.trim();
    let split = input.find(char::is_whitespace)?;
    let (name, rest) = input.split_at(split);
    let name = name.trim();
    let rest = rest.trim();
    let parts = rest.split_whitespace().collect::<Vec<_>>();
    for end in (1..=parts.len()).rev() {
        let ty_text = parts[..end].join(" ");
        let Some(ty) = parse_type_ref(ty_text.as_str()) else {
            continue;
        };
        let docs = (!parts[end..].is_empty()).then(|| parts[end..].join(" "));
        return Some((name, ty, docs));
    }
    None
}

#[cfg(test)]
mod tests {
    use rhai_syntax::{TextSize, parse_text};

    use crate::ty::TypeRef;

    use crate::docs::{DocTag, collect_doc_block};

    fn assert_valid_rhai_syntax(source: &str) {
        let parse = parse_text(source);
        assert!(
            parse.errors().is_empty(),
            "expected valid Rhai syntax, got errors: {:?}",
            parse.errors()
        );
    }

    #[test]
    fn collects_leading_doc_block_and_tags() {
        let source = r#"
/// Adds values.
/// @param x int
/// @param y int
/// @return int
fn add(x, y) { x + y }
"#;
        assert_valid_rhai_syntax(source);
        let parse = parse_text(source);
        let fn_offset = TextSize::from(u32::try_from(source.find("fn").unwrap()).unwrap());
        let docs = collect_doc_block(&parse.root(), fn_offset).expect("expected docs");

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
    fn parses_field_tags_with_optional_docs() {
        let source = r#"
/// User docs.
/// @field name string Primary display name
let user = #{ name: "Ada" };
"#;
        assert_valid_rhai_syntax(source);
        let parse = parse_text(source);
        let let_offset = TextSize::from(u32::try_from(source.find("let").unwrap()).unwrap());
        let docs = collect_doc_block(&parse.root(), let_offset).expect("expected docs");

        assert_eq!(
            docs.tags,
            vec![DocTag::Field {
                name: "name".to_owned(),
                ty: TypeRef::String,
                docs: Some("Primary display name".to_owned()),
            }]
        );
    }

    #[test]
    fn stops_doc_attachment_at_blank_lines() {
        let source = "/// first\n\nfn value() {}";
        assert_valid_rhai_syntax(source);
        let parse = parse_text(source);
        let fn_offset = TextSize::from(u32::try_from(source.find("fn").unwrap()).unwrap());

        assert!(collect_doc_block(&parse.root(), fn_offset).is_none());
    }

    #[test]
    fn does_not_attach_docs_past_a_previous_statement() {
        let source = "/// @type int\nlet first = value;\nlet second = first;";
        assert_valid_rhai_syntax(source);
        let parse = parse_text(source);
        let second_offset =
            TextSize::from(u32::try_from(source.find("let second").unwrap()).unwrap());

        assert!(collect_doc_block(&parse.root(), second_offset).is_none());
    }
}
