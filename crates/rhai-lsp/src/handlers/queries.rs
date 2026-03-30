use anyhow::{Result, anyhow};
use lsp_types::{
    FoldingRange, FoldingRangeKind, Position, Range, SemanticToken, SemanticTokenModifier,
    SemanticTokenType, SemanticTokens, SemanticTokensLegend, TextEdit, Uri,
};
use rhai_ide::{
    CallHierarchyItem, CompletionItem, DocumentHighlight, FilePosition,
    FoldingRange as IdeFoldingRange, IncomingCall, InlayHint, OutgoingCall,
    SemanticToken as IdeSemanticToken, SemanticTokenKind,
    SemanticTokenModifier as IdeSemanticTokenModifier, SignatureHelp, SourceChange,
};
use rhai_vfs::FileId;

use crate::server::{Server, WorkspaceSymbolMatch, uri_from_path};

impl Server {
    pub fn completions(&self, uri: &Uri, offset: u32) -> Result<Vec<CompletionItem>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.completions(FilePosition { file_id, offset }))
    }

    pub fn document_highlights(&self, uri: &Uri, offset: u32) -> Result<Vec<DocumentHighlight>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.document_highlights(FilePosition { file_id, offset }))
    }

    pub fn prepare_call_hierarchy(&self, uri: &Uri, offset: u32) -> Result<Vec<CallHierarchyItem>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.prepare_call_hierarchy(FilePosition { file_id, offset }))
    }

    pub fn incoming_calls(&self, item: &CallHierarchyItem) -> Result<Vec<IncomingCall>> {
        Ok(self.analysis_host.snapshot().incoming_calls(item))
    }

    pub fn outgoing_calls(&self, item: &CallHierarchyItem) -> Result<Vec<OutgoingCall>> {
        Ok(self.analysis_host.snapshot().outgoing_calls(item))
    }

    pub fn resolve_completion(&self, item: CompletionItem) -> CompletionItem {
        self.analysis_host.snapshot().resolve_completion(item)
    }

    pub fn signature_help(&self, uri: &Uri, offset: u32) -> Result<Option<SignatureHelp>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.signature_help(FilePosition { file_id, offset }))
    }

    pub fn inlay_hints(&self, uri: &Uri) -> Result<Vec<InlayHint>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.inlay_hints(file_id))
    }

    pub fn format_document(&self, uri: &Uri) -> Result<Option<Vec<TextEdit>>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let Some(change) = analysis.format_document(file_id) else {
            return Ok(None);
        };

        Ok(source_change_to_lsp_text_edits(text.as_ref(), change))
    }

    pub fn format_range(&self, uri: &Uri, range: Range) -> Result<Option<Vec<TextEdit>>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let line_starts = line_start_offsets(text.as_ref());
        let start = position_to_offset(text.as_ref(), &line_starts, range.start)
            .ok_or_else(|| anyhow!("range start is outside document `{}`", uri.as_str()))?;
        let end = position_to_offset(text.as_ref(), &line_starts, range.end)
            .ok_or_else(|| anyhow!("range end is outside document `{}`", uri.as_str()))?;
        let rhai_range = rhai_syntax::TextRange::new((start as u32).into(), (end as u32).into());

        let Some(change) = analysis.format_range(file_id, rhai_range) else {
            return Ok(None);
        };

        Ok(source_change_to_lsp_text_edits(text.as_ref(), change))
    }

    pub fn folding_ranges(&self, uri: &Uri) -> Result<Vec<FoldingRange>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;

        Ok(encode_folding_ranges(
            text.as_ref(),
            &analysis.folding_ranges(file_id),
        ))
    }

    pub fn semantic_tokens(&self, uri: &Uri) -> Result<SemanticTokens> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;

        Ok(SemanticTokens {
            result_id: None,
            data: encode_semantic_tokens(text.as_ref(), &analysis.semantic_tokens(file_id)),
        })
    }

    pub fn workspace_symbols(&self, query: &str) -> Result<Vec<WorkspaceSymbolMatch>> {
        let analysis = self.analysis_host.snapshot();
        let symbols = if query.is_empty() {
            analysis.workspace_symbols()
        } else {
            analysis.workspace_symbols_matching(query)
        };

        symbols
            .into_iter()
            .map(|symbol| {
                let path = analysis.normalized_path(symbol.file_id).ok_or_else(|| {
                    anyhow!(
                        "workspace symbol `{}` is missing a normalized path",
                        symbol.name
                    )
                })?;

                Ok(WorkspaceSymbolMatch {
                    uri: uri_from_path(path)?,
                    symbol,
                })
            })
            .collect()
    }

    fn analysis_for_open_document(&self, uri: &Uri) -> Result<(rhai_ide::Analysis, FileId)> {
        let uri_text = uri.as_str();
        let document = self
            .open_documents
            .get(uri)
            .ok_or_else(|| anyhow!("document `{uri_text}` is not open"))?;
        let analysis = self.analysis_host.snapshot();
        let file_id = analysis
            .file_id_for_path(&document.normalized_path)
            .ok_or_else(|| anyhow!("document `{uri_text}` is not loaded in the analysis host"))?;

        Ok((analysis, file_id))
    }
}

pub(crate) fn semantic_token_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::COMMENT,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::TYPE,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::DEFAULT_LIBRARY,
        ],
    }
}

