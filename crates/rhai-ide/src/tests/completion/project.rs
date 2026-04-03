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
        .find(|item| item.label == "echo" && item.source == CompletionItemSource::AutoImport)
        .expect("expected auto import echo completion");
    assert_eq!(echo.detail.as_deref(), Some("fun() -> string"));

    let value_offset = u32::try_from(text.rfind("va").expect("expected value prefix") + "va".len())
        .expect("offset");
    let value_completions = analysis.completions(FilePosition {
        file_id,
        offset: value_offset,
    });
    let value = value_completions
        .iter()
        .find(|item| item.label == "value" && item.source == CompletionItemSource::AutoImport)
        .expect("expected auto import value completion");
    assert_eq!(value.detail.as_deref(), Some("string"));
}

#[test]
fn project_completions_hide_bare_duplicates_when_auto_import_is_available() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        con
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    fn connect() {}
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
    let offset = u32::try_from(text.find("con").expect("expected completion prefix") + "con".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert_eq!(
        completions
            .iter()
            .filter(|item| item.label == "connect")
            .map(|item| item.source)
            .collect::<Vec<_>>(),
        vec![CompletionItemSource::AutoImport]
    );
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

#[test]
fn completions_prioritize_visible_type_matches_from_expected_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param value string
                fn wants_string(value) {}

                fn run() {
                    let seed = 1;
                    let story = "Ada";
                    wants_string(s)
                }
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.rfind("wants_string(s)")
            .expect("expected completion prefix")
            + "wants_string(s".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let seed_index = completions
        .iter()
        .position(|item| item.label == "seed" && item.source == CompletionItemSource::Visible)
        .expect("expected seed completion");
    let story_index = completions
        .iter()
        .position(|item| item.label == "story" && item.source == CompletionItemSource::Visible)
        .expect("expected story completion");

    assert!(
        story_index < seed_index,
        "expected string-typed visible symbol to outrank int-typed one: {completions:?}"
    );
}

#[test]
fn completions_prioritize_project_type_matches_from_expected_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    /// @param value string
                    fn wants_string(value) {}

                    fn run() {
                        wants_string(st)
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    let stack = 1;
                    let story = "Ada";

                    export stack as stack;
                    export story as story;
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(
        text.rfind("wants_string(st)")
            .expect("expected completion prefix")
            + "wants_string(st".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let stack_index = completions
        .iter()
        .position(|item| item.label == "stack" && item.source == CompletionItemSource::AutoImport)
        .expect("expected stack completion");
    let story_index = completions
        .iter()
        .position(|item| item.label == "story" && item.source == CompletionItemSource::AutoImport)
        .expect("expected story completion");

    assert!(
        story_index < stack_index,
        "expected string-typed project symbol to outrank int-typed one: {completions:?}"
    );
}

#[test]
fn completions_prioritize_visible_callables_in_call_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn helper() {}

                fn run() {
                    let helium = 1;
                    he(
                }
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.rfind("he(").expect("expected completion prefix") + "he".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::Visible)
        .expect("expected helper completion");
    let helium_index = completions
        .iter()
        .position(|item| item.label == "helium" && item.source == CompletionItemSource::Visible)
        .expect("expected helium completion");

    assert!(
        helper_index < helium_index,
        "expected callable visible symbol to outrank non-callable one in call context: {completions:?}"
    );
}

#[test]
fn completions_prioritize_project_callables_in_call_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        he(
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn helper() {}
                    let helium = 1;

                    export helper as helper;
                    export helium as helium;
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.rfind("he(").expect("expected completion prefix") + "he".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::AutoImport)
        .expect("expected helper completion");
    let helium_index = completions
        .iter()
        .position(|item| item.label == "helium" && item.source == CompletionItemSource::AutoImport)
        .expect("expected helium completion");

    assert!(
        helper_index < helium_index,
        "expected callable project symbol to outrank non-callable one in call context: {completions:?}"
    );
}

#[test]
fn completions_prioritize_zero_arg_visible_callables_in_immediate_call_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn helper() {}
                fn helper_with(value) {}

                fn run() {
                    hel(
                }
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.rfind("hel(").expect("expected completion prefix") + "hel".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::Visible)
        .expect("expected helper completion");
    let helper_with_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_with" && item.source == CompletionItemSource::Visible
        })
        .expect("expected helper_with completion");

    assert!(
        helper_index < helper_with_index,
        "expected zero-arg callable to outrank arity-mismatched one in call context: {completions:?}"
    );
}

#[test]
fn completions_prioritize_zero_arg_project_callables_in_immediate_call_context() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        hel(
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn helper_with(value) {}

                    export helper as helper;
                    export helper_with as helper_with;
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.rfind("hel(").expect("expected completion prefix") + "hel".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::AutoImport)
        .expect("expected helper completion");
    let helper_with_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_with" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected helper_with completion");

    assert!(
        helper_index < helper_with_index,
        "expected zero-arg auto import callable to outrank arity-mismatched one in call context: {completions:?}"
    );
}

