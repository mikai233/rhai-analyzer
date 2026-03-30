pub(crate) mod layout;
pub(crate) mod support;
pub(crate) mod syntax;
pub(crate) mod trivia;

use crate::{FormatOptions, FormatResult, RangeFormatResult};
use rhai_syntax::{AstNode, Root, TextRange, TextSize, parse_text};

use crate::formatter::layout::doc::Doc;
use crate::formatter::layout::render::{render_doc, render_doc_with_indent};
use crate::formatter::support::utils::{minimal_changed_region, ranges_intersect};

pub fn format_text(text: &str, options: &FormatOptions) -> FormatResult {
    let parse = parse_text(text);
    if !parse.errors().is_empty() {
        return FormatResult {
            text: text.to_owned(),
            changed: false,
        };
    }

    let Some(root) = Root::cast(parse.root()) else {
        return FormatResult {
            text: text.to_owned(),
            changed: false,
        };
    };

    let formatter = Formatter {
        source: text,
        options,
    };
    let formatted = render_doc(&formatter.format_root(root), options);

    FormatResult {
        changed: formatted != text,
        text: formatted,
    }
}

pub fn format_range(
    text: &str,
    requested_range: TextRange,
    options: &FormatOptions,
) -> Option<RangeFormatResult> {
    let formatted = format_text(text, options);
    if !formatted.changed {
        return None;
    }

    let (start, end, replacement) = minimal_changed_region(text, &formatted.text)?;
    let changed_range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    if !ranges_intersect(changed_range, requested_range) {
        return None;
    }

    Some(RangeFormatResult {
        range: changed_range,
        text: replacement.to_owned(),
        changed: true,
    })
}

pub(crate) struct Formatter<'a> {
    source: &'a str,
    options: &'a FormatOptions,
}

impl Formatter<'_> {
    pub(crate) fn render_fragment(&self, doc: &Doc, base_indent: usize) -> String {
        render_doc_with_indent(doc, self.options, base_indent)
    }
}
