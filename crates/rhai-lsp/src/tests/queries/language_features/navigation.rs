use lsp_types::{FoldingRangeKind, Position};
use rhai_syntax::TextSize;

use crate::Server;
use crate::tests::{assert_valid_rhai_syntax, file_url, offset_in};

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
