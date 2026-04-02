use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

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
fn completion_queries_include_postfix_templates_for_member_receiver_prefixes() {
    let mut server = Server::new();
    let uri = file_url("main.rhai");
    let text = r#"
        fn run() {
            let student = #{ name: "mikai233" };
            let branch = student.name.s
        }
    "#;

    assert_valid_rhai_syntax(text);
    server
        .open_document(uri.clone(), 1, text)
        .expect("expected open_document to succeed");

    let completions = server
        .completions(
            &uri,
            offset_in(text, "student.name.s") + "student.name.s".len() as u32,
        )
        .expect("expected completions query to succeed");
    let switch = completions
        .iter()
        .find(|item| item.label == "switch")
        .expect("expected switch postfix completion");

    let edit = switch.text_edit.as_ref().expect("expected text edit");
    assert_eq!(
        edit.new_text,
        "switch student.name {\n    ${1:_} => {\n        $0\n    }\n}"
    );
    assert!(edit.insert_range.is_some());
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
