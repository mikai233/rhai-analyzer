# rhai-ide

`rhai-ide` is the editor-facing semantic layer for `rhai-analyzer`.

It translates database facts into stable IDE-shaped results without exposing raw storage details or LSP protocol types.

## Implemented Features

### Core API and Edit Model

- `AnalysisHost` for applying changes to the long-lived database
- Immutable `Analysis` snapshots for read queries
- IDE-specific result types for diagnostics, hover, completion, navigation, rename, and source changes
- Shared `TextEdit` and `SourceChange` models for semantic edits
- Assist/fix identifiers and grouped action metadata

### Read Queries

- Diagnostics
- Document formatting
- Hover
- Call hierarchy
- Document highlights
- Folding ranges
- Semantic tokens
- Document symbols
- Workspace symbols and fuzzy workspace-symbol matching
- Goto definition
- Project-wide references
- Rename planning
- Completion
- Auto-import/source-fix actions
- Signature help
- Inlay hints

### Diagnostics and Source Actions

- Project-aware diagnostics built on database and workspace state
- Diagnostic-associated quick fixes
- Auto-import planning
- Broken-import fixes after export visibility changes
- Whole-document and range formatting backed by `rhai-fmt`
- Organize-imports, remove-unused-imports, and import normalization planning

### Type-Aware UX

- Hover fallback to inferred local/function types when explicit annotations are absent
- Hover metadata that distinguishes declared and inferred signatures and surfaces ambiguity/inference notes
- Completion detail backed by inferred types
- Lazy completion-item resolve for symbol docs/details without forcing every completion list to carry full payloads
- Signature help for local functions, builtin functions, typed methods, imported typed methods, and module-qualified imported functions
- Builtin and host-type member completion/signature help, including receiver-specialized generic host methods
- Inlay hints for inferred local variable types, closure parameter types, and function/closure return types
- Semantic token classification for Rhai keywords, comments, literals, operators, namespaces, HIR-backed symbols, typed-method receivers, and declaration/readonly modifiers

### Rename and Cross-File Editing

- Cross-file rename planning
- Rename preflight issue reporting
- Concrete edit generation from rename plans
- Preview-friendly grouping of source changes

## Current Boundaries

- Completion ranking and lazy completion-item resolve are now present, but still fairly lightweight
- Hover/diagnostic presentation can still grow richer in related information and explanatory detail
- Golden-style output tests for larger edit plans are still fairly light

## Next Steps

- Broader inlay hints beyond the current inferred variable/closure/function type hints
- Better completion ranking, richer resolve payloads, and import-on-accept behavior
- Richer hover and diagnostic presentation
- Additional editor-facing semantic queries beyond the current folding/call-hierarchy surface
