use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionProviderCapability,
    DocumentChangeOperation, DocumentChanges, FoldingRangeKind, FormattingOptions, OneOf, Position,
    Range, ResourceOp, SelectionRangeProviderCapability, SemanticTokenModifier, SemanticTokenType,
    SemanticTokensFullDeltaResult, SemanticTokensFullOptions, SemanticTokensServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};
use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};
use rhai_syntax::TextSize;

use crate::handlers::queries::semantic_token_legend;
use crate::protocol::{diagnostic_to_lsp, rename_to_workspace_edit};
use crate::state::uri_from_path;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};
use crate::{InlayHintSettings, Server, ServerSettings};

#[test]
fn capabilities_expose_signature_help_inlay_hints_workspace_symbols_and_semantic_tokens() {
    let server = Server::new();
    let capabilities = server.capabilities();

    assert!(matches!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Options(ref options))
            if options.open_close == Some(true)
                && options.change == Some(TextDocumentSyncKind::INCREMENTAL)
    ));
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
    assert!(matches!(
        capabilities.declaration_provider,
        Some(lsp_types::DeclarationCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.type_definition_provider,
        Some(lsp_types::TypeDefinitionProviderCapability::Simple(true))
    ));
    assert!(capabilities.signature_help_provider.is_some());
    assert_eq!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.resolve_provider),
        Some(true)
    );
    assert!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.trigger_characters.as_ref())
            .is_some_and(|triggers| triggers.iter().any(|trigger| trigger == ":"))
    );
    assert!(
        capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.trigger_characters.as_ref())
            .is_some_and(|triggers| {
                !triggers
                    .iter()
                    .any(|trigger| trigger == " " || trigger == "a" || trigger == "A")
            }),
        "expected narrowed completion triggers"
    );
    assert!(matches!(
        capabilities.inlay_hint_provider,
        Some(OneOf::Right(_))
    ));
    assert!(matches!(
        capabilities.workspace_symbol_provider,
        Some(OneOf::Left(true))
    ));
    assert!(
        capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_folders.as_ref())
            .is_some_and(|folders| {
                folders.supported == Some(true)
                    && folders.change_notifications == Some(OneOf::Left(true))
            }),
        "expected workspace folder support in workspace capabilities"
    );
    assert!(
        capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.file_operations.as_ref())
            .and_then(|operations| operations.did_rename.as_ref())
            .is_some(),
        "expected didRenameFiles registration in workspace capabilities"
    );
    assert!(matches!(
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(ref options))
            if options.range == Some(true)
                && matches!(
                    options.full,
                    Some(SemanticTokensFullOptions::Delta { delta: Some(true) })
                )
    ));
    assert!(capabilities.document_formatting_provider.is_some());
    assert!(capabilities.document_range_formatting_provider.is_some());
    assert!(
        capabilities
            .document_on_type_formatting_provider
            .as_ref()
            .is_some_and(|options| {
                options.first_trigger_character == ";"
                    && options
                        .more_trigger_character
                        .as_ref()
                        .is_some_and(|chars| chars.iter().any(|ch| ch == "}"))
            }),
        "expected on-type formatting triggers for ';' and '}}'"
    );
    assert!(matches!(
        capabilities.selection_range_provider,
        Some(SelectionRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.linked_editing_range_provider,
        Some(lsp_types::LinkedEditingRangeServerCapabilities::Simple(
            true
        ))
    ));
    assert!(matches!(
        capabilities.code_action_provider,
        Some(CodeActionProviderCapability::Options(ref options))
            if options.resolve_provider == Some(true)
                && options
                    .code_action_kinds
                    .as_ref()
                    .is_some_and(|kinds| kinds.contains(&CodeActionKind::SOURCE_ORGANIZE_IMPORTS))
    ));
    assert!(matches!(
        capabilities.rename_provider,
        Some(OneOf::Right(ref options)) if options.prepare_provider == Some(true)
    ));
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
fn on_type_formatting_queries_reformat_current_structure_for_statement_terminators() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "fn run(){\nlet value=1;\n}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_on_type(
            &uri,
            Position {
                line: 2,
                character: 1,
            },
            "}",
            default_formatting_options(),
        )
        .expect("expected on-type formatting query to succeed")
        .expect("expected on-type formatting edits");

    assert!(
        !edits.is_empty(),
        "expected non-empty on-type formatting edits"
    );
}

