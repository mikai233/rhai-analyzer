use crate::FilePosition;
use crate::completion::{
    CompletionContext, DocCompletionContext, PostfixCompletionContext, text_range,
};
use rhai_db::DatabaseSnapshot;

pub(super) fn completion_context(
    snapshot: &DatabaseSnapshot,
    position: FilePosition,
) -> CompletionContext {
    let Some(text) = snapshot.file_text(position.file_id) else {
        return CompletionContext {
            prefix: String::new(),
            replace_range: text_range(0, 0),
            query_offset: 0,
            member_access: false,
            module_path: None,
            postfix_completion: None,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: false,
        };
    };
    let offset = clamp_to_char_boundary(
        text.as_ref(),
        usize::try_from(position.offset)
            .unwrap_or(usize::MAX)
            .min(text.len()),
    );
    let bytes = text.as_bytes();
    if bytes.get(offset).copied() == Some(b'.') {
        let postfix_completion = postfix_completion_context_at_dot(text.as_ref(), offset);
        return CompletionContext {
            prefix: String::new(),
            replace_range: text_range(offset + 1, offset + 1),
            query_offset: offset + 1,
            member_access: true,
            module_path: None,
            postfix_completion,
            suppress_completion: false,
            doc_completion: None,
            next_char_is_open_paren: bytes.get(offset + 1).copied() == Some(b'('),
        };
    }

    let mut start = offset;

    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let prefix = text[start..offset].to_owned();
    let replace_range = text_range(start, offset);
    let member_access = start > 0 && bytes[start - 1] == b'.';
    let module_path = module_path_before_offset(text.as_ref(), start);
    let postfix_completion = postfix_completion_context(text.as_ref(), start, offset);
    let suppress_completion = single_colon_path_context_before_offset(text.as_ref(), start);
    let doc_completion = doc_completion_context(text.as_ref(), offset);
    let next_char_is_open_paren = bytes.get(offset).copied() == Some(b'(');

    CompletionContext {
        prefix,
        replace_range,
        query_offset: offset,
        member_access,
        module_path,
        postfix_completion,
        suppress_completion,
        doc_completion,
        next_char_is_open_paren,
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn module_path_before_offset(text: &str, prefix_start: usize) -> Option<Vec<String>> {
    if !has_double_colon_before(text.as_bytes(), prefix_start) {
        return None;
    }

    let bytes = text.as_bytes();
    let mut end = prefix_start - 2;
    let mut parts = Vec::<String>::new();

    loop {
        let mut start = end;
        while start > 0 && is_identifier_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == end {
            return None;
        }
        parts.push(text.get(start..end)?.to_owned());
        if !has_double_colon_before(bytes, start) {
            break;
        }
        end = start - 2;
    }

    parts.reverse();
    Some(parts)
}

fn single_colon_path_context_before_offset(text: &str, prefix_start: usize) -> bool {
    let bytes = text.as_bytes();
    if prefix_start == 0 || bytes[prefix_start - 1] != b':' {
        return false;
    }
    if has_double_colon_before(bytes, prefix_start) {
        return false;
    }

    let mut segment_start = prefix_start - 1;
    while segment_start > 0 && is_identifier_byte(bytes[segment_start - 1]) {
        segment_start -= 1;
    }

    segment_start < prefix_start - 1
}

fn doc_completion_context(text: &str, offset: usize) -> Option<DocCompletionContext> {
    let offset = offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line = &text[line_start..offset];
    let trimmed = line.trim_start();
    let marker = if trimmed.starts_with("///") {
        "///"
    } else if trimmed.starts_with("//!") {
        "//!"
    } else {
        return None;
    };
    let leading_ws = line.len() - trimmed.len();
    let marker_start = line_start + leading_ws;
    let after_marker_start = marker_start + marker.len();
    let after_marker = &text[after_marker_start..offset];
    let content = after_marker.trim_start();
    let content_start = after_marker_start + (after_marker.len() - content.len());

    if let Some(tag) = content.strip_prefix('@')
        && !tag.contains(char::is_whitespace)
    {
        let prefix_start = content_start + 1;
        return Some(DocCompletionContext::Tag {
            prefix: tag.to_owned(),
            replace_range: text_range(prefix_start, offset),
        });
    }

    let parts = content.split_whitespace().collect::<Vec<_>>();
    let trailing_space = content.chars().last().is_some_and(char::is_whitespace);

    match parts.first().copied() {
        Some("@type") | Some("@return") => {
            let replace_start = if trailing_space {
                offset
            } else {
                offset.saturating_sub(parts.get(1).copied().unwrap_or_default().len())
            };
            let prefix = if trailing_space {
                String::new()
            } else {
                parts.get(1).copied().unwrap_or_default().to_owned()
            };
            Some(DocCompletionContext::Type {
                prefix,
                replace_range: text_range(replace_start, offset),
            })
        }
        Some("@param") | Some("@field") => {
            let replace_start = if trailing_space {
                match parts.len() {
                    0..=2 => return None,
                    _ => offset,
                }
            } else if parts.len() >= 3 {
                offset.saturating_sub(parts.last().copied().unwrap_or_default().len())
            } else {
                return None;
            };
            let prefix = if trailing_space {
                match parts.len() {
                    0..=2 => return None,
                    _ => String::new(),
                }
            } else if parts.len() >= 3 {
                parts.last().copied().unwrap_or_default().to_owned()
            } else {
                return None;
            };
            Some(DocCompletionContext::Type {
                prefix,
                replace_range: text_range(replace_start, offset),
            })
        }
        _ => None,
    }
}

fn postfix_completion_context(
    text: &str,
    prefix_start: usize,
    offset: usize,
) -> Option<PostfixCompletionContext> {
    if prefix_start == 0 || text.as_bytes().get(prefix_start - 1).copied() != Some(b'.') {
        return None;
    }

    let bytes = text.as_bytes();
    let mut receiver_start = prefix_start - 1;
    while receiver_start > 0 && is_identifier_byte(bytes[receiver_start - 1]) {
        receiver_start -= 1;
    }

    if receiver_start == prefix_start - 1 {
        return None;
    }

    Some(PostfixCompletionContext {
        receiver_text: text.get(receiver_start..prefix_start - 1)?.to_owned(),
        replace_range: text_range(receiver_start, offset),
    })
}

fn postfix_completion_context_at_dot(
    text: &str,
    dot_offset: usize,
) -> Option<PostfixCompletionContext> {
    if text.as_bytes().get(dot_offset).copied() != Some(b'.') || dot_offset == 0 {
        return None;
    }

    let bytes = text.as_bytes();
    let mut receiver_start = dot_offset;
    while receiver_start > 0 && is_identifier_byte(bytes[receiver_start - 1]) {
        receiver_start -= 1;
    }

    if receiver_start == dot_offset {
        return None;
    }

    Some(PostfixCompletionContext {
        receiver_text: text.get(receiver_start..dot_offset)?.to_owned(),
        replace_range: text_range(receiver_start, dot_offset + 1),
    })
}

fn clamp_to_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn has_double_colon_before(bytes: &[u8], offset: usize) -> bool {
    offset >= 2 && bytes[offset - 2] == b':' && bytes[offset - 1] == b':'
}
