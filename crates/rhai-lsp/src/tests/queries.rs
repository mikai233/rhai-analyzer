use lsp_types::{
    CallHierarchyServerCapability, FoldingRangeKind, OneOf, Position, Range,
    SemanticTokenModifier, SemanticTokenType, SemanticTokensServerCapabilities,
};

use crate::Server;
use crate::handlers::queries::semantic_token_legend;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

#[test]
fn capabilities_expose_signature_help_inlay_hints_workspace_symbols_and_semantic_tokens() {
    let server = Server::new();
    let capabilities = server.capabilities();

    assert!(matches!(
        capabilities.document_highlight_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.call_hierarchy_provider,
        Some(CallHierarchyServerCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.folding_range_provider,
        Some(lsp_types::FoldingRangeProviderCapability::Simple(true))
    ));
    assert!(capabilities.signature_help_provider.is_some());
    assert_eq!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.resolve_provider),
        Some(true)
    );
    assert!(matches!(
        capabilities.inlay_hint_provider,
        Some(OneOf::Right(_))
    ));
    assert!(matches!(
        capabilities.workspace_symbol_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(_))
    ));
    assert!(capabilities.document_formatting_provider.is_some());
    assert!(capabilities.document_range_formatting_provider.is_some());
}

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
        .inlay_hints(&uri)
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
        .semantic_tokens(&uri)
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
fn workspace_symbol_queries_return_uri_backed_results() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn helper() {}\nconst VALUE = 1;";
    let consumer_text = "fn run() { helper(); }";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let symbols = server
        .workspace_symbols("help")
        .expect("expected workspace symbols query to succeed");

    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.uri == provider_uri && symbol.symbol.name == "helper"),
        "expected provider helper symbol, got {symbols:?}"
    );
}

#[test]
fn document_formatting_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "fn run(){let value=1+2;value}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri)
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.range.start.character, 0);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 0);
    assert_eq!(
        edit.new_text,
        "fn run() {\n    let value = 1 + 2;\n    value\n}\n"
    );
}

#[test]
fn document_range_formatting_queries_flow_through_server() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "let prefix = 1;\nfn run(){let value=1+2;value}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_range(
            &uri,
            Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 2,
                    character: 0,
                },
            },
        )
        .expect("expected format range query to succeed")
        .expect("expected range formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(edit.range.start.character, 0);
    assert!(edit.range.end.line <= 2);
    assert!(edit.new_text.contains("fn run"));
    assert!(edit.new_text.contains("let value = 1 + 2;"));
}