#[test]
fn code_actions_include_source_actions_and_resolve_to_workspace_edits() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        import "z" as z;
        import "a" as a;

        fn run() {
            a::work();
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let actions = server
        .code_actions(
            &uri,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            &[],
            Some(&[CodeActionKind::SOURCE_ORGANIZE_IMPORTS]),
        )
        .expect("expected code actions query to succeed");

    let organize = actions
        .into_iter()
        .find(|action| action.kind == CodeActionKind::SOURCE_ORGANIZE_IMPORTS)
        .expect("expected organize imports action");
    assert!(
        !organize.is_preferred,
        "expected source organize imports action to avoid preferred quick-fix semantics"
    );

    let payload = crate::protocol::CodeActionResolvePayload {
        uri: uri.to_string(),
        request_range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 0,
            },
        },
        id: organize.id,
        kind: organize.kind.as_str().to_owned(),
        title: organize.title,
        target_start: u32::from(organize.target.start()),
        target_end: u32::from(organize.target.end()),
    };

    let resolved = server
        .resolve_code_action(&payload)
        .expect("expected code action resolve to succeed")
        .expect("expected resolved action");
    assert!(
        !resolved.source_change.file_edits.is_empty(),
        "expected resolved code action to contain file edits"
    );
}

#[test]
fn code_actions_prefer_diagnostic_quickfixes_and_attach_matching_diagnostics() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = "import \"shared_tools\" as tools;\nfn run() {}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let snapshot = server.analysis_host().snapshot();
    let file_id = snapshot
        .file_id_for_path(&std::env::current_dir().expect("cwd").join("main.rhai"))
        .expect("expected main.rhai file id");
    let diagnostics = snapshot
        .diagnostics(file_id)
        .iter()
        .filter_map(|diagnostic| diagnostic_to_lsp(text, diagnostic))
        .collect::<Vec<_>>();

    let actions = server
        .code_actions(
            &uri,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            &diagnostics,
            Some(&[CodeActionKind::QUICKFIX]),
        )
        .expect("expected code actions query to succeed");

    let remove_unused = actions
        .into_iter()
        .find(|action| action.id == "import.remove_unused")
        .expect("expected remove unused import quickfix");
    assert!(remove_unused.is_preferred);
    assert_eq!(remove_unused.diagnostics.len(), 1);
    assert_eq!(
        remove_unused.diagnostics[0].message,
        "unused symbol `tools`"
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
        .format_document(&uri, default_formatting_options())
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
            default_formatting_options(),
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

#[test]
fn document_formatting_queries_apply_request_and_server_formatting_options() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            max_line_length: 12,
            trailing_commas: false,
            final_newline: false,
            container_layout: ContainerLayoutStyle::Auto,
            import_sort_order: ImportSortOrder::Preserve,
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = "fn run(){let values=[12345,67890,abcde];}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(
            &uri,
            FormattingOptions {
                tab_size: 2,
                insert_spaces: false,
                ..default_formatting_options()
            },
        )
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(edit.new_text.contains("\tlet values = ["));
    assert!(edit.new_text.contains("\t\t12345,"));
    assert!(edit.new_text.contains("\t\tabcde\n\t];"));
    assert!(!edit.new_text.contains("\t\tabcde,\n\t];"));
    assert!(!edit.new_text.ends_with('\n'));
}

#[test]
fn document_formatting_queries_apply_container_layout_preferences() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            container_layout: ContainerLayoutStyle::PreferMultiLine,
            ..crate::FormatterSettings::default()
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text = "fn run(){let values=[1,2,3]; helper(alpha,beta);}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri, default_formatting_options())
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(edit.new_text.contains("let values = [\n"));
    assert!(edit.new_text.contains("helper(\n"));
}

#[test]
fn document_formatting_queries_apply_import_sorting_preferences() {
    let mut server = Server::new();
    server.configure_settings(ServerSettings {
        formatter: crate::FormatterSettings {
            import_sort_order: ImportSortOrder::ModulePath,
            ..crate::FormatterSettings::default()
        },
        ..ServerSettings::default()
    });

    let uri = file_url("main.rhai");
    let text =
        "import \"zebra\" as zebra;\nimport \"alpha\";\nimport \"beta\" as beta;\nfn run(){}\n";

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let edits = server
        .format_document(&uri, default_formatting_options())
        .expect("expected format document query to succeed")
        .expect("expected formatting edits");
    let [edit] = edits.as_slice() else {
        panic!("expected a single formatting edit");
    };

    assert!(
        edit.new_text.starts_with(
            "import \"alpha\";\nimport \"beta\" as beta;\nimport \"zebra\" as zebra;\n"
        )
    );
}

