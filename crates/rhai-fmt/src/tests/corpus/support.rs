use std::borrow::Cow;

use crate::tests::assert_parse_stable;
use crate::{FormatOptions, format_text};

pub(crate) struct CorpusCase {
    pub(crate) name: &'static str,
    pub(crate) source: &'static str,
    pub(crate) default_expected: &'static str,
}

pub(crate) const TASK_RUNNER: CorpusCase = CorpusCase {
    name: "task_runner",
    source: include_str!("fixtures/task_runner.input.rhai"),
    default_expected: include_str!("fixtures/task_runner.expected.rhai"),
};

pub(crate) const WORKSPACE_BOOTSTRAP: CorpusCase = CorpusCase {
    name: "workspace_bootstrap",
    source: include_str!("fixtures/workspace_bootstrap.input.rhai"),
    default_expected: include_str!("fixtures/workspace_bootstrap.expected.rhai"),
};

pub(crate) const DIAGNOSTICS_PIPELINE: CorpusCase = CorpusCase {
    name: "diagnostics_pipeline",
    source: include_str!("fixtures/diagnostics_pipeline.input.rhai"),
    default_expected: include_str!("fixtures/diagnostics_pipeline.expected.rhai"),
};

pub(crate) const COMMENT_HEAVY_REPORT: CorpusCase = CorpusCase {
    name: "comment_heavy_report",
    source: include_str!("fixtures/comment_heavy_report.input.rhai"),
    default_expected: include_str!("fixtures/comment_heavy_report.expected.rhai"),
};

pub(crate) const CONFIG_HEAVY_WORKSPACE: CorpusCase = CorpusCase {
    name: "config_heavy_workspace",
    source: include_str!("fixtures/config_heavy_workspace.input.rhai"),
    default_expected: include_str!("fixtures/config_heavy_workspace.expected.rhai"),
};

pub(crate) fn assert_default_snapshot(case: &CorpusCase) {
    assert_profile_snapshot(
        case,
        "default",
        case.default_expected,
        &FormatOptions::default(),
    );
}

pub(crate) fn assert_profile_snapshot(
    case: &CorpusCase,
    profile_name: &str,
    expected: &str,
    options: &FormatOptions,
) {
    let expected = normalized_expected_text(expected, options);
    let first = format_text(case.source, options);

    assert_eq!(
        first.text, expected,
        "unexpected corpus snapshot for `{}` under `{}`",
        case.name, profile_name
    );
    assert_parse_stable(&first.text);

    let second = format_text(&first.text, options);
    assert_eq!(
        second.text, first.text,
        "expected idempotent corpus snapshot for `{}` under `{}`",
        case.name, profile_name
    );
    assert!(
        !second.changed,
        "expected unchanged second pass for `{}` under `{}`",
        case.name, profile_name
    );
}

fn normalized_expected_text<'a>(expected: &'a str, options: &FormatOptions) -> Cow<'a, str> {
    if options.final_newline {
        Cow::Borrowed(expected)
    } else {
        Cow::Borrowed(expected.strip_suffix('\n').unwrap_or(expected))
    }
}
