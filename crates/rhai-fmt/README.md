# rhai-fmt

`rhai-fmt` is the dedicated Rhai formatting crate for `rhai-analyzer`.

It owns formatting policy, layout decisions, and edit generation for formatter callers, while keeping parser, IDE, and LSP layers focused on syntax, analysis, and transport.

## Overview

`rhai-fmt` provides a syntax-driven formatter for the current Rhai surface exposed by `rhai-syntax`.
It supports whole-document formatting, structural range formatting, width-aware layout, and comment-preserving rewrites built on top of a dedicated document IR.

The crate is intended to be the single formatting policy layer for:

- document layout
- whitespace normalization
- comment placement
- import normalization
- range-formatting edits

## Feature Summary

### Whole-Document Formatting

- Stable whole-document formatting entry point
- Syntax-tree-driven formatting for the current Rhai AST surface
- Changed/unchanged result reporting for formatter consumers
- Idempotent formatter behavior backed by regression and guarantee tests
- Shared formatting core consumed by both editor integrations and the standalone formatter CLI

### Structural Range Formatting

- Range formatting built on the same core engine as full-document formatting
- Structural owner selection instead of ad hoc text slicing
- Support for small, syntax-stable owners including:
  - `Root`
  - `RootItemList`
  - `Item`
  - `Block`
  - `BlockItemList`
  - supported `Expr` owners
  - `ParamList`
  - `ClosureParamList`
  - `ArgList`
  - `ArrayItemList`
  - `StringPartList`
  - `InterpolationItemList`
  - `ObjectFieldList`
  - `SwitchArmList`
  - `SwitchPatternList`
  - `ForBindings`
  - `DoCondition`
  - `CatchClause`
  - `AliasClause`
  - `ElseBranch`

### Layout Engine

- Dedicated `Doc`-style formatting IR
- `concat`, `indent`, `group`, `soft_line`, `hard_line`, and conditional line-breaking primitives
- Width-aware rendering with tab-aware column measurement
- Shared layout engine for document formatting and range formatting

### Width-Aware Formatting

- Width-constrained wrapping for:
  - multiline containers
  - binary chains
  - access and path chains
  - closure heads
  - import/export heads
  - longer statement and control-flow heads
  - long function signature token sequences
- Configurable container layout preferences
- Trailing-comma-aware multiline layout

### Comment-Preserving Formatting

- Syntax-owned trivia consumption via `rhai-syntax`
- Boundary-aware and sequence-aware comment handling
- Preservation across:
  - top-level item sequences
  - block bodies
  - switch bodies and switch arms
  - delimited containers
  - statement and clause boundaries
  - many expression operator and suffix boundaries
- Comment-directive-based formatter skipping for preserving hand-written source in selected regions

### Import and Top-Level Normalization

- Import sorting support
- Preservation of blank-line-separated import groups
- Syntax/trivia-driven group boundary detection
- Root-level blank-line normalization for mixed top-level item kinds

### Coverage and Fallback

- Explicit support classification for:
  - expressions
  - statements
  - items
  - trivia policy surfaces
  - layout policy surfaces
- Conservative raw-source fallback for unsupported or risky rewrites

## Supported Syntax Areas

The formatter currently covers the high-value Rhai syntax families used throughout the analyzer stack, including:

- functions and parameter lists
- `let`, `const`, `import`, and `export`
- blocks, `if`, `switch`, `while`, `loop`, `for`, and `do`
- closures
- arrays and objects
- calls, indexing, field access, and path expressions
- unary, binary, assignment, and parenthesized expressions
- interpolated strings
- safe navigation and safe indexing
- typed methods and caller-scope calls
- `try/catch` and `do/until`

The remaining gaps are primarily about formatting depth and policy richness rather than syntax reach.

## Configuration

`rhai-fmt` exposes a shared option model used by formatter callers, IDE integrations, and the LSP layer.

Current option surface:

- `indent_style`
- `indent_width`
- `max_line_length`
- `trailing_commas`
- `final_newline`
- `container_layout`
- `import_sort_order`

Project-level formatter configuration can also be loaded from the nearest `rhai.toml` file via a `[formatting]` section.

See [`RHAI_TOML.md`](../../../RHAI_TOML.md) for the formal configuration reference and precedence model.

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

Current policy enums:

- `IndentStyle`
  - `Spaces`
  - `Tabs`
- `ContainerLayoutStyle`
  - `Auto`
  - `PreferSingleLine`
  - `PreferMultiLine`
- `ImportSortOrder`
  - `Preserve`
  - `ModulePath`

## Command-Line Usage

`rhai-fmt` also ships as a standalone formatter binary built on the same formatting core used by `rhai-ide` and `rhai-lsp`.

Typical usage:

```bash
cargo run -p rhai-fmt --
cargo run -p rhai-fmt -- path/to/file.rhai
cargo run -p rhai-fmt -- src scripts --check
```

When installed as a cargo subcommand binary, it can also be invoked as:

```bash
cargo rhai-fmt --check
```

Current CLI behavior:

- recursively formats `.rhai` files from the current directory when no paths are provided
- accepts file and directory inputs
- skips `.git` and `target`
- supports `--check` for CI-style verification
- loads the nearest `rhai.toml` `[formatting]` section for each file before applying CLI overrides
- uses the same `FormatOptions` model as the IDE and LSP integrations
- skips files with syntax errors instead of forcing a rewrite

## Comment Directives

`rhai-fmt` supports a formatter directive for preserving the next syntax item or statement exactly as written:

- `// rhai-fmt: skip`

Example:

```rhai
fn run() {
    // rhai-fmt: skip
    let  weird   =#{ name :"Ada", values :[1,2,3]};

    let normal = 1 + 2;
}
```

Current behavior:

- the directive applies to the next format-stable item or statement
- skipped regions are emitted from original source instead of being reformatted
- skipped imports are excluded from import-run reordering so the formatter does not shuffle past explicitly preserved source

## Syntax and Trivia Model

`rhai-fmt` is built on the rowan-based `rhai-syntax` tree and consumes syntax-owned trivia data rather than relying on direct source slicing as its primary model.

The current formatter model uses:

- a trivia-bearing parse tree
- boundary-to-slot trivia resolution
- owned trivia for node- and sequence-level rendering
- syntax-side structural spans for range formatting

This means parser and AST shape are no longer the primary limitation they were earlier in the project. Most remaining work now lives in formatter policy, layout depth, and fallback confidence.

## Current Boundaries

The formatter is already suitable for structural, comment-aware formatting work across the current analyzer stack, but some areas remain intentionally conservative:

- width-aware layout is broad, but not yet fully generalized across every statement-like or policy-heavy surface
- import/export policy is still relatively shallow compared with the rest of the formatter model
- some range-formatting owners are still missing
- unsupported or risky syntax still falls back to original source slices rather than forcing a rewrite
- trivia/layout support tracking exists, but is still coarser than the syntax-family support matrix

## Quality and Test Coverage

The regression suite currently includes:

- idempotence checks
- parse-stability checks
- comment-preservation cases
- range-formatting boundary cases
- corpus and profile snapshots
- fallback behavior checks
- width-sensitive formatting cases

This test coverage is intended to keep formatting behavior stable while the structural model continues to deepen.

## Position in the Workspace

`rhai-fmt` is the formatting layer for the wider analyzer workspace.

- `rhai-syntax` provides the syntax tree and trivia model
- `rhai-fmt` owns formatting policy and edit generation
- `rhai-ide` and `rhai-lsp` act as thin adapters that pass configuration and request formatting

This separation keeps formatting behavior centralized and avoids pushing formatter policy into syntax, IDE, or protocol code.
