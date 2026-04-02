use lsp_types::{CodeActionKind, DocumentChangeOperation, DocumentChanges, ResourceOp};
use rhai_hir::SymbolKind;
use rhai_ide::{
    CompletionInsertFormat, CompletionItem, CompletionItemKind, CompletionItemSource,
    CompletionRelevance, FileRename, FileTextEdit, HoverResult, HoverSignatureSource,
    SignatureHelp as IdeSignatureHelp, SignatureInformation as IdeSignatureInformation,
    SignatureParameter as IdeSignatureParameter, SourceChange, TextEdit,
};
use rhai_syntax::{TextRange, TextSize};

use crate::protocol::{
    completion_item_from_lsp, completion_item_to_lsp, hover_to_lsp, prepared_rename_to_lsp,
    signature_help_to_lsp, source_change_code_action_to_lsp,
};
use crate::tests::file_url;
use crate::{Server, ServerState};

#[test]
fn code_action_conversion_supports_multi_file_edits_and_file_renames() {
    let mut server = Server::new();
    let provider_uri = file_url("provider.rhai");
    let consumer_uri = file_url("consumer.rhai");
    let provider_text = "fn helper() {}\n";
    let consumer_text = "import \"provider\" as p;\np::helper();\n";

    server
        .open_document(provider_uri.clone(), 1, provider_text)
        .expect("expected provider open to succeed");
    server
        .open_document(consumer_uri.clone(), 1, consumer_text)
        .expect("expected consumer open to succeed");

    let snapshot = server.analysis_host().snapshot();
    let provider_file_id = snapshot
        .file_id_for_path(&std::env::current_dir().expect("cwd").join("provider.rhai"))
        .expect("expected provider file id");
    let consumer_file_id = snapshot
        .file_id_for_path(&std::env::current_dir().expect("cwd").join("consumer.rhai"))
        .expect("expected consumer file id");
    let change = SourceChange::new(vec![
        FileTextEdit::new(
            provider_file_id,
            vec![TextEdit::replace(
                TextRange::new(TextSize::from(3), TextSize::from(9)),
                "renamed".to_owned(),
            )],
        ),
        FileTextEdit::new(
            consumer_file_id,
            vec![TextEdit::replace(
                TextRange::new(TextSize::from(24), TextSize::from(30)),
                "renamed".to_owned(),
            )],
        ),
    ])
    .with_file_renames(vec![FileRename::new(
        provider_file_id,
        std::env::current_dir()
            .expect("cwd")
            .join("renamed_provider.rhai"),
    )]);

    let action = source_change_code_action_to_lsp(
        &server,
        "Apply import fix",
        CodeActionKind::QUICKFIX,
        &change,
    )
    .expect("expected code action conversion");

    let lsp_types::CodeActionOrCommand::CodeAction(action) = action else {
        panic!("expected code action");
    };
    let document_changes = action
        .edit
        .expect("expected workspace edit")
        .document_changes
        .expect("expected document changes");
    let DocumentChanges::Operations(operations) = document_changes else {
        panic!("expected operation-based workspace edit");
    };

    assert!(
        operations
            .iter()
            .filter(|operation| matches!(operation, DocumentChangeOperation::Edit(_)))
            .count()
            >= 2
    );
    assert!(operations.iter().any(|operation| matches!(
        operation,
        DocumentChangeOperation::Op(ResourceOp::Rename(rename))
            if rename.new_uri.as_str().ends_with("/renamed_provider.rhai")
                || rename.new_uri.as_str().ends_with("\\renamed_provider.rhai")
    )));
}

#[test]
fn prepare_rename_placeholder_strips_surrounding_quotes() {
    let prepared = rhai_ide::PreparedRename {
        plan: rhai_ide::RenamePlan {
            new_name: String::new(),
            targets: Vec::new(),
            occurrences: vec![rhai_ide::ReferenceLocation {
                file_id: rhai_vfs::FileId(0),
                range: TextRange::new(TextSize::from(7), TextSize::from(13)),
                kind: rhai_ide::ReferenceKind::Reference,
            }],
            issues: Vec::new(),
        },
        source_change: None,
    };

    let response =
        prepared_rename_to_lsp("import \"demo\";\n", &prepared, 8).expect("expected response");

    match response {
        lsp_types::PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } => {
            assert_eq!(placeholder, "demo");
        }
        other => panic!("expected placeholder response, got {other:?}"),
    }
}

