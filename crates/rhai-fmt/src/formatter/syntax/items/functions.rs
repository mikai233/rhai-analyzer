use rhai_syntax::*;

use crate::formatter::Formatter;
use crate::formatter::layout::doc::Doc;

impl Formatter<'_> {
    pub(crate) fn format_function(&self, function: FnItem, indent: usize) -> Doc {
        let params = function.params();
        let params_doc = self.format_params_doc(params.clone(), indent);
        let body_expr = function.body();
        let signature = self.format_function_signature_doc(&function);
        let body = body_expr
            .clone()
            .map(|body| self.format_block_doc(body, indent))
            .unwrap_or_else(|| Doc::text("{}"));
        let body_separator = match (params.clone(), body_expr.clone()) {
            (Some(params), Some(body)) => self.head_body_separator_for_boundary(
                function.syntax(),
                TriviaBoundary::NodeNode(params.syntax(), body.syntax()),
            ),
            _ => {
                let params_end = params
                    .map(|params| u32::from(params.syntax().text_range().end()) as usize)
                    .unwrap_or_else(|| u32::from(function.syntax().text_range().start()) as usize);
                let body_start = body_expr
                    .map(|body| u32::from(body.syntax().text_range().start()) as usize)
                    .unwrap_or_else(|| u32::from(function.syntax().text_range().end()) as usize);
                self.head_body_separator_doc(params_end, body_start)
            }
        };

        Doc::concat(vec![
            signature,
            self.function_params_separator_doc(&function),
            params_doc,
            body_separator,
            body,
        ])
    }

    pub(crate) fn function_requires_raw_fallback(&self, function: &FnItem) -> bool {
        let Some(params) = function.params() else {
            return self.node_has_unowned_comments(function.syntax());
        };
        let signature_tokens = function.signature_tokens().collect::<Vec<_>>();
        let mut allowed_boundaries = signature_tokens
            .windows(2)
            .map(|pair| TriviaBoundary::TokenToken(pair[0].clone(), pair[1].clone()))
            .collect::<Vec<_>>();
        if let Some(last_token) = signature_tokens.last().cloned() {
            allowed_boundaries.push(TriviaBoundary::TokenNode(last_token, params.syntax()));
        }
        if let Some(body) = function.body() {
            allowed_boundaries.push(TriviaBoundary::NodeNode(params.syntax(), body.syntax()));
        }

        self.node_has_unowned_comments_outside_boundaries(function.syntax(), &allowed_boundaries)
    }

    pub(crate) fn format_function_signature_doc(&self, function: &FnItem) -> Doc {
        let tokens = function.signature_tokens();
        let mut parts = Vec::new();
        let mut previous: Option<SyntaxToken> = None;

        for token in tokens {
            if let Some(previous) = previous.as_ref() {
                let gap = self.boundary_trivia(
                    function.syntax(),
                    TriviaBoundary::TokenToken(previous.clone(), token.clone()),
                );
                parts.push(self.function_signature_separator_doc(
                    gap,
                    function_signature_inline_separator(previous, &token),
                ));
            }

            parts.push(Doc::text(token.text()));
            previous = Some(token);
        }

        Doc::group(Doc::concat(parts))
    }

    pub(crate) fn function_params_separator_doc(&self, function: &FnItem) -> Doc {
        let Some(params) = function.params() else {
            return Doc::nil();
        };
        let Some(last_token) = function.signature_tokens().last() else {
            return Doc::nil();
        };
        let Some(gap) = self.boundary_trivia(
            function.syntax(),
            TriviaBoundary::TokenNode(last_token, params.syntax()),
        ) else {
            return Doc::nil();
        };

        self.tight_comment_gap_from_gap(&gap, true)
    }

    pub(crate) fn function_signature_separator_doc(
        &self,
        gap: Option<GapTrivia>,
        inline_text: &str,
    ) -> Doc {
        if gap.as_ref().is_none_or(|gap| !gap.has_comments()) {
            return if inline_text.is_empty() {
                Doc::text("")
            } else {
                Doc::soft_line()
            };
        }

        self.tight_comment_gap_from_gap(&gap.unwrap_or_default(), true)
    }
}

fn function_signature_inline_separator(
    previous: &SyntaxToken,
    current: &SyntaxToken,
) -> &'static str {
    match (previous.kind().token_kind(), current.kind().token_kind()) {
        (Some(TokenKind::PrivateKw), Some(TokenKind::FnKw)) => " ",
        (Some(TokenKind::FnKw), _) => " ",
        (Some(TokenKind::Dot), _) => "",
        (_, Some(TokenKind::Dot)) => "",
        _ => " ",
    }
}
