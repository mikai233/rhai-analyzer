use crate::tests::signature_help::builtins::{load_analysis, signature_help_at};

#[test]
fn signature_help_returns_builtin_print_and_parse_signatures() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn run() {
                print(1);
                let _a = parse_int("42", 16);
                let _b = parse_float("3.14");
            }
        "#,
    );
    let diagnostics = analysis.diagnostics(file_id);
    assert!(
        diagnostics.is_empty(),
        "expected builtin print/parse calls to avoid diagnostics, got {diagnostics:?}"
    );

    let print_offset =
        u32::try_from(text.find("1").expect("expected print argument")).expect("offset");
    let print_help = signature_help_at(&analysis, file_id, print_offset);
    assert_eq!(print_help.signatures.len(), 1);
    assert_eq!(print_help.signatures[0].label, "fn print(any) -> ()");

    let parse_int_offset =
        u32::try_from(text.find("16").expect("expected parse_int radix")).expect("offset");
    let parse_int_help = signature_help_at(&analysis, file_id, parse_int_offset);
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
    let parse_float_help = signature_help_at(&analysis, file_id, parse_float_offset);
    assert_eq!(parse_float_help.signatures.len(), 1);
    assert_eq!(
        parse_float_help.signatures[0].label,
        "fn parse_float(string) -> float"
    );
}
