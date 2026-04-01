use crate::CompletionItemSource;
use crate::tests::completion::members::load_analysis;

#[test]
fn completions_fall_back_to_inferred_local_symbol_types() {
    let (analysis, file_id, text) = load_analysis(
        r#"
            fn echo(value) {
                value
            }

            fn run() {
                let result = echo(blob(10));
                res
            }
        "#,
    );

    let offset =
        u32::try_from(text.rfind("res").expect("expected completion target")).expect("offset");

    let completions = analysis.completions(crate::FilePosition { file_id, offset });
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