#[test]
fn workspace_preload_enables_cross_file_references_and_rename_for_unopened_importers() {
    let workspace = create_temp_workspace("workspace-preload");
    let provider_path = workspace.join("provider.rhai");
    let consumer_path = workspace.join("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"provider\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    fs::write(&provider_path, provider_text).expect("expected provider write to succeed");
    fs::write(&consumer_path, consumer_text).expect("expected consumer write to succeed");

    let mut server = Server::new();
    server
        .load_workspace_roots(std::slice::from_ref(&workspace))
        .expect("expected workspace preload to succeed");

    let provider_uri = uri_from_path(&provider_path).expect("expected provider uri");
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");

    let references = server
        .find_references(&provider_uri, offset_in(provider_text, "hello") + 1)
        .expect("expected references query to succeed")
        .expect("expected references");
    assert!(
        references
            .references
            .iter()
            .any(|reference| reference.file_id
                == server
                    .analysis_host()
                    .snapshot()
                    .file_id_for_path(&consumer_path)
                    .expect("expected consumer file id")),
        "expected consumer references, got {references:?}"
    );

    let prepared = server
        .rename(
            &provider_uri,
            offset_in(provider_text, "hello") + 1,
            "renamed_hello".to_owned(),
        )
        .expect("expected rename query to succeed")
        .expect("expected prepared rename");
    let source_change = prepared
        .source_change
        .expect("expected rename source change");
    let consumer_file_id = server
        .analysis_host()
        .snapshot()
        .file_id_for_path(&consumer_path)
        .expect("expected consumer file id");
    assert!(
        source_change
            .file_edits
            .iter()
            .any(|edit| edit.file_id == consumer_file_id),
        "expected consumer file edits, got {source_change:?}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn static_imports_can_load_modules_outside_workspace_roots() {
    let base = create_temp_workspace("external-imports");
    let workspace = base.join("workspace");
    let shared = base.join("shared");
    fs::create_dir_all(&workspace).expect("expected workspace directory");
    fs::create_dir_all(&shared).expect("expected shared directory");

    let provider_path = shared.join("provider.rhai");
    let consumer_path = workspace.join("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"../shared/provider\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    fs::write(&provider_path, provider_text).expect("expected provider write to succeed");
    fs::write(&consumer_path, consumer_text).expect("expected consumer write to succeed");

    let mut server = Server::new();
    server
        .load_workspace_roots(std::slice::from_ref(&workspace))
        .expect("expected workspace preload to succeed");

    let consumer_uri = uri_from_path(&consumer_path).expect("expected consumer uri");
    let provider_uri = uri_from_path(&provider_path).expect("expected provider uri");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let definitions = server
        .goto_definition(&consumer_uri, offset_in(consumer_text, "hello") + 1)
        .expect("expected goto definition query to succeed");
    assert!(
        definitions.iter().any(|target| target.file_id
            == server
                .analysis_host()
                .snapshot()
                .file_id_for_path(&provider_path)
                .expect("expected provider file id")),
        "expected external provider target, got {definitions:?}"
    );

    let symbols = server
        .workspace_symbols("hello")
        .expect("expected workspace symbols query to succeed");
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.uri == provider_uri && symbol.symbol.name == "hello"),
        "expected external provider symbol, got {symbols:?}"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn goto_and_references_resolve_local_symbols_in_object_field_values() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn make_config(root, mode) {
            let workspace_name = workspace::name(root);
            let config = #{
                mode: mode,
                workspace: workspace_name,
            };
            config
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let mode_decl = offset_in(text, "root, mode") + 6;
    let mode_usage = offset_in(text, "mode: mode") + 7;
    let workspace_decl = offset_in(text, "workspace_name =");
    let workspace_usage = offset_in(text, "workspace: workspace_name") + 11;

    let mode_definitions = server
        .goto_definition(&uri, mode_usage)
        .expect("expected goto definition query to succeed");
    assert_eq!(mode_definitions.len(), 1);
    assert!(
        mode_definitions[0]
            .full_range
            .contains(TextSize::from(mode_decl))
    );

    let workspace_definitions = server
        .goto_definition(&uri, workspace_usage)
        .expect("expected goto definition query to succeed");
    assert_eq!(workspace_definitions.len(), 1);
    assert!(
        workspace_definitions[0]
            .full_range
            .contains(TextSize::from(workspace_decl))
    );

    let mode_references = server
        .find_references(&uri, mode_decl)
        .expect("expected references query to succeed")
        .expect("expected references");
    assert!(mode_references.references.iter().any(|reference| {
        reference.file_id == mode_definitions[0].file_id
            && reference.range.contains(TextSize::from(mode_usage))
    }));

    let workspace_references = server
        .find_references(&uri, workspace_decl)
        .expect("expected references query to succeed")
        .expect("expected references");
    assert!(workspace_references.references.iter().any(|reference| {
        reference.file_id == workspace_definitions[0].file_id
            && reference.range.contains(TextSize::from(workspace_usage))
    }));
}

#[test]
fn rename_updates_object_field_usages_across_files() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = r#"
        export const DEFAULTS = #{
            name: "demo",
            watch: true,
        };
    "#;
    let consumer_text = r#"
        import "provider" as tools;
        let value = tools::DEFAULTS.name;
    "#;

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let prepared = server
        .rename(
            &provider_uri,
            offset_in(provider_text, "name: \"demo\""),
            "title".to_owned(),
        )
        .expect("expected rename query to succeed")
        .expect("expected prepared rename");
    assert!(
        prepared.plan.issues.is_empty(),
        "{:?}",
        prepared.plan.issues
    );

    let source_change = prepared
        .source_change
        .expect("expected object field rename source change");
    assert!(
        source_change.file_edits.len() >= 2,
        "expected provider+consumer edits, got {:?}",
        source_change.file_edits
    );
    assert!(
        source_change
            .file_edits
            .iter()
            .all(|file_edit| file_edit.edits.iter().all(|edit| edit.new_text == "title"))
    );
}

