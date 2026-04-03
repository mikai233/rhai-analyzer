use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionInsertFormat, CompletionItemSource, FilePosition};

#[test]
fn postfix_templates_appear_after_dot_trigger() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = 42;
                value.
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
        text.find("value.")
            .expect("expected postfix completion target")
            + "value.".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "switch" && item.source == CompletionItemSource::Postfix),
        "expected switch postfix completion, got {completions:?}"
    );
}

#[test]
fn postfix_completion_uses_insert_replace_ranges_for_member_receiver_prefixes() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let student = #{ name: "mikai233" };
                let branch = student.name.s
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
        text.find("student.name.s")
            .expect("expected postfix completion target")
            + "student.name.s".len(),
    )
    .expect("offset");

    let switch = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "switch" && item.source == CompletionItemSource::Postfix)
        .expect("expected switch postfix completion");

    assert_eq!(switch.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(switch.filter_text.as_deref(), Some("switch"));
    assert_eq!(
        switch.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("switch student.name {\n    ${1:_} => {\n        $0\n    }\n}")
    );

    let text_edit = switch.text_edit.expect("expected text edit");
    let prefix_start = text
        .find("student.name.s")
        .expect("expected completion prefix start")
        + "student.name.".len();
    let receiver_start = text
        .find("student.name.s")
        .expect("expected receiver start");

    assert_eq!(
        text_edit.insert_range,
        Some(rhai_syntax::TextRange::new(
            rhai_syntax::TextSize::from(prefix_start as u32),
            rhai_syntax::TextSize::from(offset),
        ))
    );
    assert_eq!(
        text_edit.replace_range,
        rhai_syntax::TextRange::new(
            rhai_syntax::TextSize::from(receiver_start as u32),
            rhai_syntax::TextSize::from(offset),
        )
    );
}

#[test]
fn postfix_templates_include_not_and_let_expansions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = flag_value();
                value.n
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
        text.find("value.n")
            .expect("expected postfix completion target")
            + "value.n".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let not = completions
        .iter()
        .find(|item| item.label == "not" && item.source == CompletionItemSource::Postfix)
        .expect("expected not postfix completion");
    assert_eq!(
        not.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("!value$0")
    );

    let let_item = analysis
        .completions(FilePosition {
            file_id,
            offset: offset - 1,
        })
        .into_iter()
        .find(|item| item.label == "let" && item.source == CompletionItemSource::Postfix)
        .expect("expected let postfix completion");
    assert_eq!(
        let_item
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str()),
        Some("let ${1:value} = value;$0")
    );
}

#[test]
fn postfix_templates_include_fori_expansion() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let items = [1, 2, 3];
                items.fori
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
        text.find("items.fori")
            .expect("expected postfix completion target")
            + "items.fori".len(),
    )
    .expect("offset");

    let fori = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "fori" && item.source == CompletionItemSource::Postfix)
        .expect("expected fori postfix completion");

    assert_eq!(fori.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        fori.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("for (${1:item}, ${2:index}) in items {\n    $0\n}")
    );
}
