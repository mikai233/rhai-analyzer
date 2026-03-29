use rhai_syntax::{TextRange, parse_text};

pub(crate) fn slice_range(source: &str, range: TextRange) -> &str {
    let start: u32 = range.start().into();
    let end: u32 = range.end().into();
    &source[start as usize..end as usize]
}

pub(crate) fn parse_valid(source: &str) -> rhai_syntax::Parse {
    let parse = parse_text(source);
    assert!(
        parse.errors().is_empty(),
        "expected valid Rhai syntax, got errors: {:?}",
        parse.errors()
    );
    parse
}

mod diagnostics;
mod functions;
mod lowering;
mod modules;
mod queries;
