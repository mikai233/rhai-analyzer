use anyhow::{Result, anyhow};
use lsp_types::{CodeActionKind, Diagnostic, Range, Uri};
use rhai_ide::{Assist, AssistKind, SourceChange};
use rhai_syntax::{TextRange, TextSize};

use crate::handlers::queries::lsp_range_to_text_range;
use crate::protocol::{CodeActionResolvePayload, diagnostic_to_lsp};
use crate::state::ServerState;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedCodeAction {
    pub id: String,
    pub title: String,
    pub kind: CodeActionKind,
    pub target: TextRange,
    pub diagnostics: Vec<Diagnostic>,
    pub is_preferred: bool,
    pub source_change: SourceChange,
}

impl ServerState {
    pub(crate) fn code_actions(
        &self,
        uri: &Uri,
        range: Range,
        diagnostics: &[Diagnostic],
        requested_kinds: Option<&[CodeActionKind]>,
    ) -> Result<Vec<ResolvedCodeAction>> {
        let (analysis, file_id) = self.analysis_for_open_document(uri)?;
        let text = analysis
            .file_text(file_id)
            .ok_or_else(|| anyhow!("document `{}` is missing file text", uri.as_str()))?;
        let requested_range = lsp_range_to_text_range(text.as_ref(), range)?;
        let file_range = TextRange::new(TextSize::from(0), TextSize::from(text.len() as u32));

        let mut actions = diagnostic_fix_actions(
            &analysis,
            file_id,
            text.as_ref(),
            requested_range,
            diagnostics,
            requested_kinds,
        );

        actions.extend(
            analysis
                .assists(file_id, requested_range)
                .into_iter()
                .filter(|assist| assist.kind != AssistKind::Source)
                .filter_map(|assist| {
                    let kind = assist_kind_to_lsp(&assist);
                    kind_matches_requested(&kind, requested_kinds).then(|| {
                        resolved_assist_to_action(text.as_ref(), assist, kind, diagnostics, false)
                    })
                }),
        );

        if let Some(source_change) = analysis.remove_unused_imports(file_id) {
            let kind = CodeActionKind::SOURCE_FIX_ALL;
            if kind_matches_requested(&kind, requested_kinds) {
                actions.push(ResolvedCodeAction {
                    id: "import.remove_unused".to_owned(),
                    title: "Remove unused imports".to_owned(),
                    kind,
                    target: file_range,
                    diagnostics: Vec::new(),
                    is_preferred: false,
                    source_change,
                });
            }
        }

        if let Some(source_change) = analysis.organize_imports(file_id) {
            let kind = CodeActionKind::SOURCE_ORGANIZE_IMPORTS;
            if kind_matches_requested(&kind, requested_kinds) {
                actions.push(ResolvedCodeAction {
                    id: "import.organize".to_owned(),
                    title: "Organize imports".to_owned(),
                    kind,
                    target: file_range,
                    diagnostics: Vec::new(),
                    is_preferred: false,
                    source_change,
                });
            }
        }

        dedupe_actions(&mut actions);
        Ok(actions)
    }

    pub(crate) fn resolve_code_action(
        &self,
        payload: &CodeActionResolvePayload,
    ) -> Result<Option<ResolvedCodeAction>> {
        let uri: Uri = payload
            .uri
            .parse()
            .map_err(|_| anyhow!("invalid code action uri `{}`", payload.uri))?;
        let requested = self.code_actions(
            &uri,
            Range {
                start: payload.request_range.start,
                end: payload.request_range.end,
            },
            &[],
            None,
        )?;

        Ok(requested.into_iter().find(|action| {
            action.id == payload.id
                && action.title == payload.title
                && action.kind.as_str() == payload.kind.as_str()
                && action.target.start() == TextSize::from(payload.target_start)
                && action.target.end() == TextSize::from(payload.target_end)
        }))
    }
}

