# `rhai.toml`

`rhai.toml` is the project-level configuration file for `rhai-analyzer` tooling.

It is intended to provide a stable, repository-local place for settings that should be shared by:

- the standalone `rhai-fmt` CLI
- editor integrations backed by `rhai-lsp`
- future workspace-level tooling that needs the same Rhai-facing policy

## Current Scope

The current supported surface is formatter configuration through a `[formatting]` section.

Example:

```toml
[formatting]
indent_style = "spaces"
indent_width = 4
max_line_length = 100
trailing_commas = true
final_newline = true
container_layout = "auto"
import_sort_order = "preserve"
```

## Resolution Model

For a given file, tooling searches for the nearest `rhai.toml` by walking upward from the file's directory.

That means:

- files in nested subdirectories can inherit a repository-level `rhai.toml`
- subtrees can override parent configuration by placing another `rhai.toml` closer to the file
- formatting behavior is resolved per file, not only once per process

## Precedence

The current precedence model is:

1. built-in formatter defaults
2. nearest `rhai.toml` `[formatting]` values
3. caller-specific overrides

Caller-specific overrides currently mean:

- CLI flags for `rhai-fmt`
- LSP/editor formatting options and server-side formatter settings

This keeps project policy central while still allowing editor-specific indentation requests and explicit command-line overrides.

## Formatting Keys

### `indent_style`

Controls indentation style.

Supported values:

- `"spaces"`
- `"tabs"`

### `indent_width`

Controls the indentation width used by the formatter.

For `tabs`, this is also the width used for line-measurement decisions.

### `max_line_length`

Controls the target line width for width-aware layout and line breaking.

### `trailing_commas`

Controls whether multiline comma-separated structures should preserve trailing commas when the formatter chooses multiline layout.

Supported values:

- `true`
- `false`

### `final_newline`

Controls whether formatted files should end with a trailing newline.

Supported values:

- `true`
- `false`

### `container_layout`

Controls preferred layout style for formatter-managed containers.

Supported values:

- `"auto"`
- `"prefer_single_line"`
- `"prefer_multi_line"`

### `import_sort_order`

Controls import normalization policy.

Supported values:

- `"preserve"`
- `"module_path"`

## Tooling Consumers

### `rhai-fmt`

The standalone formatter loads the nearest `rhai.toml` for each file before applying explicit CLI overrides.

This makes repository-wide formatting predictable in batch formatting and CI-style `--check` runs.

### `rhai-lsp`

LSP formatting requests also load the nearest `rhai.toml` before applying editor-provided indentation options and server formatter settings.

This keeps editor formatting and CLI formatting aligned on the same project policy.

## Current Boundaries

- Only the `[formatting]` section is currently supported.
- Unknown keys are ignored only insofar as TOML deserialization allows omitted known fields; unrelated top-level sections are fine.
- The file is currently formatter-focused and does not yet replace Rust-side project metadata supplied through `rhai-project`.

## Future Growth

`rhai.toml` is intended to be the natural place for additional workspace-level analyzer policy once those settings are ready to be shared across CLI, IDE, and LSP entry points.
