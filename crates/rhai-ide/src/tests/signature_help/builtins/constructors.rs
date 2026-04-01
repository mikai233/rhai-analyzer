use crate::tests::signature_help::builtins::{load_analysis, signature_help_at};

#[test]
fn signature_help_returns_builtin_blob_overloads() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let _empty = blob();
                let _sized = blob(10);
                let _filled = blob(50, 42);
            }
        "#,
    );
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin blob call to avoid diagnostics, got {diagnostics:?}"
    );

    let offset = u32::try_from(text.find("42").expect("expected argument")).expect("offset");
    let help = signature_help_at(&analysis, file_id, offset);

    assert_eq!(help.active_parameter, 1);
    assert_eq!(help.active_signature, 2);
    assert_eq!(help.signatures.len(), 3);
    assert_eq!(help.signatures[0].label, "fn blob() -> blob");
    assert_eq!(help.signatures[1].label, "fn blob(int) -> blob");
    assert_eq!(help.signatures[2].label, "fn blob(int, int) -> blob");
}

#[test]
fn signature_help_returns_builtin_timestamp_and_fn_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn helper() {}

            fn run() {
                let _now = timestamp();
                let _callback = Fn("helper");
            }
        "#,
    );
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin timestamp/Fn calls to avoid diagnostics, got {diagnostics:?}"
    );

    let timestamp_offset =
        u32::try_from(text.find("timestamp").expect("expected timestamp call")).expect("offset");
    let timestamp_help = signature_help_at(&analysis, file_id, timestamp_offset);
    assert_eq!(timestamp_help.signatures.len(), 1);
    assert_eq!(
        timestamp_help.signatures[0].label,
        "fn timestamp() -> timestamp"
    );

    let fn_offset =
        u32::try_from(text.find("\"helper\"").expect("expected Fn argument")).expect("offset");
    let fn_help = signature_help_at(&analysis, file_id, fn_offset);
    assert_eq!(fn_help.active_parameter, 0);
    assert_eq!(fn_help.signatures.len(), 1);
    assert_eq!(fn_help.signatures[0].label, "fn Fn(string) -> Fn");
}

#[test]
fn signature_help_returns_builtin_eval_signature() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                let result = eval("40 + 2");
                result;
            }
        "#,
    );
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin eval call to avoid diagnostics, got {diagnostics:?}"
    );

    let offset =
        u32::try_from(text.find("\"40 + 2\"").expect("expected eval input")).expect("offset");
    let help = signature_help_at(&analysis, file_id, offset);

    assert_eq!(help.active_parameter, 0);
    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "fn eval(string) -> Dynamic");
}
