use rhai_syntax::{SyntaxNode, TextRange, TokenKind};

pub(crate) fn contains_token(node: &SyntaxNode, kind: TokenKind) -> bool {
    node.children().iter().any(|child| {
        child.as_token().is_some_and(|token| token.kind() == kind)
            || child
                .as_node()
                .is_some_and(|child_node| contains_token(child_node, kind))
    })
}

pub(crate) fn minimal_changed_region<'a>(
    original: &'a str,
    formatted: &'a str,
) -> Option<(usize, usize, &'a str)> {
    if original == formatted {
        return None;
    }

    let prefix = common_prefix_len(original, formatted);
    let original_suffix = &original[prefix..];
    let formatted_suffix = &formatted[prefix..];
    let suffix = common_suffix_len(original_suffix, formatted_suffix);

    let original_end = original.len().saturating_sub(suffix);
    let formatted_end = formatted.len().saturating_sub(suffix);
    Some((prefix, original_end, &formatted[prefix..formatted_end]))
}

fn common_prefix_len(left: &str, right: &str) -> usize {
    let mut left_iter = left.char_indices();
    let mut right_iter = right.char_indices();
    let mut len = 0;

    loop {
        match (left_iter.next(), right_iter.next()) {
            (Some((left_index, left_char)), Some((right_index, right_char)))
                if left_char == right_char && left_index == right_index =>
            {
                len = left_index + left_char.len_utf8();
            }
            _ => break,
        }
    }

    len
}

fn common_suffix_len(left: &str, right: &str) -> usize {
    let mut left_iter = left.chars().rev();
    let mut right_iter = right.chars().rev();
    let mut len = 0;

    loop {
        match (left_iter.next(), right_iter.next()) {
            (Some(left_char), Some(right_char)) if left_char == right_char => {
                len += left_char.len_utf8();
            }
            _ => break,
        }
    }

    len.min(left.len()).min(right.len())
}

pub(crate) fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    let left_start = u32::from(left.start());
    let left_end = u32::from(left.end());
    let right_start = u32::from(right.start());
    let right_end = u32::from(right.end());

    left_start < right_end && right_start < left_end
}
