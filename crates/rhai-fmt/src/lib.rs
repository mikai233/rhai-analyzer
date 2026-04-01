mod cli;
mod config;
mod file_config;
mod formatter;
#[cfg(test)]
mod tests;
mod types;

pub use crate::cli::run_from_env;
pub use crate::config::{
    ContainerLayoutStyle, FormatMode, FormatOptions, ImportSortOrder, IndentStyle,
};
pub use crate::file_config::{
    LoadedFormatConfig, PartialFormatOptions, apply_partial_format_options,
    load_format_config_for_path,
};
pub use crate::formatter::{format_range, format_text};
pub use crate::types::{FormatResult, RangeFormatResult};
