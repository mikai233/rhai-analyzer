use std::path::PathBuf;

use lsp_types::Uri;
use rhai_syntax::parse_text;

pub(crate) mod code_actions;
pub(crate) mod diagnostics;

pub(crate) fn file_url(path: &str) -> Uri {
    let absolute = absolute_test_path(path);
    let raw = format!("file:///{}", absolute.display()).replace('\\', "/");
    raw.parse::<Uri>().expect("expected file URI")
}

pub(crate) fn absolute_test_path(path: &str) -> PathBuf {
    std::env::current_dir()
        .expect("expected current dir")
        .join(path)
}

pub(crate) fn offset_in(text: &str, needle: &str) -> u32 {
    u32::try_from(text.find(needle).expect("expected needle")).expect("expected offset")
}

pub(crate) fn assert_valid_rhai_syntax(text: &str) {
    let parse = parse_text(text);
    assert!(
        parse.errors().is_empty(),
        "expected valid Rhai syntax, got errors: {:?}",
        parse.errors()
    );
}
