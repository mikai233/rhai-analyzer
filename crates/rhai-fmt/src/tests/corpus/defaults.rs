use crate::tests::corpus::support::{
    COMMENT_HEAVY_REPORT, CONFIG_HEAVY_WORKSPACE, DIAGNOSTICS_PIPELINE, TASK_RUNNER,
    WORKSPACE_BOOTSTRAP, assert_default_snapshot,
};

#[test]
fn corpus_case_task_runner_snapshot_stays_stable() {
    assert_default_snapshot(&TASK_RUNNER);
}

#[test]
fn corpus_case_workspace_bootstrap_snapshot_stays_stable() {
    assert_default_snapshot(&WORKSPACE_BOOTSTRAP);
}

#[test]
fn corpus_case_diagnostics_pipeline_snapshot_stays_stable() {
    assert_default_snapshot(&DIAGNOSTICS_PIPELINE);
}

#[test]
fn corpus_case_comment_heavy_report_snapshot_stays_stable() {
    assert_default_snapshot(&COMMENT_HEAVY_REPORT);
}

#[test]
fn corpus_case_config_heavy_workspace_snapshot_stays_stable() {
    assert_default_snapshot(&CONFIG_HEAVY_WORKSPACE);
}
