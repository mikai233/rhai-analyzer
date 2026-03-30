# rhai-analyzer

`rhai-analyzer` is a workspace for a Rhai language-analysis stack built around the real Rhai language model instead of an analyzer-specific dialect.

The workspace is split into a few focused crates:

- `rhai-vfs`: virtual file-system state, file identities, and document versions
- `rhai-syntax`: resilient Rhai lexer/parser and typed AST wrappers
- `rhai-hir`: single-file semantic lowering, symbol/reference modeling, and semantic diagnostics
- `rhai-project`: host-environment and project metadata such as modules, types, and engine options
- `rhai-db`: incremental analysis database, cross-file indexes, and type inference
- `rhai-fmt`: formatter-facing layout pipeline and formatting policy entry points
- `rhai-ide`: editor-facing semantic queries such as diagnostics, completion, hover, and rename
- `rhai-lsp`: LSP transport, request handling, and protocol wiring

## Design Goals

- Stay aligned with official Rhai syntax and semantics
- Keep parsing, lowering, database state, and editor queries cleanly separated
- Make cross-file analysis incremental and predictable
- Expose IDE-friendly results without leaking storage details into higher layers

## Current Shape

- Rhai syntax support is broad enough for real IDE work, including modules, typed methods, caller-scope calls, and resilient recovery.
- HIR lowering models scopes, symbols, references, bodies, imports/exports, function semantics, and type-inference support metadata.
- The database layer already supports cross-file module linking, project-aware diagnostics, substantial type inference, builtin/host signatures, and workspace queries.
- The IDE layer exposes diagnostics, hover, completion, signature help, references, rename planning, and import/source-edit assists.
- The formatter layer now has a first whole-document formatter for core Rhai constructs, with room to expand coverage independently from syntax and IDE crates.

## Near-Term Focus

- Continue deepening the type-inference model, especially narrowing/refinement, container semantics, and remaining dynamic-module edge cases.
- Strengthen lower-layer semantic metadata where inference and cross-file navigation still rely on recovery logic.
- Keep refining the user-facing IDE surface so inferred and imported semantics present cleanly in completion, hover, and diagnostics.
- Keep expanding `rhai-fmt` beyond the initial whole-document core toward broader syntax coverage and LSP-facing formatting support.
