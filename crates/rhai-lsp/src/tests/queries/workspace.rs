use std::fs;

use lsp_types::{DocumentChangeOperation, DocumentChanges, ResourceOp};
use rhai_syntax::TextSize;

use crate::Server;
use crate::protocol::rename_to_workspace_edit;
use crate::state::uri_from_path;
use crate::tests::queries::create_temp_workspace;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

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
