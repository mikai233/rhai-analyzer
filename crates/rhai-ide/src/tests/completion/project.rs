use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
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

    let member_offset =
        u32::try_from(main_text.find("user.").expect("expected member access") + "user.".len())
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
    assert!(
        !member_completions
            .iter()
            .any(|item| item.label == "helper" && item.source == CompletionItemSource::Visible),
        "member completion should not mix visible symbols: {member_completions:?}"
    );
    assert!(
        !member_completions
            .iter()
            .any(|item| item.label == "shared_helper"
                && item.source == CompletionItemSource::Project),
        "member completion should not mix project symbols: {member_completions:?}"
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

#[test]
fn global_module_completions_expose_file_constants_and_project_modules() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// docs for answer
                const ANSWER = 42;

                fn run() {
                    global::
                    global::math::a
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            modules: [(
                "math".to_owned(),
                rhai_project::ModuleSpec {
                    docs: Some("Math helpers".to_owned()),
                    functions: [(
                        "add".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(int, int) -> int".to_owned(),
                            return_type: None,
                            docs: Some("Add numbers".to_owned()),
                        }],
                    )]
                    .into_iter()
                    .collect(),
                    constants: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            ..ProjectConfig::default()
        }),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    let global_root_offset =
        u32::try_from(text.find("global::").expect("expected global path") + "global::".len())
            .expect("offset");
    let global_root = analysis.completions(FilePosition {
        file_id,
        offset: global_root_offset,
    });
    assert!(
        global_root
            .iter()
            .any(|item| { item.label == "ANSWER" && item.source == CompletionItemSource::Module })
    );
    assert!(
        global_root
            .iter()
            .any(|item| { item.label == "math" && item.source == CompletionItemSource::Module })
    );

    let module_offset = u32::try_from(
        text.find("global::math::a")
            .expect("expected nested global module completion")
            + "global::math::a".len(),
    )
    .expect("offset");
    let module_items = analysis.completions(FilePosition {
        file_id,
        offset: module_offset,
    });
    let add = module_items
        .iter()
        .find(|item| item.label == "add" && item.source == CompletionItemSource::Module)
        .expect("expected global module member completion");
    assert_eq!(add.origin.as_deref(), Some("global::math"));
}

#[test]
fn explicit_global_import_alias_overrides_automatic_global_module_completions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    import "env" as global;

                    const ANSWER = 42;

                    fn run() {
                        global::DE
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "env.rhai".into(),
                text: r#"
                    export const DEFAULTS = 1;
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
    let offset = u32::try_from(
        text.find("global::DE").expect("expected global alias path") + "global::DE".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "DEFAULTS" && item.source == CompletionItemSource::Module)
    );
    assert!(
        !completions.iter().any(|item| item.label == "ANSWER"),
        "explicit `global` alias should suppress automatic global members: {completions:?}"
    );
}

#[test]
fn member_completions_preserve_inferred_callable_field_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                export const DEFAULTS = #{
                    tag: || "hello world",
                };

                DEFAULTS.ta
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
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
    let offset = u32::try_from(
        text.find("DEFAULTS.ta")
            .expect("expected member completion prefix")
            + "DEFAULTS.ta".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let tag = completions
        .iter()
        .find(|item| item.label == "tag" && item.source == CompletionItemSource::Member)
        .expect("expected member tag completion");

    assert_eq!(tag.detail.as_deref(), Some("fun() -> string"));
}
