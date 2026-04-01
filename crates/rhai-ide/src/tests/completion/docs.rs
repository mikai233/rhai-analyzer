use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionItemSource, FilePosition};

#[test]
fn completions_suggest_doc_tags_inside_doc_comments() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @pa
            fn sample(value) {
                value
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
    let offset = u32::try_from(text.find("@pa").expect("expected doc tag prefix") + "@pa".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let param = completions
        .iter()
        .find(|item| item.label == "param")
        .expect("expected param completion");

    assert_eq!(param.source, CompletionItemSource::Builtin);
    assert_eq!(param.detail.as_deref(), Some("doc tag"));
    assert_eq!(param.filter_text.as_deref(), Some("param"));
    assert_eq!(
        param.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("param")
    );
}
#[test]
fn completions_suggest_doc_type_annotations_inside_param_tags() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value st
            fn sample(value) {
                value
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
    let offset =
        u32::try_from(text.find("st").expect("expected type prefix") + "st".len()).expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let string = completions
        .iter()
        .find(|item| item.label == "string")
        .expect("expected string type completion");

    assert_eq!(string.source, CompletionItemSource::Builtin);
    assert_eq!(string.detail.as_deref(), Some("type"));
    assert_eq!(string.filter_text.as_deref(), Some("string"));
    assert_eq!(
        string.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("string")
    );
}
#[test]
fn completions_suggest_doc_type_annotations_inside_return_tags_after_space() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @return 
            fn sample(value) {
                value
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
    let offset =
        u32::try_from(text.find("@return ").expect("expected return tag") + "@return ".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let string = completions
        .iter()
        .find(|item| item.label == "string")
        .expect("expected string type completion");

    assert_eq!(string.source, CompletionItemSource::Builtin);
    assert_eq!(string.detail.as_deref(), Some("type"));
    assert_eq!(
        string.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("string")
    );
}
