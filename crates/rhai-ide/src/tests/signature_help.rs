use std::path::Path;

use rhai_db::ChangeSet;
use rhai_project::ProjectConfig;
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
fn signature_help_prefers_typed_script_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @param delta int
            /// @return int
            fn int.bump(delta) {
                this + delta
            }

            /// @param delta string
            /// @return string
            fn bump(delta) {
                delta
            }

            fn run() {
                let value = 1;
                value.bump(amount);
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
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected method signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn bump(delta: int) -> int");
    assert_eq!(help.signatures[0].file_id, Some(file_id));
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "delta");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}

#[test]
fn signature_help_supports_imported_global_typed_methods() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @param delta int
                    /// @return int
                    fn int.bump(delta) {
                        this + delta
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider";

                    fn run() {
                        let value = 1;
                        value.bump(amount);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected imported method signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn bump(delta: int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "delta");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}

#[test]
fn signature_help_supports_module_qualified_imported_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    /// @param value int
                    /// @return int
                    fn helper(value) {
                        value
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::helper(amount);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected imported module function signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(help.signatures[0].parameters[0].label, "value");
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}

#[test]
fn signature_help_supports_nested_module_qualified_imported_functions() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![
            rhai_db::FileChange {
                path: "sub.rhai".into(),
                text: r#"
                    /// @param value int
                    /// @return int
                    fn helper(value) {
                        value
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "provider.rhai".into(),
                text: r#"
                    import "sub" as sub;
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
            rhai_db::FileChange {
                path: "consumer.rhai".into(),
                text: r#"
                    import "provider" as tools;

                    fn run() {
                        tools::sub::helper(amount);
                    }
                "#
                .to_owned(),
                version: DocumentVersion(1),
            },
        ],
        removed_files: Vec::new(),
        project: Some(rhai_project::ProjectConfig::default()),
    });

    let analysis = host.snapshot();
    let file_id = analysis
        .db
        .vfs()
        .file_id(Path::new("consumer.rhai"))
        .expect("expected consumer.rhai");
    assert_no_syntax_diagnostics(&analysis, file_id);
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("amount").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected nested imported module function signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn helper(value: int) -> int");
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

#[test]
fn signature_help_returns_builtin_blob_overloads() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let _empty = blob();
                let _sized = blob(10);
                let _filled = blob(50, 42);
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin blob call to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("42").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.active_signature, 2);
    assert_eq!(help.signatures.len(), 3);
    assert_eq!(help.signatures[0].label, "fn blob() -> blob");
    assert_eq!(help.signatures[1].label, "fn blob(int) -> blob");
    assert_eq!(help.signatures[2].label, "fn blob(int, int) -> blob");
}

#[test]
fn signature_help_returns_builtin_timestamp_and_fn_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn helper() {}

            fn run() {
                let _now = timestamp();
                let _callback = Fn("helper");
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin timestamp/Fn calls to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");

    let timestamp_offset =
        u32::try_from(text.find("timestamp").expect("expected timestamp call")).expect("offset");
    let timestamp_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: timestamp_offset,
        })
        .expect("expected timestamp signature help");
    assert_eq!(timestamp_help.signatures.len(), 1);
    assert_eq!(
        timestamp_help.signatures[0].label,
        "fn timestamp() -> timestamp"
    );

    let fn_offset =
        u32::try_from(text.find("\"helper\"").expect("expected Fn argument")).expect("offset");
    let fn_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: fn_offset,
        })
        .expect("expected Fn signature help");
    assert_eq!(fn_help.active_parameter, 0);
    assert_eq!(fn_help.signatures.len(), 1);
    assert_eq!(fn_help.signatures[0].label, "fn Fn(string) -> Fn");
}