fn resolved_assist_to_action(
    text: &str,
    assist: Assist,
    kind: CodeActionKind,
    diagnostics: &[Diagnostic],
    is_preferred: bool,
) -> ResolvedCodeAction {
    ResolvedCodeAction {
        id: assist.id.as_str().to_owned(),
        title: assist.label,
        kind,
        target: assist.target,
        diagnostics: related_diagnostics(text, assist.target, diagnostics),
        is_preferred,
        source_change: assist.source_change,
    }
}

fn diagnostic_fix_actions(
    analysis: &rhai_ide::Analysis,
    file_id: rhai_vfs::FileId,
    text: &str,
    requested_range: TextRange,
    requested_diagnostics: &[Diagnostic],
    requested_kinds: Option<&[CodeActionKind]>,
) -> Vec<ResolvedCodeAction> {
    let mut actions = Vec::new();

    for entry in analysis.diagnostics_with_fixes(file_id) {
        if !text_ranges_intersect(entry.diagnostic.range, requested_range) {
            continue;
        }

        let Some(diagnostic) = diagnostic_to_lsp(text, &entry.diagnostic) else {
            continue;
        };
        if !diagnostic_matches_requested(&diagnostic, requested_diagnostics) {
            continue;
        }

        let mut preferred_quickfix_assigned = false;
        for assist in entry.fixes {
            let kind = assist_kind_to_lsp(&assist);
            if !kind_matches_requested(&kind, requested_kinds) {
                continue;
            }

            let is_preferred = kind == CodeActionKind::QUICKFIX && !preferred_quickfix_assigned;
            preferred_quickfix_assigned |= kind == CodeActionKind::QUICKFIX;
            actions.push(resolved_assist_to_action(
                text,
                assist,
                kind,
                std::slice::from_ref(&diagnostic),
                is_preferred,
            ));
        }
    }

    actions
}

fn assist_kind_to_lsp(assist: &Assist) -> CodeActionKind {
    match assist.kind {
        AssistKind::QuickFix => CodeActionKind::QUICKFIX,
        AssistKind::Refactor => CodeActionKind::REFACTOR,
        AssistKind::Source => match assist.id.as_str() {
            "import.organize" => CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
            "import.remove_unused" => CodeActionKind::SOURCE_FIX_ALL,
            _ => CodeActionKind::SOURCE,
        },
    }
}

fn related_diagnostics(
    text: &str,
    target: TextRange,
    diagnostics: &[Diagnostic],
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let range = lsp_range_to_text_range(text, diagnostic.range).ok()?;
            text_ranges_intersect(target, range).then(|| diagnostic.clone())
        })
        .collect()
}

fn kind_matches_requested(
    kind: &CodeActionKind,
    requested_kinds: Option<&[CodeActionKind]>,
) -> bool {
    requested_kinds.is_none_or(|requested_kinds| {
        requested_kinds.iter().any(|requested| {
            kind.as_str() == requested.as_str()
                || kind
                    .as_str()
                    .strip_prefix(requested.as_str())
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    })
}

fn dedupe_actions(actions: &mut Vec<ResolvedCodeAction>) {
    actions.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.target.start().cmp(&right.target.start()))
            .then_with(|| left.target.end().cmp(&right.target.end()))
            .then_with(|| right.is_preferred.cmp(&left.is_preferred))
            .then_with(|| right.diagnostics.len().cmp(&left.diagnostics.len()))
    });
    actions.dedup_by(|left, right| {
        left.id == right.id
            && left.title == right.title
            && left.kind == right.kind
            && left.target == right.target
            && left.source_change == right.source_change
    });
}

fn text_ranges_intersect(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn diagnostic_matches_requested(diagnostic: &Diagnostic, requested: &[Diagnostic]) -> bool {
    requested.is_empty()
        || requested
            .iter()
            .any(|candidate| diagnostics_equivalent(diagnostic, candidate))
}

fn diagnostics_equivalent(left: &Diagnostic, right: &Diagnostic) -> bool {
    left.range == right.range
        && left.message == right.message
        && left.severity == right.severity
        && left.source == right.source
        && left.tags == right.tags
}
