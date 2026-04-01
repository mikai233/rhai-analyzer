use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, IndentStyle};

const CONFIG_FILE_NAME: &str = "rhai.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialFormatOptions {
    pub indent_style: Option<IndentStyle>,
    pub indent_width: Option<usize>,
    pub max_line_length: Option<usize>,
    pub trailing_commas: Option<bool>,
    pub final_newline: Option<bool>,
    pub container_layout: Option<ContainerLayoutStyle>,
    pub import_sort_order: Option<ImportSortOrder>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedFormatConfig {
    pub path: PathBuf,
    pub options: PartialFormatOptions,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RhaiTomlConfig {
    #[serde(default)]
    formatting: FileFormattingConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct FileFormattingConfig {
    indent_style: Option<FileIndentStyle>,
    indent_width: Option<usize>,
    max_line_length: Option<usize>,
    trailing_commas: Option<bool>,
    final_newline: Option<bool>,
    container_layout: Option<FileContainerLayoutStyle>,
    import_sort_order: Option<FileImportSortOrder>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileIndentStyle {
    Spaces,
    Tabs,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileContainerLayoutStyle {
    Auto,
    PreferSingleLine,
    PreferMultiLine,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileImportSortOrder {
    Preserve,
    ModulePath,
}

pub fn load_format_config_for_path(path: &Path) -> Result<Option<LoadedFormatConfig>> {
    let search_start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    for directory in search_start.ancestors() {
        let config_path = directory.join(CONFIG_FILE_NAME);
        if !config_path.is_file() {
            continue;
        }

        let text = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read `{}`", config_path.display()))?;
        let parsed = toml::from_str::<RhaiTomlConfig>(&text)
            .with_context(|| format!("failed to parse `{}`", config_path.display()))?;
        return Ok(Some(LoadedFormatConfig {
            path: config_path,
            options: PartialFormatOptions::from(parsed.formatting),
        }));
    }

    Ok(None)
}

pub fn apply_partial_format_options(options: &mut FormatOptions, partial: &PartialFormatOptions) {
    if let Some(indent_style) = partial.indent_style {
        options.indent_style = indent_style;
    }
    if let Some(indent_width) = partial.indent_width {
        options.indent_width = indent_width;
    }
    if let Some(max_line_length) = partial.max_line_length {
        options.max_line_length = max_line_length;
    }
    if let Some(trailing_commas) = partial.trailing_commas {
        options.trailing_commas = trailing_commas;
    }
    if let Some(final_newline) = partial.final_newline {
        options.final_newline = final_newline;
    }
    if let Some(container_layout) = partial.container_layout {
        options.container_layout = container_layout;
    }
    if let Some(import_sort_order) = partial.import_sort_order {
        options.import_sort_order = import_sort_order;
    }
}

impl From<FileFormattingConfig> for PartialFormatOptions {
    fn from(value: FileFormattingConfig) -> Self {
        Self {
            indent_style: value.indent_style.map(|style| match style {
                FileIndentStyle::Spaces => IndentStyle::Spaces,
                FileIndentStyle::Tabs => IndentStyle::Tabs,
            }),
            indent_width: value.indent_width,
            max_line_length: value.max_line_length,
            trailing_commas: value.trailing_commas,
            final_newline: value.final_newline,
            container_layout: value.container_layout.map(|layout| match layout {
                FileContainerLayoutStyle::Auto => ContainerLayoutStyle::Auto,
                FileContainerLayoutStyle::PreferSingleLine => {
                    ContainerLayoutStyle::PreferSingleLine
                }
                FileContainerLayoutStyle::PreferMultiLine => ContainerLayoutStyle::PreferMultiLine,
            }),
            import_sort_order: value.import_sort_order.map(|order| match order {
                FileImportSortOrder::Preserve => ImportSortOrder::Preserve,
                FileImportSortOrder::ModulePath => ImportSortOrder::ModulePath,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{apply_partial_format_options, load_format_config_for_path};
    use crate::{ContainerLayoutStyle, FormatOptions, ImportSortOrder, IndentStyle};

    #[test]
    fn applies_partial_format_options() {
        let mut options = FormatOptions::default();
        let partial = super::PartialFormatOptions {
            indent_style: Some(IndentStyle::Tabs),
            indent_width: Some(2),
            max_line_length: Some(88),
            trailing_commas: Some(false),
            final_newline: Some(false),
            container_layout: Some(ContainerLayoutStyle::PreferMultiLine),
            import_sort_order: Some(ImportSortOrder::ModulePath),
        };

        apply_partial_format_options(&mut options, &partial);

        assert_eq!(options.indent_style, IndentStyle::Tabs);
        assert_eq!(options.indent_width, 2);
        assert_eq!(options.max_line_length, 88);
        assert!(!options.trailing_commas);
        assert!(!options.final_newline);
        assert_eq!(
            options.container_layout,
            ContainerLayoutStyle::PreferMultiLine
        );
        assert_eq!(options.import_sort_order, ImportSortOrder::ModulePath);
    }

    #[test]
    fn loads_nearest_rhai_toml_formatting_section() {
        let temp_root = std::env::temp_dir().join(format!(
            "rhai_fmt_config_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let nested = temp_root.join("src").join("nested");
        fs::create_dir_all(&nested).expect("create nested");
        fs::write(
            temp_root.join("rhai.toml"),
            r#"[formatting]
indent_style = "tabs"
indent_width = 2
max_line_length = 88
trailing_commas = false
final_newline = false
container_layout = "prefer_multi_line"
import_sort_order = "module_path"
"#,
        )
        .expect("write config");
        let file_path = nested.join("main.rhai");
        fs::write(&file_path, "let value = 1;").expect("write file");

        let loaded = load_format_config_for_path(&file_path)
            .expect("load config")
            .expect("expected config");

        assert_eq!(loaded.path, temp_root.join("rhai.toml"));
        assert_eq!(loaded.options.indent_style, Some(IndentStyle::Tabs));
        assert_eq!(loaded.options.indent_width, Some(2));
        assert_eq!(loaded.options.max_line_length, Some(88));
        assert_eq!(loaded.options.trailing_commas, Some(false));
        assert_eq!(loaded.options.final_newline, Some(false));
        assert_eq!(
            loaded.options.container_layout,
            Some(ContainerLayoutStyle::PreferMultiLine)
        );
        assert_eq!(
            loaded.options.import_sort_order,
            Some(ImportSortOrder::ModulePath)
        );

        fs::remove_dir_all(&temp_root).expect("cleanup temp root");
    }
}
