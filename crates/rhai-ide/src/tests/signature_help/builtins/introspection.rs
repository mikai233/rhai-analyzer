use crate::tests::signature_help::builtins::{load_analysis, signature_help_at};

#[test]
fn signature_help_returns_builtin_introspection_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn int.bump(delta) { this + delta }

            fn run(value) {
                let _a = is_def_var("value");
                let _b = is_def_fn("int", "bump", 1);
                let _c = value.type_of();
                let _d = value.is_shared();
            }
        "#,
    );
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin introspection calls to avoid diagnostics, got {diagnostics:?}"
    );

    let is_def_fn_offset =
        u32::try_from(text.find("\"bump\", 1").expect("expected is_def_fn arg") + 8)
            .expect("offset");
    let is_def_fn_help = signature_help_at(&analysis, file_id, is_def_fn_offset);
    assert_eq!(is_def_fn_help.active_parameter, 2);
    assert_eq!(is_def_fn_help.signatures.len(), 2);
    assert_eq!(
        is_def_fn_help.signatures[1].label,
        "fn is_def_fn(string, string, int) -> bool"
    );

    let type_of_offset =
        u32::try_from(text.find("type_of").expect("expected type_of call")).expect("offset");
    let type_of_help = signature_help_at(&analysis, file_id, type_of_offset);
    assert_eq!(type_of_help.signatures.len(), 1);
    assert_eq!(type_of_help.signatures[0].label, "fn type_of() -> string");

    let is_shared_offset =
        u32::try_from(text.find("is_shared").expect("expected is_shared call")).expect("offset");
    let is_shared_help = signature_help_at(&analysis, file_id, is_shared_offset);
    assert_eq!(is_shared_help.signatures.len(), 1);
    assert_eq!(is_shared_help.signatures[0].label, "fn is_shared() -> bool");
}