#[test]
fn signature_help_returns_builtin_introspection_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn int.bump(delta) { this + delta }

            fn run(value) {
                let _a = is_def_var("value");
                let _b = is_def_fn("int", "bump", 1);
                let _c = value.type_of();
                let _d = value.is_shared();
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin introspection calls to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");

    let is_def_fn_offset =
        u32::try_from(text.find("\"bump\", 1").expect("expected is_def_fn arg") + 8)
            .expect("offset");
    let is_def_fn_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: is_def_fn_offset,
        })
        .expect("expected is_def_fn signature help");
    assert_eq!(is_def_fn_help.active_parameter, 2);
    assert_eq!(is_def_fn_help.signatures.len(), 2);
    assert_eq!(
        is_def_fn_help.signatures[1].label,
        "fn is_def_fn(string, string, int) -> bool"
    );

    let type_of_offset =
        u32::try_from(text.find("type_of").expect("expected type_of call")).expect("offset");
    let type_of_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: type_of_offset,
        })
        .expect("expected type_of signature help");
    assert_eq!(type_of_help.signatures.len(), 1);
    assert_eq!(type_of_help.signatures[0].label, "fn type_of() -> string");

    let is_shared_offset =
        u32::try_from(text.find("is_shared").expect("expected is_shared call")).expect("offset");
    let is_shared_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: is_shared_offset,
        })
        .expect("expected is_shared signature help");
    assert_eq!(is_shared_help.signatures.len(), 1);
    assert_eq!(is_shared_help.signatures[0].label, "fn is_shared() -> bool");
}

#[test]
fn signature_help_returns_builtin_print_and_parse_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                print(1);
                let _a = parse_int("42", 16);
                let _b = parse_float("3.14");
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin print/parse calls to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");

    let print_offset =
        u32::try_from(text.find("1").expect("expected print argument")).expect("offset");
    let print_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: print_offset,
        })
        .expect("expected print signature help");
    assert_eq!(print_help.signatures.len(), 1);
    assert_eq!(print_help.signatures[0].label, "fn print(any) -> ()");

    let parse_int_offset =
        u32::try_from(text.find("16").expect("expected parse_int radix")).expect("offset");
    let parse_int_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: parse_int_offset,
        })
        .expect("expected parse_int signature help");
    assert_eq!(parse_int_help.active_parameter, 1);
    assert_eq!(parse_int_help.signatures.len(), 2);
    assert_eq!(
        parse_int_help.signatures[0].label,
        "fn parse_int(string) -> int"
    );
    assert_eq!(
        parse_int_help.signatures[1].label,
        "fn parse_int(string, int) -> int"
    );

    let parse_float_offset =
        u32::try_from(text.find("\"3.14\"").expect("expected parse_float input")).expect("offset");
    let parse_float_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: parse_float_offset,
        })
        .expect("expected parse_float signature help");
    assert_eq!(parse_float_help.signatures.len(), 1);
    assert_eq!(
        parse_float_help.signatures[0].label,
        "fn parse_float(string) -> float"
    );
}

#[test]
fn signature_help_returns_builtin_eval_signature() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let result = eval("40 + 2");
                result;
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
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin eval call to avoid diagnostics, got {diagnostics:?}"
    );
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset =
        u32::try_from(text.find("\"40 + 2\"").expect("expected eval input")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected eval signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn eval(string) -> Dynamic");
}

#[test]
fn signature_help_returns_builtin_string_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = "hello";
                value.contains("ell");
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
        u32::try_from(text.find("\"ell\"").expect("expected contains arg") + 2).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected builtin string method signature help");
    assert!(!help.signatures.is_empty());
    assert!(
        help.signatures
            .iter()
            .any(|signature| signature.label == "fn contains(string) -> bool")
    );
}

