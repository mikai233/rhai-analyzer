use anyhow::Result;
use clap::{Parser, ValueEnum};
use lsp_server::Connection;
use lsp_types::InitializeParams;
use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};
use serde::Deserialize;
use tracing::info;

use crate::runtime::notifications::publish_diagnostics_updates;
use crate::runtime::progress::WorkDoneProgressHandle;
use crate::state::path_from_uri;
use crate::state::{FormatterSettings, InlayHintSettings, ServerSettings, ServerState};

use super::event_loop::event_loop;
use super::logging::{LogLevel, LogTarget, init_logging};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TransportKind {
    Stdio,
    Tcp,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "rhai-lsp")]
#[command(about = "Rhai language server")]
struct LaunchOptions {
    #[arg(long, value_enum, default_value_t = TransportKind::Stdio)]
    transport: TransportKind,

    #[arg(long, default_value = "127.0.0.1:9257")]
    tcp_listen: String,

    #[arg(long, value_enum, default_value_t = LogLevel::Warn)]
    log_level: LogLevel,

    #[arg(long, value_enum, default_value_t = LogTarget::Auto)]
    log_target: LogTarget,
}

pub fn run_from_env() -> Result<()> {
    let options = LaunchOptions::parse();
    init_logging(
        options.log_level,
        resolve_log_target(options.transport, options.log_target),
    );

    match options.transport {
        TransportKind::Stdio => run_stdio(),
        TransportKind::Tcp => run_tcp_listen(&options.tcp_listen),
    }
}

fn resolve_log_target(transport: TransportKind, configured: LogTarget) -> LogTarget {
    match configured {
        LogTarget::Auto => match transport {
            TransportKind::Stdio => LogTarget::Stderr,
            TransportKind::Tcp => LogTarget::Stdout,
        },
        explicit => explicit,
    }
}

fn run_stdio() -> Result<()> {
    info!("starting rhai-lsp over stdio");
    let (connection, io_threads) = Connection::stdio();
    run_connection(connection)?;
    io_threads.join()?;
    Ok(())
}

fn run_tcp_listen(address: &str) -> Result<()> {
    info!(address = %address, "starting rhai-lsp over tcp");
    let (connection, io_threads) = Connection::listen(address)?;
    info!(address = %address, "rhai-lsp is listening");
    run_connection(connection)?;
    io_threads.join()?;
    Ok(())
}

fn run_connection(connection: Connection) -> Result<()> {
    let mut server = ServerState::new();
    let initialize = serde_json::to_value(server.initialize_result())?;
    info!("waiting for client initialization");
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let initialize_params = serde_json::from_value::<InitializeParams>(initialize_params)?;
    connection.initialize_finish(initialize_id, initialize)?;
    info!("client initialization completed");
    server.configure_client_capabilities(client_supports_work_done_progress(&initialize_params));
    server.configure_settings(server_settings_from_initialize(&initialize_params));
    let workspace_roots = workspace_roots_from_initialize(&initialize_params);
    if !workspace_roots.is_empty() {
        info!(roots = ?workspace_roots, "loading workspace rhai files");
        let progress = WorkDoneProgressHandle::begin_workspace_warmup(&connection, &mut server)?;
        let load = server.load_workspace_roots(&workspace_roots)?;
        if let Some(progress) = &progress {
            progress.report(
                &connection,
                format!("Loaded {} Rhai files.", load.file_count),
            )?;
        }
        publish_diagnostics_updates(&connection, &server, load.updates)?;
        if let Some(progress) = &progress {
            progress.end(&connection, "Rhai workspace is ready.")?;
        }
    }
    event_loop(&connection, &mut server)?;
    info!("rhai-lsp main loop exited");
    Ok(())
}

