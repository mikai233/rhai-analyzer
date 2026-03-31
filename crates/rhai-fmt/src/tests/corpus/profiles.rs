use crate::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, IndentStyle};

use crate::tests::corpus::support::{
    COMMENT_HEAVY_REPORT, CONFIG_HEAVY_WORKSPACE, DIAGNOSTICS_PIPELINE, TASK_RUNNER,
    assert_profile_snapshot,
};

#[test]
fn corpus_case_task_runner_tabs_multiline_snapshot_stays_stable() {
    assert_profile_snapshot(
        &TASK_RUNNER,
        "tabs_multiline_sorted",
        include_str!("fixtures/profiles/tabs_multiline_sorted/task_runner.expected.rhai"),
        &tabs_multiline_sorted_options(),
    );
}

#[test]
fn corpus_case_comment_heavy_report_tabs_multiline_snapshot_stays_stable() {
    assert_profile_snapshot(
        &COMMENT_HEAVY_REPORT,
        "tabs_multiline_sorted",
        include_str!("fixtures/profiles/tabs_multiline_sorted/comment_heavy_report.expected.rhai"),
        &tabs_multiline_sorted_options(),
    );
}

#[test]
fn corpus_case_diagnostics_pipeline_compact_snapshot_stays_stable() {
    assert_profile_snapshot(
        &DIAGNOSTICS_PIPELINE,
        "compact_no_final_newline",
        include_str!(
            "fixtures/profiles/compact_no_final_newline/diagnostics_pipeline.expected.rhai"
        ),
        &compact_no_final_newline_options(),
    );
}

#[test]
fn corpus_case_config_heavy_workspace_compact_snapshot_stays_stable() {
    assert_profile_snapshot(
        &CONFIG_HEAVY_WORKSPACE,
        "compact_no_final_newline",
        include_str!(
            "fixtures/profiles/compact_no_final_newline/config_heavy_workspace.expected.rhai"
        ),
        &compact_no_final_newline_options(),
    );
}

fn tabs_multiline_sorted_options() -> FormatOptions {
    FormatOptions {
        indent_style: IndentStyle::Tabs,
        indent_width: 2,
        max_line_length: 80,
        trailing_commas: true,
        final_newline: true,
        container_layout: ContainerLayoutStyle::PreferMultiLine,
        import_sort_order: ImportSortOrder::ModulePath,
        ..FormatOptions::default()
    }
}

fn compact_no_final_newline_options() -> FormatOptions {
    FormatOptions {
        indent_style: IndentStyle::Spaces,
        indent_width: 2,
        max_line_length: 72,
        trailing_commas: false,
        final_newline: false,
        container_layout: ContainerLayoutStyle::PreferSingleLine,
        import_sort_order: ImportSortOrder::Preserve,
        ..FormatOptions::default()
    }
}