#[test]
fn signature_help_returns_builtin_array_and_map_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3];
                values.contains(item);

                let user = #{ name: "Ada" };
                user.contains(key);
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

    let array_offset =
        u32::try_from(text.find("item").expect("expected array argument")).expect("offset");
    let array_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: array_offset,
        })
        .expect("expected array signature help");
    assert_eq!(array_help.signatures[0].label, "fn contains(any) -> bool");

    let map_offset =
        u32::try_from(text.rfind("key").expect("expected map argument")).expect("offset");
    let map_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: map_offset,
        })
        .expect("expected map signature help");
    assert_eq!(map_help.signatures[0].label, "fn contains(string) -> bool");
}

#[test]
fn signature_help_returns_builtin_primitive_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let count = 1;
                count.max(limit);

                let ratio = 3.5;
                ratio.max(limit);
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

    let int_offset =
        u32::try_from(text.find("limit").expect("expected int max arg")).expect("offset");
    let int_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: int_offset,
        })
        .expect("expected int method signature help");
    assert_eq!(int_help.signatures[0].label, "fn max(int) -> int");

    let float_offset =
        u32::try_from(text.rfind("limit").expect("expected float max arg")).expect("offset");
    let float_help = analysis
        .signature_help(FilePosition {
            file_id,
            offset: float_offset,
        })
        .expect("expected float method signature help");
    assert_eq!(
        float_help.signatures[0].label,
        "fn max(int | float) -> float"
    );
}

#[test]
fn signature_help_returns_host_method_signatures() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param widget Widget
                fn run(widget) {
                    widget.open("home");
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "open".to_owned(),
                        vec![
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by route".to_owned()),
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(int) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by id".to_owned()),
                            },
                        ],
                    )]
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
    let offset = u32::try_from(text.find("\"home\"").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.active_signature, 0);
    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.signatures[0].label, "fn open(string) -> bool");
    assert_eq!(help.signatures[1].label, "fn open(int) -> bool");
    assert_eq!(help.signatures[0].docs.as_deref(), Some("Open by route"));
    assert_eq!(help.signatures[1].docs.as_deref(), Some("Open by id"));
}

#[test]
fn signature_help_prefers_host_method_overload_matching_argument_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param widget Widget
                fn run(widget) {
                    widget.open("home");
                }
            "#
            .to_owned(),
            version: DocumentVersion(1),
        }],
        removed_files: Vec::new(),
        project: Some(ProjectConfig {
            types: [(
                "Widget".to_owned(),
                rhai_project::TypeSpec {
                    docs: None,
                    methods: [(
                        "open".to_owned(),
                        vec![
                            rhai_project::FunctionSpec {
                                signature: "fun(int) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by id".to_owned()),
                            },
                            rhai_project::FunctionSpec {
                                signature: "fun(string) -> bool".to_owned(),
                                return_type: None,
                                docs: Some("Open by route".to_owned()),
                            },
                        ],
                    )]
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
    let offset = u32::try_from(text.find("\"home\"").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.active_signature, 1);
    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.signatures[0].label, "fn open(int) -> bool");
    assert_eq!(help.signatures[1].label, "fn open(string) -> bool");
}

#[test]
fn signature_help_specializes_generic_host_method_signatures_from_receiver_types() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet {
        files: vec![rhai_db::FileChange {
            path: "main.rhai".into(),
            text: r#"
                /// @param boxed Box<int>
                fn run(boxed) {
                    boxed.unwrap_or(value);
                }
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
                    methods: [(
                        "unwrap_or".to_owned(),
                        vec![rhai_project::FunctionSpec {
                            signature: "fun(T) -> T".to_owned(),
                            return_type: None,
                            docs: Some("Return the boxed value or a fallback".to_owned()),
                        }],
                    )]
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
    let offset = u32::try_from(text.find("value").expect("expected argument")).expect("offset");

    let help = analysis
        .signature_help(FilePosition { file_id, offset })
        .expect("expected signature help");

    assert_eq!(help.active_signature, 0);
    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn unwrap_or(int) -> int");
    assert_eq!(help.signatures[0].parameters.len(), 1);
    assert_eq!(
        help.signatures[0].parameters[0].annotation.as_deref(),
        Some("int")
    );
}