#[test]
fn hover_conversion_omits_duplicate_signature_lines() {
    let hover = hover_to_lsp(HoverResult {
        signature: "let result: any".to_owned(),
        docs: Some("hover docs".to_owned()),
        source: HoverSignatureSource::Declared,
        declared_signature: Some("let result: any".to_owned()),
        inferred_signature: Some("let result: any | blob".to_owned()),
        overload_signatures: vec!["fn result(blob) -> blob".to_owned()],
        notes: vec!["A note".to_owned()],
    });

    let lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(markup.value.contains("### Documentation"));
    assert!(markup.value.contains("hover docs"));
    assert!(markup.value.contains("### Source"));
    assert!(markup.value.contains("Declared"));
    assert!(!markup.value.contains("### Declared Signature"));
    assert!(markup.value.contains("### Inferred Signature"));
    assert!(
        markup
            .value
            .contains("```rhai\nlet result: any | blob\n```")
    );
    assert!(markup.value.contains("### Other Overloads"));
    assert!(
        markup
            .value
            .contains("```rhai\nfn result(blob) -> blob\n```")
    );
    assert!(markup.value.contains("### Notes"));
    assert!(markup.value.contains("- A note"));
}

#[test]
fn completion_conversion_surfaces_source_descriptions() {
    let server = ServerState::new();
    let item = completion_item_to_lsp(
        &server,
        None,
        CompletionItem {
            label: "shared_helper".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source: CompletionItemSource::Project,
            origin: None,
            sort_text: "0".to_owned(),
            detail: Some("fun() -> ()".to_owned()),
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::PlainText,
            relevance: CompletionRelevance::default(),
            file_id: None,
            exported: true,
            resolve_data: None,
        },
    );

    assert_eq!(item.detail.as_deref(), Some("project export"));
    assert_eq!(
        item.label_details
            .as_ref()
            .and_then(|details| details.detail.as_deref()),
        Some(" fun() -> ()")
    );
    assert_eq!(
        item.label_details
            .as_ref()
            .and_then(|details| details.description.as_deref()),
        None
    );
    assert_eq!(item.filter_text.as_deref(), Some("shared_helper"));
}

#[test]
fn completion_conversion_includes_project_module_name() {
    let mut server = ServerState::new();
    server
        .open_document(file_url("support.rhai"), 1, "fn shared_helper() {}")
        .expect("expected support.rhai to open");
    let file_id = server
        .analysis_host()
        .snapshot()
        .file_id_for_path(&std::env::current_dir().expect("cwd").join("support.rhai"))
        .expect("expected support.rhai");

    let item = completion_item_to_lsp(
        &server,
        None,
        CompletionItem {
            label: "shared_helper".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source: CompletionItemSource::Project,
            origin: Some("support".to_owned()),
            sort_text: "0".to_owned(),
            detail: Some("fun() -> ()".to_owned()),
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::PlainText,
            relevance: CompletionRelevance::default(),
            file_id: Some(file_id),
            exported: true,
            resolve_data: None,
        },
    );

    assert_eq!(item.detail.as_deref(), Some("project export · support"));
    assert_eq!(
        item.label_details
            .as_ref()
            .and_then(|details| details.detail.as_deref()),
        Some(" fun() -> ()")
    );
}

#[test]
fn completion_conversion_includes_module_origin_name() {
    let server = ServerState::new();

    let item = completion_item_to_lsp(
        &server,
        None,
        CompletionItem {
            label: "shared_helper".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source: CompletionItemSource::Module,
            origin: Some("demo".to_owned()),
            sort_text: "0".to_owned(),
            detail: Some("fun() -> ()".to_owned()),
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::PlainText,
            relevance: CompletionRelevance::default(),
            file_id: None,
            exported: true,
            resolve_data: None,
        },
    );

    assert_eq!(item.detail.as_deref(), Some("module export · demo"));
    assert_eq!(
        item.label_details
            .as_ref()
            .and_then(|details| details.detail.as_deref()),
        Some(" fun() -> ()")
    );
    assert_eq!(item.filter_text.as_deref(), Some("shared_helper"));
}

