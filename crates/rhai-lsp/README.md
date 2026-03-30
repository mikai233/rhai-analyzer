# rhai-lsp

`rhai-lsp` is the LSP transport layer for `rhai-analyzer`.

It is responsible for turning editor/LSP requests into calls against `rhai-ide`, and for translating semantic results into protocol-shaped responses and notifications.

It does not own parsing, semantic lowering, incremental database logic, or editor-facing semantic policy.

## Implemented Features

### Server Core

- Long-lived server state built around `rhai-ide::AnalysisHost`
- Managed open-document tracking with normalized file paths and document versions
- Full-document sync handling for open/change/close workflows
- Query-support warming for hot files after rebuilds

### Protocol and Capability Wiring

- `initialize` response with server metadata
- Advertised capabilities for:
  - full text document sync
  - goto definition
  - references
  - rename
  - hover
  - document highlights
  - call hierarchy
  - folding ranges
  - document symbols
  - workspace symbols
  - completion
  - signature help
  - inlay hints
  - semantic tokens
  - document formatting
  - document range formatting
  - code actions

### Handlers

- Diagnostic publication based on semantic and project-aware analysis results
- Close/update behavior that refreshes diagnostics consistently
- Code action translation for source fixes and import-related edits
- Query forwarding for completion, completion resolve, signature help, call hierarchy, document highlights, folding ranges, inlay hints, semantic tokens, workspace symbols, document formatting, and document range formatting
- Semantic-token legend/encoding for Rhai token categories plus declaration/readonly modifiers

### Test Coverage

- Focused LSP-layer tests for diagnostics
- Focused LSP-layer tests for code actions
- Focused LSP-layer tests for query-style language features

## Current Boundaries

- The crate is intentionally thin and currently exposes only a small set of LSP features.
- Sync is currently full-document sync, not incremental edit sync.
- Protocol presentation is still fairly minimal and can grow richer as `rhai-ide` adds more structured metadata.

## Next Steps

- Add protocol wiring for more `rhai-ide` queries and richer completion flows
- Support incremental sync if/when it becomes worthwhile for the editor integration story
- Expand protocol output richness for diagnostics, code actions, and future semantic features
