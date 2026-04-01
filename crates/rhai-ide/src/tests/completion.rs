use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, CompletionInsertFormat, CompletionItemSource, FilePosition};

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
fn completions_include_builtin_string_members() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let text = "hello";
                text.
                helper();
            }

            fn helper() {}
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("text.").expect("expected string member access") + "text.".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "contains" && item.source == CompletionItemSource::Member)
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "is_shared" && item.source == CompletionItemSource::Member)
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "len" && item.source == CompletionItemSource::Member)
    );
}

#[test]
fn completions_include_builtin_array_members_for_incomplete_trailing_dot_access() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let a = [1, 2, 3];
                a.
                next();
            }

            fn next() {}
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
    let offset = u32::try_from(text.find("a.").expect("expected array member access") + "a.".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "len" && item.source == CompletionItemSource::Member),
        "expected array len member completion, got {completions:?}"
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "push" && item.source == CompletionItemSource::Member),
        "expected array push member completion, got {completions:?}"
    );
}

#[test]
fn completions_merge_object_fields_with_builtin_map_members() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ name: "Ada" };
                user.
                helper();
            }

            fn helper() {}
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("user.").expect("expected object member access") + "user.".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    assert!(
        completions
            .iter()
            .any(|item| item.label == "name" && item.source == CompletionItemSource::Member)
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "keys" && item.source == CompletionItemSource::Member)
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "values" && item.source == CompletionItemSource::Member)
    );
}

#[test]
fn completions_include_builtin_primitive_members() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let count = 1;
                count.
                next();

                let ratio = 3.14;
                ratio.
                next();

                let initial = 'a';
                initial.
                next();
            }

            fn next() {}
        "#,
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");

    let int_offset =
        u32::try_from(text.find("count.").expect("expected int member access") + "count.".len())
            .expect("offset");
    let int_completions = analysis.completions(FilePosition {
        file_id,
        offset: int_offset,
    });
    assert!(
        int_completions
            .iter()
            .any(|item| item.label == "is_odd" && item.source == CompletionItemSource::Member)
    );
    assert!(
        int_completions
            .iter()
            .any(|item| item.label == "to_float" && item.source == CompletionItemSource::Member)
    );

    let float_offset =
        u32::try_from(text.find("ratio.").expect("expected float member access") + "ratio.".len())
            .expect("offset");
    let float_completions = analysis.completions(FilePosition {
        file_id,
        offset: float_offset,
    });
    assert!(
        float_completions
            .iter()
            .any(|item| item.label == "floor" && item.source == CompletionItemSource::Member)
    );
    assert!(
        float_completions
            .iter()
            .any(|item| item.label == "to_int" && item.source == CompletionItemSource::Member)
    );

    let char_offset = u32::try_from(
        text.find("initial.").expect("expected char member access") + "initial.".len(),
    )
    .expect("offset");
    let char_completions = analysis.completions(FilePosition {
        file_id,
        offset: char_offset,
    });
    assert!(
        char_completions
            .iter()
            .any(|item| item.label == "to_upper" && item.source == CompletionItemSource::Member)
    );
    assert!(
        char_completions
            .iter()
            .any(|item| item.label == "to_int" && item.source == CompletionItemSource::Member)
    );
}

#[test]
fn completions_include_builtin_global_functions_with_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                pri
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
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("pri").expect("expected builtin prefix") + "pri".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let print = completions
        .iter()
        .find(|item| item.label == "print" && item.source == CompletionItemSource::Builtin)
        .expect("expected builtin print completion");
    assert_eq!(print.detail.as_deref(), Some("fun(any) -> ()"));
    assert_eq!(
        print.docs.as_deref(),
        Some("Print a value via the engine's print callback.")
    );
    assert_eq!(print.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        print.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("print(${1:any})$0")
    );
}