fn client_supports_work_done_progress(params: &InitializeParams) -> bool {
    params
        .capabilities
        .window
        .as_ref()
        .and_then(|window| window.work_done_progress)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct InitializeOptions {
    #[serde(default)]
    inlay_hints: InitializeInlayHintOptions,
    #[serde(default)]
    formatting: InitializeFormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeInlayHintOptions {
    #[serde(default = "default_true")]
    variables: bool,
    #[serde(default = "default_true")]
    parameters: bool,
    #[serde(default = "default_true")]
    return_types: bool,
}

impl Default for InitializeInlayHintOptions {
    fn default() -> Self {
        Self {
            variables: true,
            parameters: true,
            return_types: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeFormattingOptions {
    #[serde(default = "default_max_line_length")]
    max_line_length: usize,
    #[serde(default = "default_true")]
    trailing_commas: bool,
    #[serde(default = "default_true")]
    final_newline: bool,
    #[serde(default)]
    container_layout: InitializeContainerLayoutStyle,
    #[serde(default)]
    import_sort_order: InitializeImportSortOrder,
}

impl Default for InitializeFormattingOptions {
    fn default() -> Self {
        Self {
            max_line_length: default_max_line_length(),
            trailing_commas: true,
            final_newline: true,
            container_layout: InitializeContainerLayoutStyle::Auto,
            import_sort_order: InitializeImportSortOrder::Preserve,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
enum InitializeContainerLayoutStyle {
    #[default]
    Auto,
    PreferSingleLine,
    PreferMultiLine,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
enum InitializeImportSortOrder {
    #[default]
    Preserve,
    ModulePath,
}

fn default_true() -> bool {
    true
}

fn default_max_line_length() -> usize {
    100
}

fn server_settings_from_initialize(params: &InitializeParams) -> ServerSettings {
    params
        .initialization_options
        .as_ref()
        .map(server_settings_from_value)
        .unwrap_or_default()
}

pub(crate) fn server_settings_from_value(value: &serde_json::Value) -> ServerSettings {
    let options = value.get("rhai").cloned().unwrap_or_else(|| value.clone());
    let options = serde_json::from_value::<InitializeOptions>(options).unwrap_or_default();

    ServerSettings {
        inlay_hints: InlayHintSettings {
            variables: options.inlay_hints.variables,
            parameters: options.inlay_hints.parameters,
            return_types: options.inlay_hints.return_types,
        },
        formatter: FormatterSettings {
            max_line_length: options.formatting.max_line_length,
            trailing_commas: options.formatting.trailing_commas,
            final_newline: options.formatting.final_newline,
            container_layout: match options.formatting.container_layout {
                InitializeContainerLayoutStyle::Auto => ContainerLayoutStyle::Auto,
                InitializeContainerLayoutStyle::PreferSingleLine => {
                    ContainerLayoutStyle::PreferSingleLine
                }
                InitializeContainerLayoutStyle::PreferMultiLine => {
                    ContainerLayoutStyle::PreferMultiLine
                }
            },
            import_sort_order: match options.formatting.import_sort_order {
                InitializeImportSortOrder::Preserve => ImportSortOrder::Preserve,
                InitializeImportSortOrder::ModulePath => ImportSortOrder::ModulePath,
            },
        },
    }
}

// Keep the root_uri fallback for older clients that do not send workspace_folders.
#[allow(deprecated)]
fn workspace_roots_from_initialize(params: &InitializeParams) -> Vec<std::path::PathBuf> {
    let mut roots = std::collections::BTreeSet::<std::path::PathBuf>::new();

    if let Some(workspace_folders) = &params.workspace_folders {
        for folder in workspace_folders {
            if let Ok(path) = path_from_uri(&folder.uri) {
                roots.insert(path);
            }
        }
    }

    if roots.is_empty()
        && let Some(root_uri) = &params.root_uri
        && let Ok(path) = path_from_uri(root_uri)
    {
        roots.insert(path);
    }

    roots.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use lsp_types::{ClientCapabilities, InitializeParams, WindowClientCapabilities};
    use rhai_fmt::{ContainerLayoutStyle, ImportSortOrder};
    use serde_json::json;

    use crate::runtime::logging::{LogLevel, LogTarget};
    use crate::state::{FormatterSettings, ServerSettings};

    use super::{
        LaunchOptions, TransportKind, client_supports_work_done_progress, resolve_log_target,
        server_settings_from_initialize,
    };

    #[test]
    fn parses_default_stdio_options() {
        let options = LaunchOptions::parse_from(["rhai-lsp"]);

        assert_eq!(options.transport, TransportKind::Stdio);
        assert_eq!(options.tcp_listen, "127.0.0.1:9257");
        assert_eq!(options.log_level, LogLevel::Warn);
        assert_eq!(options.log_target, LogTarget::Auto);
    }

    #[test]
    fn parses_tcp_options() {
        let options = LaunchOptions::parse_from([
            "rhai-lsp",
            "--transport",
            "tcp",
            "--tcp-listen",
            "127.0.0.1:9999",
            "--log-level",
            "debug",
        ]);

        assert_eq!(options.transport, TransportKind::Tcp);
        assert_eq!(options.tcp_listen, "127.0.0.1:9999");
        assert_eq!(options.log_level, LogLevel::Debug);
    }

    #[test]
    fn auto_log_target_uses_stderr_for_stdio() {
        assert_eq!(
            resolve_log_target(TransportKind::Stdio, LogTarget::Auto),
            LogTarget::Stderr
        );
    }

    #[test]
    fn auto_log_target_uses_stdout_for_tcp() {
        assert_eq!(
            resolve_log_target(TransportKind::Tcp, LogTarget::Auto),
            LogTarget::Stdout
        );
    }

    #[test]
    fn detects_client_work_done_progress_support() {
        let params = InitializeParams {
            capabilities: ClientCapabilities {
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..WindowClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
            ..InitializeParams::default()
        };

        assert!(client_supports_work_done_progress(&params));
    }

    #[test]
    fn initialization_options_override_inlay_hint_and_formatter_settings() {
        let params = InitializeParams {
            initialization_options: Some(json!({
                "inlayHints": {
                    "variables": false,
                    "parameters": true,
                    "returnTypes": false
                },
                "formatting": {
                    "maxLineLength": 72,
                    "trailingCommas": false,
                    "finalNewline": false,
                    "containerLayout": "preferMultiLine",
                    "importSortOrder": "modulePath"
                }
            })),
            ..InitializeParams::default()
        };

        let settings = server_settings_from_initialize(&params);
        assert_eq!(
            settings,
            ServerSettings {
                inlay_hints: crate::state::InlayHintSettings {
                    variables: false,
                    parameters: true,
                    return_types: false,
                },
                formatter: FormatterSettings {
                    max_line_length: 72,
                    trailing_commas: false,
                    final_newline: false,
                    container_layout: ContainerLayoutStyle::PreferMultiLine,
                    import_sort_order: ImportSortOrder::ModulePath,
                },
            }
        );
    }
}
