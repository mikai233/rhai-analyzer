use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, CompletionItemSource, FilePosition};

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
