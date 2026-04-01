use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Parser, ValueEnum};

use crate::{
    ContainerLayoutStyle, FormatOptions, ImportSortOrder, IndentStyle, PartialFormatOptions,
    apply_partial_format_options, format_text, load_format_config_for_path,
};
use rhai_syntax::parse_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliIndentStyle {
    Spaces,
    Tabs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliContainerLayoutStyle {
    Auto,
    PreferSingleLine,
    PreferMultiLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliImportSortOrder {
    Preserve,
    ModulePath,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "rhai-fmt")]
#[command(about = "Format Rhai files with the shared rhai-fmt engine")]
struct Cli {
    /// Files or directories to format. Directories are searched recursively for .rhai files.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Check whether files are already formatted without writing changes.
    #[arg(long)]
    check: bool,

    #[arg(long, value_enum)]
    indent_style: Option<CliIndentStyle>,

    #[arg(long)]
    indent_width: Option<usize>,

    #[arg(long)]
    max_line_length: Option<usize>,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_trailing_commas")]
    trailing_commas: bool,

    #[arg(long = "no-trailing-commas", action = ArgAction::SetTrue)]
    no_trailing_commas: bool,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_final_newline")]
    final_newline: bool,

    #[arg(long = "no-final-newline", action = ArgAction::SetTrue)]
    no_final_newline: bool,

    #[arg(long, value_enum)]
    container_layout: Option<CliContainerLayoutStyle>,

    #[arg(long, value_enum)]
    import_sort_order: Option<CliImportSortOrder>,
}

#[derive(Debug, Default)]
struct RunSummary {
    scanned_files: usize,
    changed_files: usize,
    syntax_error_files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileFormatOutcome {
    Unchanged,
    Changed,
    SyntaxErrors,
}

pub fn run_from_env() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(summary) => {
            if summary.syntax_error_files > 0 {
                std::process::exit(2);
            }
            if summary.changed_files > 0 {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::exit(2);
        }
    }
}

fn run(cli: Cli) -> Result<RunSummary> {
    let overrides = cli.format_overrides();
    let input_paths = if cli.paths.is_empty() {
        vec![std::env::current_dir().context("failed to resolve current directory")?]
    } else {
        cli.paths
    };
    let files = collect_rhai_files(&input_paths)?;
    if files.is_empty() {
        bail!("no .rhai files found");
    }

    let mut summary = RunSummary {
        scanned_files: files.len(),
        ..RunSummary::default()
    };

    for path in files {
        match format_file(&path, &overrides, cli.check)? {
            FileFormatOutcome::Unchanged => {}
            FileFormatOutcome::Changed => {
                summary.changed_files += 1;
                if cli.check {
                    println!("would reformat {}", path.display());
                } else {
                    println!("reformatted {}", path.display());
                }
            }
            FileFormatOutcome::SyntaxErrors => {
                summary.syntax_error_files += 1;
                eprintln!("cannot format {}: file has syntax errors", path.display());
            }
        }
    }

    if cli.check {
        println!(
            "checked {} file(s), {} would change, {} skipped for syntax errors",
            summary.scanned_files, summary.changed_files, summary.syntax_error_files
        );
    } else {
        println!(
            "formatted {} file(s), {} changed, {} skipped for syntax errors",
            summary.scanned_files, summary.changed_files, summary.syntax_error_files
        );
    }

    Ok(summary)
}

fn format_file(
    path: &Path,
    overrides: &PartialFormatOptions,
    check_only: bool,
) -> Result<FileFormatOutcome> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let parse = parse_text(&source);
    if !parse.errors().is_empty() {
        return Ok(FileFormatOutcome::SyntaxErrors);
    }

    let mut options = FormatOptions::default();
    if let Some(config) = load_format_config_for_path(path)? {
        apply_partial_format_options(&mut options, &config.options);
    }
    apply_partial_format_options(&mut options, overrides);

    let formatted = format_text(&source, &options);
    if !formatted.changed {
        return Ok(FileFormatOutcome::Unchanged);
    }

    if !check_only {
        fs::write(path, formatted.text)
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }

    Ok(FileFormatOutcome::Changed)
}

fn collect_rhai_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = BTreeSet::<PathBuf>::new();
    for path in paths {
        collect_rhai_files_from_path(path, &mut files)?;
    }
    Ok(files.into_iter().collect())
}

