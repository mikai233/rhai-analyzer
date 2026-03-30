mod config;
mod formatter;
#[cfg(test)]
mod tests;
mod types;

pub use crate::config::{FormatMode, FormatOptions, IndentStyle};
pub use crate::formatter::{format_range, format_text};
pub use crate::types::{FormatResult, RangeFormatResult};