#[test]
fn completions_prioritize_matching_visible_callable_arity_when_arguments_exist() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn helper() {}
                fn helper_with(value) {}

                fn run() {
                    hel(1)
                }
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
    let offset =
        u32::try_from(text.rfind("hel(1)").expect("expected completion prefix") + "hel".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::Visible)
        .expect("expected helper completion");
    let helper_with_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_with" && item.source == CompletionItemSource::Visible
        })
        .expect("expected helper_with completion");

    assert!(
        helper_with_index < helper_index,
        "expected one-arg callable to outrank zero-arg one when one argument already exists: {completions:?}"
    );
}

#[test]
fn completions_prioritize_matching_project_callable_arity_when_arguments_exist() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        hel(1)
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn helper() {}
                    fn helper_with(value) {}

                    export helper as helper;
                    export helper_with as helper_with;
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
    let offset =
        u32::try_from(text.rfind("hel(1)").expect("expected completion prefix") + "hel".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_index = completions
        .iter()
        .position(|item| item.label == "helper" && item.source == CompletionItemSource::AutoImport)
        .expect("expected helper completion");
    let helper_with_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_with" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected helper_with completion");

    assert!(
        helper_with_index < helper_index,
        "expected one-arg project callable to outrank zero-arg one when one argument already exists: {completions:?}"
    );
}

#[test]
fn completions_prioritize_visible_callable_argument_type_matches() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param value int
                fn helper_int(value) {}
                /// @param value string
                fn helper_string(value) {}

                fn run() {
                    helper_("Ada")
                }
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
        text.rfind("helper_(\"Ada\")")
            .expect("expected completion prefix")
            + "helper_".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_int_index = completions
        .iter()
        .position(|item| item.label == "helper_int" && item.source == CompletionItemSource::Visible)
        .expect("expected helper_int completion");
    let helper_string_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_string" && item.source == CompletionItemSource::Visible
        })
        .expect("expected helper_string completion");

    assert!(
        helper_string_index < helper_int_index,
        "expected string-matching callable to outrank int-matching callable: {completions:?}"
    );
}

#[test]
fn completions_prioritize_project_callable_argument_type_matches() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        helper_("Ada")
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    /// @param value int
                    fn helper_int(value) {}
                    /// @param value string
                    fn helper_string(value) {}

                    export helper_int as helper_int;
                    export helper_string as helper_string;
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
        text.rfind("helper_(\"Ada\")")
            .expect("expected completion prefix")
            + "helper_".len(),
    )
    .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let helper_int_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_int" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected helper_int completion");
    let helper_string_index = completions
        .iter()
        .position(|item| {
            item.label == "helper_string" && item.source == CompletionItemSource::AutoImport
        })
        .expect("expected helper_string completion");

    assert!(
        helper_string_index < helper_int_index,
        "expected string-matching project callable to outrank int-matching callable: {completions:?}"
    );
}

#[test]
fn completions_prioritize_subsequence_name_matches() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "main.rhai".into(),
                text: r#"
                    fn run() {
                        shh
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "support.rhai".into(),
                text: r#"
                    fn shared_helper() {}
                    fn shell_history() {}

                    export shared_helper as shared_helper;
                    export shell_history as shell_history;
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
    let offset =
        u32::try_from(text.rfind("shh").expect("expected completion prefix") + "shh".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let shared_helper_index = completions
        .iter()
        .position(|item| {
            item.label == "shared_helper" && item.source == CompletionItemSource::Project
        })
        .expect("expected shared_helper completion");
    let shell_history_index = completions
        .iter()
        .position(|item| {
            item.label == "shell_history" && item.source == CompletionItemSource::Project
        })
        .expect("expected shell_history completion");

    assert!(
        shared_helper_index < shell_history_index,
        "expected smarter subsequence name match to outrank weaker contains match: {completions:?}"
    );
}

#[test]
fn completions_prioritize_nearer_visible_scope_bindings() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn run() {
                    let status_outer = 1;
                    {
                        let state_inner = 2;
                        st
                    }
                }
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
    let offset = u32::try_from(text.rfind("st").expect("expected completion prefix") + "st".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let state_inner_index = completions
        .iter()
        .position(|item| {
            item.label == "state_inner" && item.source == CompletionItemSource::Visible
        })
        .expect("expected state_inner completion");
    let status_outer_index = completions
        .iter()
        .position(|item| {
            item.label == "status_outer" && item.source == CompletionItemSource::Visible
        })
        .expect("expected status_outer completion");

    assert!(
        state_inner_index < status_outer_index,
        "expected nearer visible binding to outrank outer-scope binding: {completions:?}"
    );
}