fn collect_rhai_files_from_path(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for `{}`", path.display()))?;
    if metadata.is_file() {
        if is_rhai_file(path) {
            files.insert(path.to_path_buf());
        }
        return Ok(());
    }

    if !metadata.is_dir() {
        return Ok(());
    }

    if should_skip_directory(path) {
        return Ok(());
    }

    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", path.display()))?;
        let child = entry.path();
        if entry.file_type()?.is_dir() {
            if should_skip_directory(&child) {
                continue;
            }
            collect_rhai_files_from_path(&child, files)?;
        } else if is_rhai_file(&child) {
            files.insert(child);
        }
    }

    Ok(())
}

fn should_skip_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target"))
}

fn is_rhai_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rhai"))
}

impl Cli {
    fn format_overrides(&self) -> PartialFormatOptions {
        PartialFormatOptions {
            indent_style: self.indent_style.map(|indent_style| match indent_style {
                CliIndentStyle::Spaces => IndentStyle::Spaces,
                CliIndentStyle::Tabs => IndentStyle::Tabs,
            }),
            indent_width: self.indent_width,
            max_line_length: self.max_line_length,
            trailing_commas: if self.trailing_commas {
                Some(true)
            } else if self.no_trailing_commas {
                Some(false)
            } else {
                None
            },
            final_newline: if self.final_newline {
                Some(true)
            } else if self.no_final_newline {
                Some(false)
            } else {
                None
            },
            container_layout: self.container_layout.map(
                |container_layout| match container_layout {
                    CliContainerLayoutStyle::Auto => ContainerLayoutStyle::Auto,
                    CliContainerLayoutStyle::PreferSingleLine => {
                        ContainerLayoutStyle::PreferSingleLine
                    }
                    CliContainerLayoutStyle::PreferMultiLine => {
                        ContainerLayoutStyle::PreferMultiLine
                    }
                },
            ),
            import_sort_order: self.import_sort_order.map(|import_sort_order| {
                match import_sort_order {
                    CliImportSortOrder::Preserve => ImportSortOrder::Preserve,
                    CliImportSortOrder::ModulePath => ImportSortOrder::ModulePath,
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::{Cli, collect_rhai_files, is_rhai_file, should_skip_directory};

    #[test]
    fn recognizes_rhai_extension() {
        assert!(is_rhai_file(Path::new("main.rhai")));
        assert!(is_rhai_file(Path::new("MAIN.RHAI")));
        assert!(!is_rhai_file(Path::new("main.rs")));
    }

    #[test]
    fn skips_tooling_directories() {
        assert!(should_skip_directory(Path::new(".git")));
        assert!(should_skip_directory(Path::new("target")));
        assert!(!should_skip_directory(Path::new("scripts")));
    }

    #[test]
    fn cli_builds_format_overrides_from_flags() {
        let cli = Cli::parse_from([
            "rhai-fmt",
            "--check",
            "--indent-style",
            "tabs",
            "--indent-width",
            "2",
            "--max-line-length",
            "88",
            "--no-trailing-commas",
            "--no-final-newline",
            "--container-layout",
            "prefer-multi-line",
            "--import-sort-order",
            "module-path",
        ]);

        let options = cli.format_overrides();
        assert_eq!(options.indent_style, Some(crate::IndentStyle::Tabs));
        assert_eq!(options.indent_width, Some(2));
        assert_eq!(options.max_line_length, Some(88));
        assert_eq!(options.trailing_commas, Some(false));
        assert_eq!(options.final_newline, Some(false));
        assert_eq!(
            options.container_layout,
            Some(crate::ContainerLayoutStyle::PreferMultiLine)
        );
        assert_eq!(
            options.import_sort_order,
            Some(crate::ImportSortOrder::ModulePath)
        );
    }

    #[test]
    fn collects_rhai_files_recursively() {
        let temp_root = std::env::temp_dir().join(format!(
            "rhai_fmt_cli_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let nested = temp_root.join("src");
        let target = temp_root.join("target");
        fs::create_dir_all(&nested).expect("create nested");
        fs::create_dir_all(&target).expect("create target");
        fs::write(temp_root.join("main.rhai"), "let x = 1;").expect("write root rhai");
        fs::write(nested.join("lib.rhai"), "let y = 2;").expect("write nested rhai");
        fs::write(temp_root.join("README.md"), "# not rhai").expect("write markdown");
        fs::write(target.join("ignored.rhai"), "let z = 3;").expect("write ignored rhai");

        let files = collect_rhai_files(std::slice::from_ref(&temp_root)).expect("collect files");

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("main.rhai")));
        assert!(
            files
                .iter()
                .any(|path| path.ends_with(Path::new("src").join("lib.rhai")))
        );

        fs::remove_dir_all(&temp_root).expect("cleanup temp root");
    }
}