#[test]
fn rename_on_static_import_module_reference_returns_text_edits_and_file_rename() {
    let mut server = Server::new();
    let provider_uri = file_url("demo.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let prepared = server
        .rename(
            &consumer_uri,
            offset_in(consumer_text, "\"demo\"") + 1,
            "renamed_demo".to_owned(),
        )
        .expect("expected rename query to succeed")
        .expect("expected prepared rename");
    let workspace_edit =
        rename_to_workspace_edit(&server, prepared).expect("expected workspace edit");
    let document_changes = workspace_edit
        .document_changes
        .expect("expected document changes");
    let DocumentChanges::Operations(document_changes) = document_changes else {
        panic!("expected operation-based workspace edit");
    };

    assert!(
        document_changes
            .iter()
            .any(|change| matches!(change, DocumentChangeOperation::Edit(_))),
        "expected text edits in workspace edit, got {document_changes:?}"
    );
    assert!(
        document_changes.iter().any(|change| matches!(
            change,
            DocumentChangeOperation::Op(ResourceOp::Rename(rename))
                if rename.new_uri.as_str().ends_with("/renamed_demo.rhai")
                    || rename.new_uri.as_str().ends_with("\\renamed_demo.rhai")
        )),
        "expected file rename in workspace edit, got {document_changes:?}"
    );
}

#[test]
fn static_import_module_can_be_renamed_twice_after_file_rename_notification() {
    let mut server = Server::new();
    let provider_uri = file_url("demo.rhai");
    let renamed_provider_uri = file_url("renamed_demo.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn hello() {}\n";
    let consumer_text = "import \"demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";
    let renamed_consumer_text = "import \"renamed_demo\" as d;\n\nfn run() {\n    d::hello();\n}\n";

    assert_valid_rhai_syntax(provider_text);
    assert_valid_rhai_syntax(consumer_text);
    assert_valid_rhai_syntax(renamed_consumer_text);
    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    server
        .rename_workspace_file(&provider_uri, &renamed_provider_uri)
        .expect("expected file rename notification to succeed");
    server
        .change_document(consumer_uri.clone(), 2, renamed_consumer_text)
        .expect("expected consumer rename edit to succeed");

    let second = server
        .rename(
            &consumer_uri,
            offset_in(renamed_consumer_text, "\"renamed_demo\"") + 1,
            "demo_again".to_owned(),
        )
        .expect("expected second rename query to succeed");
    assert!(
        second.is_some(),
        "expected second static import rename to resolve"
    );
}

fn create_temp_workspace(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("expected system time")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("rhai-analyzer-{prefix}-{unique}"));
    fs::create_dir_all(&workspace).expect("expected temporary workspace directory");
    workspace
}

fn default_formatting_options() -> FormattingOptions {
    FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..FormattingOptions::default()
    }
}
