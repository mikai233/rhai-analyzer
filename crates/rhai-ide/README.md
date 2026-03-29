# rhai-ide

`rhai-ide` is the IDE-facing semantic layer for `rhai-analyzer`.

Its job is to sit between:

- `rhai-db`, which owns incremental analysis state and cross-file indexes,
- editor / LSP features, which need stable, user-facing semantic queries and edit plans.

It should be the crate that turns database facts into IDE-shaped results:

- diagnostics suitable for publication,
- hover and navigation results,
- completion items and future completion resolves,
- rename / code action / assist planning,
- source-change planning that higher layers can translate into protocol-specific edits.

It should not own parsing rules, file-local lowering, cache invalidation policy, or LSP protocol types.

## IDE Layer Checklist

This checklist tracks what `rhai-ide` already provides and what still needs to land before it becomes a strong semantic facade in the style of `rust-analyzer`'s IDE layer.

### Layer Boundary and Core API

- [x] `AnalysisHost` wrapper over the long-lived database
- [x] cheap immutable `Analysis` snapshot for read queries
- [x] IDE-facing result types that hide raw database internals
- [x] clean separation from LSP protocol types such as `Uri`, `Position`, and `CodeAction`
- [x] thin translation of `rhai-db` facts into user-facing semantic query results
- [x] unified `TextEdit` model for semantic edits
- [x] unified `SourceChange` model for single-file and multi-file edits
- [ ] explicit `FileSystemEdit` model if assists later need file creation / rename support
- [x] stable assist / fix identifiers and grouping metadata

### Read Queries for Downstream Consumers

- [x] file diagnostics query
- [x] hover query
- [x] document symbol query
- [x] workspace symbol query
- [x] workspace symbol fuzzy matching query
- [x] goto-definition query
- [x] project-wide references query
- [x] rename planning query
- [x] completion query
- [x] auto-import action query
- [x] signature help query
- [ ] document highlight query
- [ ] folding range query
- [ ] semantic tokens query
- [ ] inlay hints query
- [ ] call hierarchy query

### Diagnostics and Fixes

- [x] project-aware diagnostics surfaced through an IDE-friendly type
- [x] cross-file import/export breakage surfaced through normal diagnostics publishing
- [x] diagnostic results paired with quick-fix candidates
- [x] structured fix metadata per diagnostic
- [ ] fix-all style aggregation where multiple diagnostics share one operation
- [ ] severity / tags / related-information shaping beyond plain message and range

### Source Changes and Assists

- [x] auto-import planning for unresolved workspace exports
- [x] generic assist framework that can return multiple semantic actions at a position / range
- [x] source changes returned independently from LSP-specific `CodeAction` shapes
- [x] rename plan materialized into concrete edits
- [x] unresolved-import quick fix built on the shared assist framework
- [x] broken-import quick fix after export visibility changes
- [x] organize-imports planning
- [x] remove-unused-imports planning
- [x] merge duplicate imports / normalize import style planning

### Completion Experience

- [x] merge visible symbols, project symbols, and member completions
- [x] propagate basic detail / docs where already available
- [x] mark project-symbol completions with origin metadata
- [ ] completion ranking policy beyond current basic ordering
- [ ] completion item resolve for lazy docs / detail loading
- [ ] completion additional text edits such as auto-import on accept
- [ ] snippet / call-argument completion support where appropriate

### Type-Aware UX

- [x] signature help backed by local, workspace-exported, and builtin function signatures
- [x] signature help for imported global typed methods exposed by linked Rhai modules
- [x] signature help for module-qualified imported functions such as `tools::helper(...)`
- [x] signature help for nested module-qualified imported functions such as `tools::sub::helper(...)`
- [x] hover fallback to inferred symbol/function types when declarations lack explicit annotations
- [x] completion detail backed by inferred local symbol types when declared annotations are absent
- [ ] inlay hints driven by inferred parameter / variable / return types
- [ ] richer hover output that can explain inferred return types and origin flows

### Rename and Cross-File Editing

- [x] cross-file rename planning
- [x] rename issue reporting before edit application
- [x] concrete workspace edit generation from rename plans
- [ ] conflict grouping and richer rename diagnostics for UI presentation
- [x] preview-friendly change grouping for downstream clients

### LSP Service Readiness

- [x] query outputs that can be mapped into LSP without exposing database internals
- [x] stable position-based query entry points for open-document workflows
- [x] host methods to apply file changes and inspect hot-query support
- [x] range-based code action collection API
- [x] code action grouping by intent (`quickfix`, `refactor`, `source`)
- [ ] completion resolve payload support
- [ ] richer hover / diagnostic related information for protocol translation

### Reliability and Test Coverage

- [x] unit tests for diagnostics, navigation, references, rename planning, completion, and auto-import planning
- [ ] golden tests for multi-edit source changes
- [ ] golden tests for future assist / code action output
- [ ] regression tests for ranking and tie-breaking behavior

## Notes

- `rhai-db` should answer "what is true about the workspace?"
- `rhai-ide` should answer "what should the editor show or do with those facts?"
- `rhai-lsp` should answer "how do those results map onto the LSP protocol?"

- The next major milestone for `rhai-ide` is establishing a shared edit model:
  `TextEdit -> SourceChange -> Assist / DiagnosticFix`.
  Once that exists, auto-import, broken-import fixes, organize imports, and rename edits can all converge on one path instead of each feature inventing its own return shape.
