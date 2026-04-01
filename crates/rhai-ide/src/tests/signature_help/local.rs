use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

#[test]
fn signature_help_returns_local_function_signature() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param left int
            /// @param right string
            /// @return bool
            fn check(left, right) {
                true
            }

            fn run() {
                check(1, value);
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
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn check(left: int, right: string) -> bool"
    );
    assert_eq!(help.signatures[0].file_id, Some(file_id));
    assert_eq!(help.signatures[0].parameters.len(), 2);
    assert_eq!(help.signatures[0].parameters[0].label, "left");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
    assert_eq!(help.signatures[0].parameters[1].label, "right");
    assert_eq!(
        help.signatures[0].parameters[1].annotation.as_deref(),
        Some("string")
    );
}
#[test]
fn signature_help_selects_local_function_overload_by_arity() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @return int
            fn do_something() {
                1
            }

            /// @param value int
            /// @return string
            fn do_something(value) {
                value.to_string()
            }

            fn run() {
                do_something(arg);
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
    let offset = u32::try_from(text.find("arg").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn do_something(value: int) -> string"
    );
}
#[test]
fn signature_help_selects_local_function_overload_by_argument_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value int
            /// @return int
            fn do_something(value) {
                value
            }

            /// @param value string
            /// @return string
            fn do_something(value) {
                value
            }

            fn run() {
                do_something("hello");
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
        u32::try_from(text.find("\"hello\"").expect("expected string argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(
        help.signatures[0].label,
        "fn do_something(value: string) -> string"
    );
}
#[test]
fn signature_help_uses_caller_scope_call_targets() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param value string
            fn helper(value) {
                value
            }

            fn run() {
                call!(helper, "home");
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
    let helper_offset =
        u32::try_from(text.find("helper,").expect("expected dispatch target")).expect("offset");
    let arg_offset = u32::try_from(
        text.find("\"home\"")
            .expect("expected caller-scope argument"),
    )
    .expect("offset");

    assert!(
        analysis
            .signature_help(FilePosition {
                file_id,
                offset: helper_offset,
            })
            .is_none()
    );

    let help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: arg_offset,
        })
        .expect("expected caller-scope signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: string)");
}
#[test]
fn signature_help_is_not_returned_for_imported_module_alias_invocations() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    let helper = 1;
                    export helper;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools(1, value);
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
    let consumer = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, consumer);
    let text = analysis
        .db
        .file_text(consumer)
        .expect("expected consumer text");
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    assert!(
        analysis
            .signature_help(FilePosition {
                file_id: consumer,
                offset,
            })
            .is_none()
    );
}
