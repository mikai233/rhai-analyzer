use lsp_types::{
    Position, Range, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullDeltaResult,
};

use crate::Server;
use crate::handlers::queries::semantic_token_legend;
use crate::tests::{assert_valid_rhai_syntax, file_url};

#[test]
fn semantic_tokens_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// docs
        const LIMIT = 1;

        fn "Custom".trimmed(value) {
            this.len() + value + LIMIT
        }

        fn run() {
            blob(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let tokens = server
        .semantic_tokens(&uri, None)
        .expect("expected semantic tokens query to succeed");
    let legend = semantic_token_legend();

    let keyword_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::KEYWORD)
        .expect("expected keyword legend entry") as u32;
    let comment_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::COMMENT)
        .expect("expected comment legend entry") as u32;
    let function_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::FUNCTION)
        .expect("expected function legend entry") as u32;
    let type_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::TYPE)
        .expect("expected type legend entry") as u32;
    let method_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::METHOD)
        .expect("expected method legend entry") as u32;
    let declaration_bit = 1_u32
        << legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier == &SemanticTokenModifier::DECLARATION)
            .expect("expected declaration modifier");
    let readonly_bit = 1_u32
        << legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier == &SemanticTokenModifier::READONLY)
            .expect("expected readonly modifier");
    let default_library_bit = 1_u32
        << legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier == &SemanticTokenModifier::DEFAULT_LIBRARY)
            .expect("expected defaultLibrary modifier");

    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == keyword_index),
        "expected a keyword semantic token, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == comment_index),
        "expected a comment semantic token, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == function_index),
        "expected a function semantic token, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == type_index),
        "expected a type semantic token, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == method_index),
        "expected a method semantic token, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_modifiers_bitset & declaration_bit != 0),
        "expected a declaration semantic token modifier, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_modifiers_bitset & readonly_bit != 0),
        "expected a readonly semantic token modifier, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_modifiers_bitset & default_library_bit != 0),
        "expected a defaultLibrary semantic token modifier, got {:?}",
        tokens.data
    );
}
#[test]
fn semantic_tokens_queries_can_be_scoped_to_a_requested_range() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// docs
        const LIMIT = 1;

        fn run() {
            let value = LIMIT;
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let tokens = server
        .semantic_tokens(
            &uri,
            Some(Range {
                start: Position {
                    line: 3,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            }),
        )
        .expect("expected ranged semantic tokens query to succeed");
    let legend = semantic_token_legend();
    let comment_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::COMMENT)
        .expect("expected comment legend entry") as u32;
    let keyword_index = legend
        .token_types
        .iter()
        .position(|token_type| token_type == &SemanticTokenType::KEYWORD)
        .expect("expected keyword legend entry") as u32;

    assert!(
        tokens
            .data
            .iter()
            .all(|token| token.token_type != comment_index),
        "did not expect doc-comment tokens in ranged response, got {:?}",
        tokens.data
    );
    assert!(
        tokens
            .data
            .iter()
            .any(|token| token.token_type == keyword_index),
        "expected in-range tokens, got {:?}",
        tokens.data
    );
}
#[test]
fn semantic_tokens_delta_queries_return_incremental_edits_after_changes() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "fn run() {\n    let value = 1;\n    value\n}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let first = server
        .semantic_tokens(&uri, None)
        .expect("expected semantic tokens query to succeed");
    let first = server.semantic_tokens_full(&uri, first);
    let previous_result_id = first
        .result_id
        .clone()
        .expect("expected semantic token result id");

    server
        .change_document(
            uri.clone(),
            2,
            "fn run() {\n    let total = 1;\n    total\n}\n",
        )
        .expect("expected document change to succeed");

    let next = server
        .semantic_tokens(&uri, None)
        .expect("expected semantic tokens query to succeed");
    let delta = server.semantic_tokens_delta(&uri, &previous_result_id, next);

    match delta {
        SemanticTokensFullDeltaResult::TokensDelta(delta) => {
            assert!(
                !delta.edits.is_empty(),
                "expected semantic token delta edits"
            );
            assert!(delta.result_id.is_some(), "expected delta result id");
        }
        other => panic!("expected semantic token delta edits, got {other:?}"),
    }
}
