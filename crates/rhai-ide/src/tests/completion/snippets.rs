use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, CompletionInsertFormat, CompletionItemSource, FilePosition};

fn assert_structured_builtin_docs(docs: &str, topic: &str) {
    assert!(!docs.trim().is_empty());
    assert!(docs.contains("## Usage"));
    assert!(docs.contains("## Examples"));
    assert!(docs.contains("## Official Rhai Reference"));
    assert!(docs.contains(topic));
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
    let text = analysis.db.file_text(file_id).expect("expected text");
    let offset = u32::try_from(text.find("pri").expect("expected builtin prefix") + "pri".len())
        .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let print = completions
        .iter()
        .find(|item| item.label == "print" && item.source == CompletionItemSource::Builtin)
        .expect("expected builtin print completion");
    assert_eq!(print.detail.as_deref(), Some("fun(any) -> ()"));
    let docs = print.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "print");
    assert_eq!(print.insert_format, CompletionInsertFormat::Snippet);
    assert_eq!(
        print.text_edit.as_ref().map(|edit| edit.new_text.as_str()),
        Some("print(${1:any})$0")
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
fn overload_function_completion_resolve_preserves_selected_signature_snippet() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            /// @return int
            fn do_something() {
                1
            }

            /// @param value int
            /// @return int
            fn do_something(value) {
                value
            }

            fn run() {
                do_
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
        u32::try_from(text.rfind("do_").expect("expected completion target") + "do_".len())
            .expect("offset");

    let completions = analysis.completions(FilePosition { file_id, offset });
    let overload = completions
        .into_iter()
        .find(|item| {
            item.label == "do_something"
                && item.source == CompletionItemSource::Visible
                && item.detail.as_deref() == Some("fun(int) -> int")
        })
        .expect("expected typed overload completion");

    assert_eq!(
        overload
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str()),
        Some("do_something(${1:value})$0")
    );

    let resolved = analysis.resolve_completion(overload);
    assert_eq!(resolved.detail.as_deref(), Some("fun(int) -> int"));
    assert_eq!(
        resolved
            .text_edit
            .as_ref()
            .map(|edit| edit.new_text.as_str()),
        Some("do_something(${1:value})$0")
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
fn member_completions_include_rich_builtin_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let text = "hello";
                text.to_
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
    let offset = u32::try_from(text.find("to_").expect("expected member completion target") + 3)
        .expect("offset");

    let to_blob = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "to_blob")
        .expect("expected to_blob completion");

    let docs = to_blob
        .docs
        .as_deref()
        .expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "to_blob");
}

#[test]
fn member_completions_include_rich_map_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ name: "Ada", active: true };
                user.get
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
    let offset = u32::try_from(
        text.rfind("get")
            .expect("expected member completion target")
            + 3,
    )
    .expect("offset");

    let get = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "get")
        .expect("expected get completion");

    let docs = get.docs.as_deref().expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "get");
}

#[test]
fn completions_include_builtin_sleep_function_with_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                sle
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
    let offset = u32::try_from(text.find("sle").expect("expected builtin prefix") + "sle".len())
        .expect("offset");

    let sleep = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "sleep" && item.source == CompletionItemSource::Builtin)
        .expect("expected builtin sleep completion");

    let docs = sleep.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "sleep");
}

#[test]
fn member_completions_include_new_array_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3, 4];
                values.dra
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
    let offset = u32::try_from(text.find("dra").expect("expected member completion target") + 3)
        .expect("offset");

    let drain = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "drain")
        .expect("expected drain completion");

    let docs = drain.docs.as_deref().expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "drain");
}

#[test]
fn member_completions_include_array_callback_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let values = [1, 2, 3, 4];
                values.fil
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
    let offset = u32::try_from(text.find("fil").expect("expected member completion target") + 3)
        .expect("offset");

    let filter = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "filter")
        .expect("expected filter completion");

    let docs = filter
        .docs
        .as_deref()
        .expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "filter");
}

#[test]
fn member_completions_include_new_float_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let ratio = 3.0;
                ratio.hyp
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
    let offset = u32::try_from(text.find("hyp").expect("expected member completion target") + 3)
        .expect("offset");

    let hypot = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "hypot")
        .expect("expected hypot completion");

    let docs = hypot.docs.as_deref().expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "hypot");
}

#[test]
fn member_completions_include_new_map_method_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let user = #{ name: "Ada", active: true };
                user.fil
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
    let offset = u32::try_from(text.find("fil").expect("expected member completion target") + 3)
        .expect("offset");

    let filter = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "filter")
        .expect("expected filter completion");

    let docs = filter
        .docs
        .as_deref()
        .expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "filter");
}

#[test]
fn completions_include_new_decimal_builtin_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                parse_dec
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
    let offset = u32::try_from(
        text.find("parse_dec")
            .expect("expected builtin completion target")
            + "parse_dec".len(),
    )
    .expect("offset");

    let parse_decimal = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "parse_decimal")
        .expect("expected parse_decimal completion");

    let docs = parse_decimal
        .docs
        .as_deref()
        .expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "parse_decimal");
}

#[test]
fn completions_include_dynamic_tag_builtin_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                ta
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
    let offset = u32::try_from(text.find("ta").expect("expected builtin completion target") + 2)
        .expect("offset");

    let tag = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "tag")
        .expect("expected tag completion");

    let docs = tag.docs.as_deref().expect("expected builtin docs");
    assert_structured_builtin_docs(docs, "tag");
}

#[test]
fn member_completions_include_new_int_conversion_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let value = 10;
                value.to_b
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
    let offset = u32::try_from(
        text.find("to_b")
            .expect("expected member completion target")
            + "to_b".len(),
    )
    .expect("offset");

    let to_binary = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "to_binary")
        .expect("expected to_binary completion");

    let docs = to_binary
        .docs
        .as_deref()
        .expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "to_binary");
}

#[test]
fn member_completions_include_bit_field_docs() {
    let mut host = AnalysisHost::default();
    host.apply_change(ChangeSet::single_file(
        "main.rhai",
        r#"
            fn run() {
                let flags = 0b1010;
                flags.get_
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
        text.find("get_")
            .expect("expected member completion target")
            + "get_".len(),
    )
    .expect("offset");

    let get_bit = analysis
        .completions(FilePosition { file_id, offset })
        .into_iter()
        .find(|item| item.label == "get_bit")
        .expect("expected get_bit completion");

    let docs = get_bit
        .docs
        .as_deref()
        .expect("expected builtin method docs");
    assert_structured_builtin_docs(docs, "get_bit");
}
