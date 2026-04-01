use lsp_types::{
    FoldingRangeKind, Position, Range, SemanticTokenModifier, SemanticTokenType,
    SemanticTokensFullDeltaResult,
};
use rhai_syntax::TextSize;

use crate::handlers::queries::semantic_token_legend;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};
use crate::{InlayHintSettings, Server, ServerSettings};

#[test]
fn call_hierarchy_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn leaf() {}

        fn middle() {
            leaf();
        }

        fn root() {
            middle();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let items = server
        .prepare_call_hierarchy(&uri, offset_in(text, "middle();"))
        .expect("expected call hierarchy prepare to succeed");
    let middle = items
        .into_iter()
        .find(|item| item.name == "middle")
        .expect("expected middle call hierarchy item");

    let incoming = server
        .incoming_calls(&middle)
        .expect("expected incoming call query to succeed");
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from.name, "root");

    let outgoing = server
        .outgoing_calls(&middle)
        .expect("expected outgoing call query to succeed");
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].to.name, "leaf");
}
#[test]
fn folding_range_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// docs line 1
        /// docs line 2
        fn helper() {
            let values = [
                1,
                2,
            ];
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let ranges = server
        .folding_ranges(&uri)
        .expect("expected folding range query to succeed");

    assert!(
        ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Comment)),
        "expected a folded comment range, got {ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Region)),
        "expected a folded region range, got {ranges:?}"
    );
}
#[test]
fn document_highlight_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper() {}

        fn run() {
            helper();
            helper();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let highlights = server
        .document_highlights(&uri, offset_in(text, "helper();"))
        .expect("expected document highlight query to succeed");

    assert_eq!(highlights.len(), 3);
    assert_eq!(highlights[0].kind, rhai_ide::DocumentHighlightKind::Write);
}
#[test]
fn prepare_rename_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper(value) {
            value
        }

        fn run() {
            helper(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let prepared = server
        .prepare_rename(&uri, offset_in(text, "helper(1)") + 1)
        .expect("expected prepare rename query to succeed")
        .expect("expected prepared rename");
    let query_offset = TextSize::from(offset_in(text, "helper(1)") + 1);

    assert!(
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
                    .map(|occurrence| occurrence.range)
            )
            .any(|range| range.contains(query_offset))
    );
}
#[test]
fn declaration_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper(value) {
            value
        }

        fn run() {
            helper(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let declarations = server
        .goto_declaration(&uri, offset_in(text, "helper(1)") + 1)
        .expect("expected goto declaration query to succeed");
    let definitions = server
        .goto_definition(&uri, offset_in(text, "helper(1)") + 1)
        .expect("expected goto definition query to succeed");

    assert_eq!(declarations, definitions);
    assert_eq!(declarations.len(), 1);
}
#[test]
fn type_definition_queries_follow_structural_object_sources() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        let original = #{
            name: "demo",
            watch: true,
        };
        let alias = original;
        let current = alias;
        current.name
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let definitions = server
        .goto_type_definition(&uri, offset_in(text, "current.name") + 1)
        .expect("expected goto type definition query to succeed");

    assert_eq!(definitions.len(), 1);
    assert!(
        definitions[0]
            .full_range
            .contains(TextSize::from(offset_in(text, "#{") + 1)),
        "expected structural object source target, got {definitions:?}"
    );
}
#[test]
fn type_definition_queries_can_target_documented_symbol_annotations() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// @type int
        let answer = 1;
        answer
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let definitions = server
        .goto_type_definition(&uri, offset_in(text, "answer\n") + 1)
        .expect("expected goto type definition query to succeed");

    assert_eq!(definitions.len(), 1);
    assert!(
        definitions[0]
            .full_range
            .contains(TextSize::from(offset_in(text, "@type int") + 1)),
        "expected doc annotation target, got {definitions:?}"
    );
}
#[test]
fn selection_range_queries_expand_from_identifier_to_enclosing_items() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn run() {
            let value = helper(1 + 2);
            value
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let position = Position {
        line: 2,
        character: 24,
    };
    let ranges = server
        .selection_ranges(&uri, &[position])
        .expect("expected selection range query to succeed");
    let selection = ranges.into_iter().next().expect("expected selection range");

    assert_eq!(selection.range.start.line, 2);
    assert!(
        selection.parent.is_some(),
        "expected nested selection range"
    );
    assert!(
        selection
            .parent
            .as_ref()
            .and_then(|parent| parent.parent.as_ref())
            .is_some(),
        "expected multiple selection range parents"
    );
}
#[test]
fn linked_editing_ranges_follow_same_file_identifier_occurrences() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn make_config() {
            let value = 1;
            value + value
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let linked = server
        .linked_editing_ranges(&uri, offset_in(text, "value +") + 1)
        .expect("expected linked editing query to succeed")
        .expect("expected linked editing ranges");

    assert_eq!(
        linked.word_pattern.as_deref(),
        Some(r"[A-Za-z_][A-Za-z0-9_]*")
    );
    assert!(
        linked.ranges.len() >= 2,
        "expected at least declaration and usage ranges, got {:?}",
        linked.ranges
    );
}
#[test]
fn completion_queries_and_resolve_flow_through_server() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "/// shared helper docs\nfn shared_helper() {}";
    let consumer_text = r#"
        fn run() {
            shared
        }
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri, 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let completion = server
        .completions(
            &consumer_uri,
            offset_in(consumer_text, "shared") + "shared".len() as u32,
        )
        .expect("expected completions query to succeed")
        .into_iter()
        .find(|item| item.label == "shared_helper")
        .expect("expected shared_helper completion");
    assert!(completion.docs.is_none());

    let resolved = server.resolve_completion(completion);
    assert_eq!(resolved.docs.as_deref(), Some("shared helper docs"));
}
#[test]
fn signature_help_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        /// @param left int
        /// @param right string
        /// @return bool
        fn check(left, right) {
            true
        }

        fn run() {
            check(1, value);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let help = server
        .signature_help(&uri, offset_in(text, "value"))
        .expect("expected signature help query to succeed")
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn check(left: int, right: string) -> bool"
    );
}
#[test]
fn inlay_hints_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper() {
            1
        }

        fn run() {
            let result = helper();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(&uri, None)
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == " -> int"),
        "expected function return hint, got {hints:?}"
    );
    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected inferred variable hint, got {hints:?}"
    );
}
#[test]
fn inlay_hints_queries_respect_server_settings() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        inlay_hints: InlayHintSettings {
            variables: false,
            parameters: true,
            return_types: false,
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = r#"
        fn helper(value) {
            value
        }

        fn run() {
            let result = helper(1);
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(&uri, None)
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected parameter hint, got {hints:?}"
    );
    assert!(
        !hints.iter().any(|hint| hint.label == " -> int"),
        "did not expect return-type hint, got {hints:?}"
    );
}
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
fn inlay_hints_queries_can_be_scoped_to_a_requested_range() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn helper() {
            1
        }

        fn run() {
            let result = helper();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let hints = server
        .inlay_hints(
            &uri,
            Some(Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 7,
                    character: 0,
                },
            }),
        )
        .expect("expected inlay hints query to succeed");

    assert!(
        hints.iter().any(|hint| hint.label == ": int"),
        "expected variable hint, got {hints:?}"
    );
    assert!(
        !hints.iter().any(|hint| hint.label == " -> int"),
        "did not expect helper return hint outside range, got {hints:?}"
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
