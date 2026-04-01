use lsp_types::{self, Position, Range};
use rhai_ide::PreparedRename;
use rhai_syntax::{TextRange, TextSize};

use crate::state::{ServerState, path_from_uri};

pub(crate) fn open_document_text_by_uri(
    server: &ServerState,
    uri: &lsp_types::Uri,
) -> Option<std::sync::Arc<str>> {
    server
        .open_documents
        .get(uri)
        .map(|document| std::sync::Arc::<str>::from(document.text.as_str()))
}

pub(crate) fn file_text_by_uri(
    server: &ServerState,
    uri: &lsp_types::Uri,
) -> Option<std::sync::Arc<str>> {
    let normalized_path = path_from_uri(uri).ok()?;
    let snapshot = server.analysis_host().snapshot();
    let file_id = snapshot.file_id_for_path(&normalized_path)?;
    snapshot.file_text(file_id)
}

pub(crate) fn text_range_to_lsp_range(text: &str, range: TextRange) -> Option<Range> {
    Some(Range {
        start: offset_to_position(text, u32::from(range.start()) as usize)?,
        end: offset_to_position(text, u32::from(range.end()) as usize)?,
    })
}

pub(crate) fn prepare_rename_range(prepared: &PreparedRename, offset: u32) -> Option<TextRange> {
    let offset = TextSize::from(offset);

    prepared
        .plan
        .targets
        .iter()
        .map(|target| target.focus_range)
        .chain(
            prepared
                .plan
                .occurrences
                .iter()
                .map(|occurrence| occurrence.range),
        )
        .find(|range| range.contains(offset))
        .or_else(|| {
            prepared
                .plan
                .targets
                .first()
                .map(|target| target.focus_range)
        })
        .or_else(|| {
            prepared
                .plan
                .occurrences
                .first()
                .map(|occurrence| occurrence.range)
        })
}

pub(crate) fn text_slice(text: &str, range: TextRange) -> Option<&str> {
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    text.get(start..end)
}

pub(crate) fn rename_placeholder(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let Some(last) = text.chars().last() else {
        return String::new();
    };

    if matches!(first, '"' | '\'' | '`') && first == last && text.len() >= first.len_utf8() * 2 {
        text.strip_prefix(first)
            .and_then(|text| text.strip_suffix(last))
            .unwrap_or(text)
            .to_owned()
    } else {
        text.to_owned()
    }
}

fn offset_to_position(text: &str, offset: usize) -> Option<Position> {
    if offset > text.len() {
        return None;
    }

    let line_starts = line_start_offsets(text);
    let line_index = line_index(&line_starts, offset);
    let line_start = *line_starts.get(line_index)?;

    Some(Position {
        line: line_index as u32,
        character: utf16_len(text.get(line_start..offset)?) as u32,
    })
}

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0];

    for (offset, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(offset + ch.len_utf8());
        }
    }

    starts
}

fn line_index(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}
