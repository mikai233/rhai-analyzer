# rhai-fmt

`rhai-fmt` is the dedicated formatting crate for `rhai-analyzer`.

It is intended to own formatting policy, layout decisions, and formatting output generation without pushing formatter logic into `rhai-syntax`, `rhai-ide`, or `rhai-lsp`.

## Implemented Features

### Whole-Document Formatting Core

- Dedicated crate boundary for Rhai formatting work
- Shared formatting option model and stable whole-document entry point
- Range-formatting entry point built on top of the whole-document formatter
- Syntax-tree-driven formatting for core Rhai items and expressions
- Changed/unchanged result reporting for formatter callers
- Regression tests for basic formatting rewrites

### First Implemented Layout Rules

- Function items and parameter lists
- `let` / `const` / `import` / `export`
- Blocks, `if`, `switch`, and loop-family control flow
- Closures and tail-expression preservation
- Arrays, objects, calls, indexes, and field access
- Binary/operator spacing and root-level blank-line policy

## Current Boundaries

- Formatting coverage is still focused on common document-wide Rhai constructs, not the full language surface.
- Unsupported or risky syntax currently falls back to original source slices instead of forcing a rewrite.
- Comment-preserving formatting and range formatting are intentionally deferred.
- Range formatting is currently conservative and returns the minimal changed diff region from a whole-document rewrite.

## Next Steps

- Expand whole-document coverage across the remaining expression and statement forms
- Add import-block normalization that composes cleanly with existing import assists
- Expose formatter queries through `rhai-ide`, then wire document formatting through `rhai-lsp`
