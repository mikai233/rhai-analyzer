use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionInsertFormat, FilePosition};

#[test]
fn completions_include_module_qualified_import_members() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// helper docs
                    /// @param left int
                    /// @param right int
                    /// @return int
                    fn helper(left, right) {
                        left + right
                    }

                    export const VALUE = 1;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("tools::").expect("expected module path access") + "tools::".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper = completions
        .iter()
        .find(|item| item.label == "helper")
        .expect("expected helper completion");
    let value = completions
        .iter()
        .find(|item| item.label == "VALUE")
        .expect("expected VALUE completion");

    assert_eq!(helper.detail.as_deref(), Some("fun(int, int) -> int"));
    assert_eq!(helper.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        helper.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("helper(${1:left}, ${2:right})$0")
    );
    assert_eq!(value.detail.as_deref(), Some("int"));
}
#[test]
fn completions_filter_module_qualified_import_members_by_prefix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn world() {}
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::wo
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.find("tools::wo").expect("expected completion target") + "tools::wo".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let world_index = completions
        .iter()
        .position(|item| item.label == "world")
        .expect("expected world completion");
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper")
        .expect("expected helper completion");

    assert!(world_index < helper_index);
}
#[test]
fn completions_do_not_appear_for_single_colon_module_path_prefix() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "fn helper() {}".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools:
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("tools:").expect("expected completion target") + "tools:".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions.is_empty(),
        "expected no completions, got {completions:?}"
    );
}
