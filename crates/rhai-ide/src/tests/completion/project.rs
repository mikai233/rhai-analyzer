use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, CompletionItemSource, FilePosition};

#[test]
fn completions_merge_visible_project_and_member_results() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    /// helper docs
                    /// @type fun() -> bool
                    fn helper() {}

                    fn run() {
                        let user = #{ name: "Ada" };
                        user.
                        helper();
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: "/// shared helper docs\nfn shared_helper() {}".to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: None,
    });

    let analysis = host.snapshot();
    let main = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let support = analysis
        .db
        .vfs()
        .file_id(Path::new("support.rhai"))
        .expect("expected support.rhai");
    assert_no_syntax_diagnostics(&analysis, main);
    assert_no_syntax_diagnostics(&analysis, support);
    let main_text = analysis.db.file_text(main).expect("expected main text");

    let helper_offset = u32::try_from(main_text.find("helper();").expect("expected helper call"))
        .expect("expected offset to fit");
    let helper_completions = analysis.completions(FilePosition {
        file_id: main,
        offset: helper_offset,
    });
    assert!(
        helper_completions
            .iter()
            .any(|item| { item.label == "helper" && item.source == CompletionItemSource::Visible })
    );
    assert!(helper_completions.iter().any(|item| {
        item.label == "shared_helper" && item.source == CompletionItemSource::Project
    }));
    let shared_helper = helper_completions
        .iter()
        .find(|item| item.label == "shared_helper" && item.source == CompletionItemSource::Project)
        .cloned()
        .expect("expected shared_helper completion");
    assert!(shared_helper.docs.is_none());
    let resolved_shared_helper = analysis.resolve_completion(shared_helper);
    assert_eq!(
        resolved_shared_helper.docs.as_deref(),
        Some("shared helper docs")
    );

    let member_offset = u32::try_from(main_text.find("user.").expect("expected member access"))
        .expect("expected offset to fit");
    let member_completions = analysis.completions(FilePosition {
        file_id: main,
        offset: member_offset,
    });
    assert!(
        member_completions
            .iter()
            .any(|item| { item.label == "name" && item.source == CompletionItemSource::Member })
    );
}
#[test]
fn imported_module_completions_use_original_module_name_as_origin() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    import "demo" as dd;

                    fn run() {
                        dd::sha
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "demo.rhai".into(),
                text: r#"
                    fn shared_helper() {}
                    export shared_helper as shared_helper;
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
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("sha").expect("expected completion prefix") + "sha".len())
        .expect("offset");

    let completion = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "shared_helper" && item.source == CompletionItemSource::Module)
        .expect("expected imported module completion");

    assert_eq!(completion.origin.as_deref(), Some("demo"));
}
#[test]
fn completions_prioritize_visible_prefix_matches_over_project_symbols() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn helper() {}

                    fn run() {
                        hel
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: "fn helpful_tool() {}".to_owned(),
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
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("hel").expect("expected completion prefix") + "hel".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper")
        .expect("expected helper completion");
    let helpful_tool_index = completions
        .iter()
        .position(|item| item.label == "helpful_tool")
        .expect("expected helpful_tool completion");

    assert!(
        helper_index < helpful_tool_index,
        "expected visible prefix match to outrank project symbol: {completions:?}"
    );
}
#[test]
fn project_completions_fall_back_to_inferred_symbol_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        ec;
                        va;
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn echo() {
                        "hello"
                    }

                    let value = "hello";

                    export echo as echo;
                    export value as value;
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
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    let echo_offset =
        u32::try_from(text.find("ec").expect("expected echo prefix") + "ec".len()).expect("offset");
    let echo_completions = analysis.completions(FilePosition {
        file_id,
        offset: echo_offset,
    });
    let echo = echo_completions
        .iter()
        .find(|item| item.label == "echo" && item.source == CompletionItemSource::Project)
        .expect("expected project echo completion");
    assert_eq!(echo.detail.as_deref(), Some("fun() -> string"));

    let value_offset = u32::try_from(text.rfind("va").expect("expected value prefix") + "va".len())
        .expect("offset");
    let value_completions = analysis.completions(FilePosition {
        file_id,
        offset: value_offset,
    });
    let value = value_completions
        .iter()
        .find(|item| item.label == "value" && item.source == CompletionItemSource::Project)
        .expect("expected project value completion");
    assert_eq!(value.detail.as_deref(), Some("string"));
}
