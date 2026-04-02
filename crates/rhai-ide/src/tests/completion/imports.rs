use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::{AnalysisHost, CompletionInsertFormat, CompletionItemSource, FilePosition};

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

#[test]
fn completions_include_auto_import_candidates_for_unresolved_workspace_exports() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { shared_to }".to_owned(),
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
        text.find("shared_to").expect("expected completion prefix") + "shared_to".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| {
            item.label == "shared_tools" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected auto import completion");

    assert_eq!(item.origin.as_deref(), Some("provider"));
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("provider::shared_tools")
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .map(|edit| edit.additional_edits.len()),
        Some(1)
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .and_then(|edit| edit.additional_edits.first())
            .map(|edit| edit.new_text.as_str()),
        Some("import \"provider\" as provider;\n\n")
    );
}

#[test]
fn auto_import_completions_reuse_existing_module_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() { shared_to }
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
        text.find("shared_to").expect("expected completion prefix") + "shared_to".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| {
            item.label == "shared_tools" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected auto import completion");

    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("tools::shared_tools")
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .map(|edit| edit.additional_edits.len()),
        Some(0)
    );
}

#[test]
fn auto_import_completions_allocate_non_conflicting_aliases() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "tools.rhai".into(),
                text: "export const VALUE = 1;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "tools" as provider;

                    fn run() { shared_to }
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
        text.find("shared_to").expect("expected completion prefix") + "shared_to".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| {
            item.label == "shared_tools" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected auto import completion");

    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("provider_1::shared_tools")
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .and_then(|edit| edit.additional_edits.first())
            .map(|edit| edit.new_text.as_str()),
        Some("\nimport \"provider\" as provider_1;")
    );
}

#[test]
fn module_path_completions_include_auto_import_candidates_for_unimported_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { provider::shared_to }".to_owned(),
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
        text.find("provider::shared_to")
            .expect("expected completion prefix")
            + "provider::shared_to".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| {
            item.label == "shared_tools" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected module-path auto import completion");

    assert_eq!(item.origin.as_deref(), Some("provider"));
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("shared_tools")
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .and_then(|edit| edit.additional_edits.first())
            .map(|edit| edit.new_text.as_str()),
        Some("import \"provider\" as provider;\n\n")
    );
}

#[test]
fn global_module_path_completions_include_auto_import_candidates() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: "let helper = 1; export helper as shared_tools;".to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { global::shared_to }".to_owned(),
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
        text.find("global::shared_to")
            .expect("expected completion prefix")
            + "global::shared_to".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| {
            item.label == "shared_tools" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected global auto import completion");

    assert_eq!(item.origin.as_deref(), Some("provider"));
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("shared_tools")
    );
    assert_eq!(
        item.text_edit
            .as_ref()
            .and_then(|edit| edit.additional_edits.first())
            .map(|edit| edit.new_text.as_str()),
        Some("import \"provider\" as global;\n\n")
    );
}

#[test]
fn auto_import_callable_completions_insert_module_qualified_snippets() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn connect() {}
                    export connect as connect;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: "fn run() { con }".to_owned(),
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
    let offset = u32::try_from(text.find("con").expect("expected completion prefix") + "con".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let item = completions
        .iter()
        .find(|item| item.label == "connect" && item.source == CompletionItemSource::AutoImport)
        .expect("expected auto import completion");

    assert_eq!(item.origin.as_deref(), Some("provider"));
    assert_eq!(item.detail.as_deref(), Some("fun() -> unknown"));
    assert_eq!(item.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        item.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("provider::connect()$0")
    );
}
