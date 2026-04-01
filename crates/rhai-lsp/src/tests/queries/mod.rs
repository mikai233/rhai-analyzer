use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use lsp_types::FormattingOptions;

pub(crate) mod capabilities;
pub(crate) mod code_actions;
pub(crate) mod formatting;
pub(crate) mod language_features;
pub(crate) mod workspace;

pub(crate) fn create_temp_workspace(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("expected system time")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("rhai-analyzer-{prefix}-{unique}"));
    fs::create_dir_all(&workspace).expect("expected temporary workspace directory");
    workspace
}

pub(crate) fn default_formatting_options() -> FormattingOptions {
    FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..FormattingOptions::default()
    }
}