#[test]
fn completions_specialize_generic_host_method_details_from_receiver_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                fn run() {
                    /// @type Box<int>
                    let boxed = unknown_box;
                    boxed.
                    next();
                }

                fn next() {}
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Box<T>".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [
                        (
                            "peek".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun() -> T".to_owned(),
                                return_type: None,
                                docs: Some("Peek at the boxed value".to_owned()),
                            }],
                        ),
                        (
                            "unwrap_or".to_owned(),
                            vec![rhai_project::FunctionSpec {
                                signature: "fun(T) -> T".to_owned(),
                                return_type: None,
                                docs: Some("Return the boxed value or a fallback".to_owned()),
                            }],
                        ),
                    ]
                    .into_iter()
                    .collect(),
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
    let offset =
        u32::try_from(text.find("boxed.").expect("expected boxed member access") + "boxed.".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let peek = completions
        .iter()
        .find(|item| item.label == "peek" && item.source == CompletionItemSource::Member)
        .expect("expected peek completion");
    let unwrap_or = completions
        .iter()
        .find(|item| item.label == "unwrap_or" && item.source == CompletionItemSource::Member)
        .expect("expected unwrap_or completion");

    assert_eq!(peek.detail.as_deref(), Some("fun() -> int"));
    assert_eq!(unwrap_or.detail.as_deref(), Some("fun(int) -> int"));
}

#[test]
fn completions_fall_back_to_inferred_local_symbol_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn echo(value) {
                value
            }

            fn run() {
                let result = echo(blob(10));
                res
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
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.rfind("res").expect("expected completion target")).expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let result = completions
        .iter()
        .find(|item| item.label == "result" && item.source == CompletionItemSource::Visible)
        .expect("expected result completion");
    let echo = completions
        .iter()
        .find(|item| item.label == "echo" && item.source == CompletionItemSource::Visible)
        .expect("expected echo completion");

    assert_eq!(result.detail.as_deref(), Some("blob"));
    assert_eq!(echo.detail.as_deref(), Some("fun(blob) -> blob"));
    assert!(
        completions
            .iter()
            .position(|item| item.label == "result")
            .expect("expected result completion position")
            < completions
                .iter()
                .position(|item| item.label == "echo")
                .expect("expected echo completion position"),
        "expected variable prefix match to outrank function completion: {completions:?}"
    );
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
fn completion_resolve_populates_visible_symbol_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// local helper docs
            fn helper() {}

            fn run() {
                hel
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
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("hel").expect("expected completion prefix") + "hel".len())
        .expect("offset");

    let helper = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "helper" && item.source == CompletionItemSource::Visible)
        .expect("expected helper completion");
    assert!(helper.docs.is_none());

    let resolved = analysis.resolve_completion(helper);
    assert_eq!(resolved.docs.as_deref(), Some("local helper docs"));
}

#[test]
fn function_completions_insert_call_snippets_with_tabstops() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param left int
            /// @param right int
            /// @return int
            fn add(left, right) {
                left + right
            }

            fn run() {
                ad
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
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("ad").expect("expected completion prefix") + "ad".len())
        .expect("offset");

    let add = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "add")
        .expect("expected add completion");

    assert_eq!(add.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        add.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("add(${1:left}, ${2:right})$0")
    );
}

#[test]
fn member_completions_insert_call_snippets_with_tabstops() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.
                next();
            }

            fn next() {}
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
            .expect("expected member completion target")
            + "values.".len(),
    )
    .expect("offset");

    let push = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "push")
        .expect("expected push completion");

    assert_eq!(push.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        push.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("push(${1:any})$0")
    );
}

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
fn completions_do_not_panic_when_offset_lands_inside_multibyte_punctuation() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        "import \"demo\" as dd;\n\nlet q = 1.0 + 2。;\n\ndd::test();\nfn v() {}\n",
        DocumentVersion(1),
    ));

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("main.rhai"))
        .expect("expected main.rhai");
    let text = analysis.db.file_text(file_id).expect("expected text");
    let punctuation_offset =
        u32::try_from(text.find('。').expect("expected unicode punctuation") + 1).expect("offset");

    let _completions = analysis.completions(FilePosition {
        file_id,
        offset: punctuation_offset,
    });
}
