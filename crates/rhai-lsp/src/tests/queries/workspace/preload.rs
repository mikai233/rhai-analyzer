use std::fs;

use crate::Server;
use crate::state::uri_from_path;
use crate::tests::offset_in;
use crate::tests::queries::create_temp_workspace;

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
