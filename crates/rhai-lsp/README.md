# rhai-lsp

`rhai-lsp` is the language-server transport crate for `rhai-analyzer`.

Its responsibility is to expose the semantic capabilities implemented in `rhai-ide`
through the Language Server Protocol, while keeping protocol concerns separate from
parsing, lowering, inference, and editor policy.

## Scope

The crate owns:

- process startup and transport selection
- long-lived server state and document lifecycle
- LSP request and notification dispatch
- conversion between `rhai-ide` models and LSP protocol types
- workspace preload, warm-up, and protocol-facing diagnostics publication

The crate does not own:

- Rhai syntax parsing
- HIR lowering
- incremental semantic analysis
- type inference rules
- formatter logic

## Architecture

The current implementation follows a synchronous event-loop model with explicit state
and protocol layers.

- `main.rs`
  - process entry point
- `runtime.rs` and `runtime/`
  - transport bootstrap, event loop, request routing, notification routing, logging, and progress reporting
- `state.rs`
  - mutable server state, open documents, workspace preload, path/URI handling, and change application
- `protocol.rs`
  - conversion between internal IDE models and LSP wire types

This model is intentionally closer to the `rust-analyzer` style of "event loop +
shared state + immutable analysis snapshots" than to a fully async-first server.

## Implemented Capabilities

### Core server behavior

- long-lived state built around `rhai-ide::AnalysisHost`
- normalized path and URI management
- full-document sync for open, change, and close
- workspace preload for unopened `.rhai` files
- static-import graph expansion, including files outside workspace roots
- query-support warming for hot files after rebuilds
- support for both `stdio` and TCP transports
- configurable logging with transport-aware defaults
- handling of workspace file rename notifications to keep server state in sync

### Language features

- hover
- goto definition
- references
- rename
- completion and completion resolve
- signature help
- inlay hints
- document highlights
- document symbols
- workspace symbols
- semantic tokens
- folding ranges
- call hierarchy
- document formatting
- document range formatting
- code actions

### Protocol integration

- server metadata in `initialize`
- semantic-token legend and encoding
- protocol conversion for navigation, rename, diagnostics, formatting, and completion
- workspace file operation registration for `.rhai` files
- work-done progress during workspace warm-up
- project-level formatting policy via the nearest `rhai.toml` `[formatting]` section, layered with editor formatting options

## Shared Formatter Configuration

Formatting requests served through `rhai-lsp` use the same `rhai-fmt` core as the standalone formatter CLI.

Project-level formatter settings can be provided through `rhai.toml`; see [`RHAI_TOML.md`](../../RHAI_TOML.md) for the formal configuration reference and precedence model.

## Operational Characteristics

- synchronization is currently full-document, not incremental text diff sync
- request handling still runs on the foreground event loop
- background worker orchestration is not introduced yet
- TCP transport is intended for local debugging and protocol inspection
- protocol output is intentionally conservative and can be enriched as `rhai-ide` grows

## Testing

The crate includes focused tests for:

- diagnostics publication
- query-style language features
- code action translation
- workspace preload and cross-file behavior
- protocol-level rename and file-operation handling

## Future Work

- introduce explicit background task infrastructure for heavier workspace jobs
- evaluate incremental sync when the editor integration warrants it
- continue improving protocol richness for diagnostics, code actions, and progress reporting
