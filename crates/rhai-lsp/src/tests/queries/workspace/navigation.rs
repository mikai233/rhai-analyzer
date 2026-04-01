use rhai_syntax::TextSize;

use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

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