#[test]
fn completion_conversion_uses_label_filter_text_for_postfix_items() {
    let server = ServerState::new();
    let item = completion_item_to_lsp(
        &server,
        Some("let branch = student.name.s"),
        CompletionItem {
            label: "switch".to_owned(),
            kind: CompletionItemKind::Snippet,
            source: CompletionItemSource::Postfix,
            origin: None,
            sort_text: "0".to_owned(),
            detail: Some("postfix template".to_owned()),
            docs: None,
            filter_text: Some("student.name.switch".to_owned()),
            text_edit: Some(rhai_ide::CompletionTextEdit {
                replace_range: TextRange::new(TextSize::from(13), TextSize::from(27)),
                insert_range: Some(TextRange::new(TextSize::from(26), TextSize::from(27))),
                new_text: "switch student.name {\n    ${1:_} => {\n        $0\n    }\n}".to_owned(),
            }),
            insert_format: CompletionInsertFormat::Snippet,
            relevance: CompletionRelevance::default(),
            file_id: None,
            exported: false,
            resolve_data: None,
        },
    );

    assert_eq!(item.filter_text.as_deref(), Some("switch"));
    assert_eq!(item.kind, Some(lsp_types::CompletionItemKind::SNIPPET));
    match item.text_edit.as_ref().expect("expected text edit") {
        lsp_types::CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text,
                "switch student.name {\n    ${1:_} => {\n        $0\n    }\n}"
            );
        }
        other => panic!("expected simple edit, got {other:?}"),
    }
    assert_eq!(item.additional_text_edits.as_ref().map(Vec::len), Some(1));
}

#[test]
fn completion_roundtrip_restores_signature_detail_from_label_details() {
    let server = ServerState::new();
    let item = completion_item_to_lsp(
        &server,
        None,
        CompletionItem {
            label: "do_something".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source: CompletionItemSource::Visible,
            origin: None,
            sort_text: "0".to_owned(),
            detail: Some("fun(int) -> int".to_owned()),
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::Snippet,
            relevance: CompletionRelevance::default(),
            file_id: None,
            exported: false,
            resolve_data: Some(rhai_ide::CompletionResolveData {
                file_id: rhai_vfs::FileId(0),
                offset: 0,
            }),
        },
    );

    let restored = completion_item_from_lsp(item).expect("expected roundtrip completion");
    assert_eq!(restored.detail.as_deref(), Some("fun(int) -> int"));
}

#[test]
fn completion_roundtrip_restores_signature_detail_from_payload_without_ui_fields() {
    let server = ServerState::new();
    let mut item = completion_item_to_lsp(
        &server,
        None,
        CompletionItem {
            label: "do_something".to_owned(),
            kind: CompletionItemKind::Symbol(SymbolKind::Function),
            source: CompletionItemSource::Visible,
            origin: None,
            sort_text: "0".to_owned(),
            detail: Some("fun(int) -> int".to_owned()),
            docs: None,
            filter_text: None,
            text_edit: None,
            insert_format: CompletionInsertFormat::Snippet,
            relevance: CompletionRelevance::default(),
            file_id: None,
            exported: false,
            resolve_data: Some(rhai_ide::CompletionResolveData {
                file_id: rhai_vfs::FileId(0),
                offset: 0,
            }),
        },
    );
    item.detail = None;
    item.label_details = None;

    let restored = completion_item_from_lsp(item).expect("expected roundtrip completion");
    assert_eq!(restored.detail.as_deref(), Some("fun(int) -> int"));
}

#[test]
fn signature_help_uses_active_parameter_and_skips_empty_docs() {
    let help = signature_help_to_lsp(IdeSignatureHelp {
        signatures: vec![IdeSignatureInformation {
            label: "test(value: int, count: int)".to_owned(),
            docs: Some("   ".to_owned()),
            parameters: vec![
                IdeSignatureParameter {
                    label: "value".to_owned(),
                    annotation: Some("int".to_owned()),
                },
                IdeSignatureParameter {
                    label: "count".to_owned(),
                    annotation: Some("int".to_owned()),
                },
            ],
            file_id: None,
        }],
        active_signature: 0,
        active_parameter: 3,
    });

    assert_eq!(help.active_parameter, Some(3));
    assert_eq!(help.signatures[0].active_parameter, Some(1));
    assert!(help.signatures[0].documentation.is_none());
}
