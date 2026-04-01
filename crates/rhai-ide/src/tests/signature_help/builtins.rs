use std::path::Path;

use rhai_db::ChangeSet;
use rhai_vfs::DocumentVersion;

use crate::tests::assert_no_syntax_diagnostics;
use crate::{AnalysisHost, FilePosition};

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
