use anyhow::{Result, anyhow};
use lsp_types::{
    FoldingRange, FoldingRangeKind, FormattingOptions, LinkedEditingRanges, Position, Range,
    SelectionRange, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend, TextEdit, Uri,
};
use rhai_fmt::{
    FormatOptions as RhaiFormatOptions, IndentStyle, apply_partial_format_options,
    load_format_config_for_path,
};
use rhai_ide::{
    CallHierarchyItem, CompletionItem, DocumentHighlight, DocumentSymbol, FilePosition,
    FoldingRange as IdeFoldingRange, HoverResult, IncomingCall, InlayHint, InlayHintSource,
    NavigationTarget, OutgoingCall, PreparedRename, ReferencesResult,
    SemanticToken as IdeSemanticToken, SemanticTokenKind,
    SemanticTokenModifier as IdeSemanticTokenModifier, SignatureHelp, SourceChange,
    WorkspaceSymbol,
};
use rhai_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TextSize, TokenKind, parse_text};
use rhai_vfs::FileId;

use crate::protocol::text_range_to_lsp_range;
use crate::state::{ServerState, WorkspaceSymbolMatch};

impl ServerState {
    pub fn hover(&self, uri: &Uri, offset: u32) -> Result<Option<HoverResult>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.hover(FilePosition { file_id, offset }))
    }

    pub fn goto_definition(&self, uri: &Uri, offset: u32) -> Result<Vec<NavigationTarget>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.goto_definition(FilePosition { file_id, offset }))
    }

    pub fn goto_type_definition(&self, uri: &Uri, offset: u32) -> Result<Vec<NavigationTarget>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.goto_type_definition(FilePosition { file_id, offset }))
    }

    pub fn goto_declaration(&self, uri: &Uri, offset: u32) -> Result<Vec<NavigationTarget>> {
        self.goto_definition(uri, offset)
    }

    pub fn find_references(&self, uri: &Uri, offset: u32) -> Result<Option<ReferencesResult>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.find_references(FilePosition { file_id, offset }))
    }

    pub fn rename(
        &self,
        uri: &Uri,
        offset: u32,
        new_name: String,
    ) -> Result<Option<PreparedRename>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.rename(FilePosition { file_id, offset }, new_name))
    }

    pub fn prepare_rename(&self, uri: &Uri, offset: u32) -> Result<Option<PreparedRename>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.rename(FilePosition { file_id, offset }, String::new()))
    }

    pub fn linked_editing_ranges(
        &self,
        uri: &Uri,
        offset: u32,
    ) -> Result<Option<LinkedEditingRanges>> {
        let prepared = self.prepare_rename(uri, offset)?;
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let text = crate::protocol::open_document_text_by_uri(self, uri)
            .ok_or_else(|| anyhow!("document `{}` is not open", uri.as_str()))?;
        let offset = TextSize::from(offset);
        let active_range = rename_focus_range(&prepared, offset);
        let Some(active_range) = active_range else {
            return Ok(None);
        };
        let Some(active_text) = text_slice(text.as_ref(), active_range) else {
            return Ok(None);
        };
        if !is_identifier_like(active_text) {
            return Ok(None);
        }

        let (_, file_id) = self.analysis_for_open_document(uri)?;
        let mut ranges = prepared
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
            .filter(|range| {
                range_contains_offset(*range, offset) || {
                    prepared.plan.occurrences.iter().any(|occurrence| {
                        occurrence.file_id == file_id && occurrence.range == *range
                    }) || prepared
                        .plan
                        .targets
                        .iter()
                        .any(|target| target.file_id == file_id && target.focus_range == *range)
                }
            })
            .filter_map(|range| {
                let current = text_slice(text.as_ref(), range)?;
                (current == active_text && is_identifier_like(current)).then_some(range)
            })
            .collect::<Vec<_>>();
        ranges.sort_by_key(|range| (u32::from(range.start()), u32::from(range.end())));
        ranges.dedup();

        if ranges.len() < 2 {
            return Ok(None);
        }

        let lsp_ranges = ranges
            .into_iter()
            .filter_map(|range| text_range_to_lsp_range(text.as_ref(), range))
            .collect::<Vec<_>>();

        Ok(Some(LinkedEditingRanges {
            ranges: lsp_ranges,
            word_pattern: Some(String::from(r"[A-Za-z_][A-Za-z0-9_]*")),
        }))
    }

    pub fn selection_ranges(
        &self,
        uri: &Uri,
        positions: &[Position],
    ) -> Result<Vec<SelectionRange>> {
        let text = crate::protocol::open_document_text_by_uri(self, uri)
            .ok_or_else(|| anyhow!("document `{}` is not open", uri.as_str()))?;
        let parse = parse_text(text.as_ref());
        let root = parse.root();

        positions
            .iter()
            .map(|position| {
                let offset = position_to_offset_in_text(text.as_ref(), *position)
                    .ok_or_else(|| anyhow!("position is outside document `{}`", uri.as_str()))?;
                selection_range_chain_to_lsp(text.as_ref(), &root, TextSize::from(offset as u32))
                    .ok_or_else(|| {
                        anyhow!("unable to build selection range for `{}`", uri.as_str())
                    })
            })
            .collect()
    }

    pub fn document_symbols(&self, uri: &Uri) -> Result<Vec<DocumentSymbol>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        Ok(analysis.document_symbols(file_id))
    }

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

    pub fn inlay_hints(&self, uri: &Uri, range: Option<Range>) -> Result<Vec<InlayHint>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let requested_range = range
            .map(|range| lsp_range_to_text_range(text.as_ref(), range))
            .transpose()?;

        Ok(analysis
            .inlay_hints(file_id)
            .into_iter()
            .filter(|hint| self.inlay_hint_source_enabled(hint.source))
            .filter(|hint| {
                requested_range.is_none_or(|range| range.contains(TextSize::from(hint.offset)))
            })
            .collect())
    }

    pub fn format_document(
        &self,
        uri: &Uri,
        options: FormattingOptions,
    ) -> Result<Option<Vec<TextEdit>>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let format_options = self.formatter_options_for_uri(uri, &options);
        let Some(change) = analysis.format_document_with_options(file_id, &format_options) else {
            return Ok(None);
        };

        Ok(source_change_to_lsp_text_edits(text.as_ref(), change))
    }

    pub fn format_range(
        &self,
        uri: &Uri,
        range: Range,
        options: FormattingOptions,
    ) -> Result<Option<Vec<TextEdit>>> {
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
        let format_options = self.formatter_options_for_uri(uri, &options);

        let Some(change) = analysis.format_range_with_options(file_id, rhai_range, &format_options)
        else {
            return Ok(None);
        };

        Ok(source_change_to_lsp_text_edits(text.as_ref(), change))
    }

    pub fn format_on_type(
        &self,
        uri: &Uri,
        position: Position,
        ch: &str,
        options: FormattingOptions,
    ) -> Result<Option<Vec<TextEdit>>> {
        if !matches!(ch, ";" | "}") {
            return Ok(None);
        }

        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let offset = position_to_offset_in_text(text.as_ref(), position)
            .ok_or_else(|| anyhow!("position is outside document `{}`", uri.as_str()))?;
        let trigger_offset = offset.saturating_sub(ch.len());
        let trigger_range = TextRange::new(
            TextSize::from(trigger_offset as u32),
            TextSize::from(trigger_offset as u32),
        );
        let format_options = self.formatter_options_for_uri(uri, &options);

        let change =
            match analysis.format_range_with_options(file_id, trigger_range, &format_options) {
                Some(change) => change,
                None if ch == "}" => {
                    let Some(change) =
                        analysis.format_document_with_options(file_id, &format_options)
                    else {
                        return Ok(None);
                    };
                    return Ok(source_change_to_lsp_text_edits(text.as_ref(), change));
                }
                None => return Ok(None),
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

    pub fn semantic_tokens(&self, uri: &Uri, range: Option<Range>) -> Result<SemanticTokens> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let requested_range = range
            .map(|range| lsp_range_to_text_range(text.as_ref(), range))
            .transpose()?;
        let tokens = analysis.semantic_tokens(file_id);
        let filtered_tokens = match requested_range {
            Some(range) => tokens
                .into_iter()
                .filter(|token| text_ranges_intersect(token.range, range))
                .collect::<Vec<_>>(),
            None => tokens,
        };

        Ok(SemanticTokens {
            result_id: None,
            data: encode_semantic_tokens(text.as_ref(), &filtered_tokens),
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
                    uri: self.uri_for_path(path)?,
                    symbol,
                })
            })
            .collect()
    }

    pub fn workspace_symbols_raw(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let analysis = self.analysis_host.snapshot();
        if query.is_empty() {
            analysis.workspace_symbols()
        } else {
            analysis.workspace_symbols_matching(query)
        }
    }

    pub(crate) fn analysis_for_open_document(
        &self,
        uri: &Uri,
    ) -> Result<(rhai_ide::Analysis, FileId)> {
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

pub(crate) fn lsp_range_to_text_range(text: &str, range: Range) -> Result<TextRange> {
    let line_starts = line_start_offsets(text);
    let start = position_to_offset(text, &line_starts, range.start)
        .ok_or_else(|| anyhow!("range start is outside the current document"))?;
    let end = position_to_offset(text, &line_starts, range.end)
        .ok_or_else(|| anyhow!("range end is outside the current document"))?;
    Ok(TextRange::new(
        TextSize::from(start as u32),
        TextSize::from(end as u32),
    ))
}

fn text_ranges_intersect(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn selection_range_chain_to_lsp(
    text: &str,
    root: &SyntaxNode,
    offset: TextSize,
) -> Option<SelectionRange> {
    let leaf_token = smallest_token_covering_offset(root, offset);
    let mut ranges = Vec::<TextRange>::new();

    if let Some(token) = leaf_token.as_ref().filter(|token| {
        token
            .kind()
            .token_kind()
            .is_some_and(|kind| !kind.is_trivia())
    }) {
        push_unique_range(&mut ranges, token.text_range());
    }

    let starting_node = leaf_token
        .as_ref()
        .and_then(|token| token.parent())
        .unwrap_or_else(|| root.clone());

    for node in starting_node.ancestors() {
        let range = selection_range_for_node(&node, offset);
        push_unique_range(&mut ranges, range);
    }

    let mut selection = None;
    for range in ranges.into_iter().rev() {
        selection = Some(SelectionRange {
            range: text_range_to_lsp_range(text, range)?,
            parent: selection.map(Box::new),
        });
    }

    selection
}

fn smallest_token_covering_offset(
    root: &SyntaxNode,
    offset: TextSize,
) -> Option<rhai_syntax::SyntaxToken> {
    root.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| range_contains_offset(token.text_range(), offset))
        .min_by_key(|token| {
            let range = token.text_range();
            let width = u32::from(range.end()) - u32::from(range.start());
            let trivia_bias = token.kind().token_kind().is_some_and(TokenKind::is_trivia) as u32;
            (trivia_bias, width)
        })
}

fn selection_range_for_node(node: &SyntaxNode, offset: TextSize) -> TextRange {
    let structural = node.structural_range();
    if range_contains_offset(structural, offset) {
        structural
    } else {
        node.text_range()
    }
}

fn push_unique_range(ranges: &mut Vec<TextRange>, range: TextRange) {
    if ranges.last().copied() != Some(range) {
        ranges.push(range);
    }
}

fn range_contains_offset(range: TextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn rename_focus_range(prepared: &PreparedRename, offset: TextSize) -> Option<TextRange> {
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
        .find(|range| range_contains_offset(*range, offset))
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

fn text_slice(text: &str, range: TextRange) -> Option<&str> {
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    text.get(start..end)
}

fn is_identifier_like(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn position_to_offset_in_text(text: &str, position: Position) -> Option<usize> {
    let line_starts = line_start_offsets(text);
    let line_start = *line_starts.get(position.line as usize)?;
    let line_end = line_starts
        .get(position.line as usize + 1)
        .copied()
        .unwrap_or(text.len());
    let line_text = text.get(line_start..line_end)?;

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

impl ServerState {
    fn inlay_hint_source_enabled(&self, source: InlayHintSource) -> bool {
        match source {
            InlayHintSource::Variable => self.settings.inlay_hints.variables,
            InlayHintSource::Parameter => self.settings.inlay_hints.parameters,
            InlayHintSource::ReturnType => self.settings.inlay_hints.return_types,
        }
    }

    fn formatter_options_for_uri(
        &self,
        uri: &Uri,
        options: &FormattingOptions,
    ) -> RhaiFormatOptions {
        let mut format_options = RhaiFormatOptions::default();
        if let Some(document) = self.open_documents.get(uri)
            && let Ok(Some(config)) = load_format_config_for_path(&document.normalized_path)
        {
            apply_partial_format_options(&mut format_options, &config.options);
        }

        format_options.max_line_length = self.settings.formatter.max_line_length;
        format_options.trailing_commas = self.settings.formatter.trailing_commas;
        format_options.final_newline = self.settings.formatter.final_newline;
        format_options.container_layout = self.settings.formatter.container_layout;
        format_options.import_sort_order = self.settings.formatter.import_sort_order;
        format_options.indent_style = if options.insert_spaces {
            IndentStyle::Spaces
        } else {
            IndentStyle::Tabs
        };
        format_options.indent_width = options.tab_size as usize;
        format_options
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
            let Some(prefix_text) = text.get(line_start..segment_start) else {
                break;
            };
            let Some(segment_text) = text.get(segment_start..segment_end) else {
                break;
            };
            segments.push((
                line_index as u32,
                utf16_len(prefix_text) as u32,
                utf16_len(segment_text) as u32,
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
        character: utf16_len(text.get(line_start..offset)?) as u32,
    })
}

fn position_to_offset(text: &str, line_starts: &[usize], position: Position) -> Option<usize> {
    let line_start = *line_starts.get(position.line as usize)?;
    let line_end = line_starts
        .get(position.line as usize + 1)
        .copied()
        .unwrap_or(text.len());
    let line_text = text.get(line_start..line_end)?;

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
    let end_character = utf16_len(text.get(end_line_start..end_offset + 1)?) as u32;

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
