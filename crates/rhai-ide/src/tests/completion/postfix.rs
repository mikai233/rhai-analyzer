use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionInsertFormat, CompletionItemSource, FilePosition};

#[test]
fn postfix_for_completion_rewrites_receiver_expression_into_loop_snippet() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.for
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("values.for")
            .expect("expected postfix completion target")
            + "values.for".len(),
    )
    .expect("offset");

    let item = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "for" && item.source == CompletionItemSource::Postfix)
        .expect("expected postfix for completion");

    assert_eq!(item.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(item.filter_text.as_deref(), Some("values.for"));
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("for ${1:item} in values {\n    $0\n}")
    );
}
#[test]
fn postfix_for_completion_appears_when_typing_a_suffix_prefix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.f
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("values.f")
            .expect("expected postfix completion target")
            + "values.f".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| item.label == "for" && item.source == CompletionItemSource::Postfix)
        .expect("expected postfix for completion while typing a suffix prefix");
    assert_eq!(item.filter_text.as_deref(), Some("values.for"));
}
#[test]
fn postfix_templates_do_not_appear_before_typing_a_suffix_prefix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("values.")
            .expect("expected postfix completion target")
            + "values.".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .all(|item| item.source != CompletionItemSource::Postfix),
        "expected postfix templates to stay hidden until a suffix prefix is typed, got {completions:?}"
    );
}
#[test]
fn dot_trigger_positions_still_hide_postfix_templates() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("values.").expect("expected dot trigger position") + "values".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .all(|item| item.source != CompletionItemSource::Postfix),
        "expected dot-trigger completions to hide postfix templates until a suffix prefix is typed, got {completions:?}"
    );
}
#[test]
fn postfix_switch_completion_rewrites_receiver_expression_into_switch_snippet() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = 42;
                value.switch
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("value.switch")
            .expect("expected postfix completion target")
            + "value.switch".len(),
    )
    .expect("offset");

    let item = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "switch" && item.source == CompletionItemSource::Postfix)
        .expect("expected postfix switch completion");

    assert_eq!(item.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(item.filter_text.as_deref(), Some("value.switch"));
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("switch value {\n    ${1:_} => {\n        $0\n    }\n}")
    );
}
#[test]
fn postfix_if_and_return_completions_expand_to_expected_snippets() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = ready;
                value.if
                value.return
            }
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");

    let if_offset = u32::try_from(
        text.find("value.if")
            .expect("expected postfix completion target")
            + "value.if".len(),
    )
    .expect("offset");
    let if_item = analysis
        .completions(FilePosition {
            file_id,
            offset: if_offset,
        })
        .into_iter()
        .find(|item| item.label == "if" && item.source == CompletionItemSource::Postfix)
        .expect("expected postfix if completion");
    assert_eq!(
        if_item
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str()),
        Some("if value {\n    $0\n}")
    );

    let return_offset = u32::try_from(
        text.find("value.return")
            .expect("expected postfix completion target")
            + "value.return".len(),
    )
    .expect("offset");
    let return_item = analysis
        .completions(FilePosition {
            file_id,
            offset: return_offset,
        })
        .into_iter()
        .find(|item| item.label == "return" && item.source == CompletionItemSource::Postfix)
        .expect("expected postfix return completion");
    assert_eq!(
        return_item
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str()),
        Some("return value;$0")
    );
}
