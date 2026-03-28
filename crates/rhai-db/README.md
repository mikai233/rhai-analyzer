# rhai-db

`rhai-db` is the analysis database layer for `rhai-analyzer`.

Its job is to own the long-lived analysis state that sits between:

- editor / LSP document changes,
- project / host-environment configuration,
- file-local syntax and HIR computation,
- workspace-wide indexing and query reuse.

It should be the crate that makes downstream IDE and LSP features fast, incremental, and predictable.

It does not own parsing rules, single-file semantic lowering, or the LSP protocol surface.

## Database Coverage Checklist

This checklist tracks what `rhai-db` already provides and what still needs to land before downstream crates can build a strong and high-performance language server on top of it.

### Inputs and Base State

- [x] in-memory application of file text changes
- [x] document version tracking in incoming changes
- [x] project configuration as database input
- [x] stable `FileId` ownership through `rhai-vfs`
- [x] immutable read snapshot API
- [x] explicit database input slots for parse / HIR / index dependencies
- [x] persistent storage for per-file analysis products beyond raw text
- [x] explicit source-root and workspace membership tracking
- [x] canonical path normalization and file identity rules
- [x] explicit file removal / unload support

### Snapshots and Read Concurrency

- [x] cheap cloneable snapshot object for downstream readers
- [x] read access to file text through a snapshot
- [x] read access to project configuration through a snapshot
- [x] clear snapshot consistency guarantees across all derived analyses
- [x] snapshot access to parsed syntax trees
- [x] snapshot access to lowered HIR
- [x] snapshot access to file and workspace indexes
- [x] snapshot access to host-provided symbols and type metadata
- [x] structure suitable for concurrent read-heavy IDE workloads

### Incremental File Analysis

- [x] cached `FileId -> Parse`
- [x] cached `FileId -> FileHir`
- [x] cached syntax diagnostics per file
- [x] cached semantic diagnostics per file
- [x] cached document symbols per file
- [x] cached per-file completion/navigation support data
- [x] lazy computation so unchanged files are not repeatedly re-parsed and re-lowered
- [x] explicit dependency tracking from derived data back to file text and project inputs
- [x] targeted invalidation when a file changes

### Workspace Indexing and Cross-File State

- [x] aggregated workspace symbol index
- [x] aggregated file symbol index handoff from `rhai-hir`
- [x] module graph storage for imports / exports across files
- [x] reverse lookup from stable symbol identity to owning file
- [x] project-wide symbol search support
- [x] project-wide reference and rename planning support
- [x] cross-file import / export linkage cache
- [x] explicit workspace dependency graph with forward and reverse file edges
- [x] incremental refresh of workspace indexes after single-file edits

### Host and Project Semantics

- [x] cached host-provided module/function/type inventory from `rhai-project`
- [x] external signature index assembly for downstream type-aware queries
- [x] custom syntax / reserved-symbol policy inputs from project configuration
- [x] workspace-scoped view of engine options and enabled capabilities
- [x] clean boundary between file-local HIR facts and project/engine-provided facts

### Query Surface for Downstream Crates

- [x] direct database APIs for `parse(file_id)` and `hir(file_id)`
- [x] direct database APIs for file diagnostics
- [x] direct database APIs for project-aware diagnostics that account for workspace imports / exports
- [x] direct database APIs for document symbols and workspace symbols
- [x] direct database APIs for module graph and import/export linkage
- [x] direct database APIs for project-aware completion inputs
- [x] direct database APIs for auto-import candidate and edit planning
- [x] direct database APIs for cross-file navigation inputs
- [x] query shapes designed for `rhai-ide` instead of raw storage walking

### Invalidation and Scheduling Readiness

- [x] batched change application entry point
- [x] explicit invalidation of only affected file-local caches
- [x] explicit invalidation of only affected workspace indexes
- [x] distinction between text-only changes and project-configuration changes
- [x] fast path for no-op edits or stale document versions
- [x] bounded recomputation strategy for high-frequency editor updates
- [x] support for background precomputation / warming of hot queries
- [x] affected-file / change-impact reporting for downstream schedulers
- [x] configurable query-support cache budgeting and eviction

### LSP Service Readiness

- [x] enough caching to answer diagnostics without recomputing the whole workspace
- [x] enough indexing to answer workspace symbol queries efficiently
- [x] enough project state to support cross-file goto-definition
- [x] enough dependency tracking to support stable rename planning across files
- [x] enough workspace resolution to suppress false unresolved-import diagnostics when exports exist
- [x] enough cross-file dependency tracking to surface broken import-usage diagnostics after export visibility changes
- [x] enough import metadata to drive auto-import fixes from unresolved names
- [x] enough snapshot isolation for concurrent LSP requests
- [x] enough structure to support cancellation and stale-result dropping in higher layers

### Reliability and Observability

- [x] unit tests covering cache invalidation behavior
- [x] unit tests covering snapshot consistency after edits
- [x] unit tests covering project-config update behavior
- [x] test coverage for multi-file indexing scenarios
- [x] debug-friendly inspection hooks for cache contents and invalidation reasons
- [x] lightweight performance instrumentation for parse / lower / index recomputation
- [x] per-file performance and cache-state inspection

## Notes

- `rhai-db` should be the main home for incrementalism. `rhai-syntax` and `rhai-hir` should stay mostly pure and reusable; the database decides when their results are reused or invalidated.
- `rhai-db` should not become an LSP-shaped crate. It should expose analysis-oriented snapshots and queries that `rhai-ide` can consume cleanly.
- For a high-performance Rhai language server, the next major step is turning `rhai-db` from a text-and-config container into a cache-owning analysis coordinator.
- A good rule of thumb is: if a downstream IDE query would otherwise repeatedly re-parse, re-lower, or re-index unchanged files, that missing optimization likely belongs in `rhai-db`.