fn encode_semantic_tokens(text: &str, tokens: &[IdeSemanticToken]) -> Vec<SemanticToken> {
    let line_starts = line_start_offsets(text);
    let mut absolute_tokens = tokens.to_vec();
    absolute_tokens
        .sort_by_key(|token| (u32::from(token.range.start()), u32::from(token.range.end())));

    let mut encoded = Vec::<SemanticToken>::new();
    let mut previous_line = 0_u32;
    let mut previous_start = 0_u32;
    let mut has_previous = false;

    for token in absolute_tokens {
        for (line, start, length) in split_token_segments(text, &line_starts, &token) {
            let delta_line = if has_previous {
                line.saturating_sub(previous_line)
            } else {
                line
            };
            let delta_start = if has_previous && delta_line == 0 {
                start.saturating_sub(previous_start)
            } else {
                start
            };

            encoded.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type: semantic_token_type_index(token.kind),
                token_modifiers_bitset: semantic_token_modifier_bitset(&token.modifiers),
            });
            previous_line = line;
            previous_start = start;
            has_previous = true;
        }
    }

    encoded
}

fn split_token_segments(
    text: &str,
    line_starts: &[usize],
    token: &IdeSemanticToken,
) -> Vec<(u32, u32, u32)> {
    let start = u32::from(token.range.start()) as usize;
    let end = u32::from(token.range.end()) as usize;
    let mut segments = Vec::new();
    let mut segment_start = start;

    while segment_start < end {
        let line_index = line_index(line_starts, segment_start);
        let line_start = line_starts[line_index];
        let next_line_start = line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(text.len());
        let line_end = if line_index + 1 < line_starts.len() {
            next_line_start.saturating_sub(1)
        } else {
            text.len()
        };
        let segment_end = end.min(line_end);

        if segment_end > segment_start {
            segments.push((
                line_index as u32,
                utf16_len(&text[line_start..segment_start]) as u32,
                utf16_len(&text[segment_start..segment_end]) as u32,
            ));
        }

        if end <= line_end {
            break;
        }
        segment_start = next_line_start;
    }

    segments
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

fn source_change_to_lsp_text_edits(text: &str, change: SourceChange) -> Option<Vec<TextEdit>> {
    let [file_edit] = change.file_edits.as_slice() else {
        return None;
    };

    let line_starts = line_start_offsets(text);
    file_edit
        .edits
        .iter()
        .map(|edit| {
            Some(TextEdit {
                range: Range {
                    start: offset_to_position(
                        text,
                        &line_starts,
                        u32::from(edit.range.start()) as usize,
                    )?,
                    end: offset_to_position(
                        text,
                        &line_starts,
                        u32::from(edit.range.end()) as usize,
                    )?,
                },
                new_text: edit.new_text.clone(),
            })
        })
        .collect()
}

fn offset_to_position(text: &str, line_starts: &[usize], offset: usize) -> Option<Position> {
    if offset > text.len() {
        return None;
    }
    let line_index = line_index(line_starts, offset);
    let line_start = *line_starts.get(line_index)?;

    Some(Position {
        line: line_index as u32,
        character: utf16_len(&text[line_start..offset]) as u32,
    })
}

fn position_to_offset(text: &str, line_starts: &[usize], position: Position) -> Option<usize> {
    let line_start = *line_starts.get(position.line as usize)?;
    let line_end = line_starts
        .get(position.line as usize + 1)
        .copied()
        .unwrap_or(text.len());
    let line_text = &text[line_start..line_end];

    let mut utf16_units = 0_u32;
    for (byte_offset, ch) in line_text.char_indices() {
        if utf16_units == position.character {
            return Some(line_start + byte_offset);
        }
        utf16_units += ch.len_utf16() as u32;
        if utf16_units > position.character {
            return None;
        }
    }

    (utf16_units == position.character).then_some(line_start + line_text.len())
}

fn encode_folding_ranges(text: &str, ranges: &[IdeFoldingRange]) -> Vec<FoldingRange> {
    let line_starts = line_start_offsets(text);

    ranges
        .iter()
        .filter_map(|range| encode_folding_range(text, &line_starts, range))
        .collect()
}

fn encode_folding_range(
    text: &str,
    line_starts: &[usize],
    range: &IdeFoldingRange,
) -> Option<FoldingRange> {
    let start = u32::from(range.range.start()) as usize;
    let end = u32::from(range.range.end()) as usize;
    if start >= end {
        return None;
    }

    let start_line = line_index(line_starts, start) as u32;
    let end_offset = end.saturating_sub(1);
    let end_line = line_index(line_starts, end_offset) as u32;
    if start_line >= end_line {
        return None;
    }

    let end_line_start = *line_starts.get(end_line as usize)?;
    let end_character = utf16_len(&text[end_line_start..end_offset + 1]) as u32;

    Some(FoldingRange {
        start_line,
        start_character: Some(0),
        end_line,
        end_character: Some(end_character),
        kind: Some(match range.kind {
            rhai_ide::FoldingRangeKind::Comment => FoldingRangeKind::Comment,
            rhai_ide::FoldingRangeKind::Region => FoldingRangeKind::Region,
        }),
        collapsed_text: None,
    })
}

fn semantic_token_type_index(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::Keyword => 0,
        SemanticTokenKind::Comment => 1,
        SemanticTokenKind::String => 2,
        SemanticTokenKind::Number => 3,
        SemanticTokenKind::Type => 4,
        SemanticTokenKind::Function => 5,
        SemanticTokenKind::Method => 6,
        SemanticTokenKind::Parameter => 7,
        SemanticTokenKind::Variable => 8,
        SemanticTokenKind::Property => 9,
        SemanticTokenKind::Namespace => 10,
        SemanticTokenKind::Operator => 11,
    }
}

fn semantic_token_modifier_bitset(modifiers: &[IdeSemanticTokenModifier]) -> u32 {
    modifiers.iter().fold(0_u32, |bitset, modifier| {
        bitset
            | match modifier {
                IdeSemanticTokenModifier::Declaration => 1 << 0,
                IdeSemanticTokenModifier::Readonly => 1 << 1,
                IdeSemanticTokenModifier::DefaultLibrary => 1 << 2,
            }
    })
}
