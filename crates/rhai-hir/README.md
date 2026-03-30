# rhai-hir

`rhai-hir` is the semantic lowering layer for `rhai-analyzer`.

It turns parsed Rhai syntax into a single-file semantic model that is easier for diagnostics, navigation, completion, and type inference to consume.

## Implemented Features

### Core Semantic Model

- Stable IDs for scopes, symbols, references, expressions, and bodies
- Scope trees for functions, blocks, loops, closures, interpolations, catch scopes, and switch-arm regions
- Symbol/reference tracking with duplicate-definition and shadowing metadata
- Source-range mapping on primary semantic entities

### Name and Scope Semantics

- Lexical resolution for locals and parent scopes
- Forward resolution for functions
- Correct handling of `global::` roots and contextual `this`
- Function semantics aligned with Rhai: no outer-variable capture, access to global functions, and access to global import aliases

### Module and Function Semantics

- Explicit export lowering for variables/constants plus `export let` / `export const`
- Implicit export of top-level non-`private` functions
- Semantic rejection of explicit `export` targets that are not valid module exports
- Import/export metadata shaped for downstream module-graph and visibility queries
- Typed methods, quoted typed methods, and caller-scope call metadata

### Docs, Diagnostics, and Queries

- Doc block attachment and extraction of `@type`, `@param`, `@return`, and `@field`
- Semantic diagnostics for unresolved imports/exports, duplicate definitions, doc/type mismatches, and related local issues
- Query helpers for symbol visibility, definitions, references, bodies, docs, function parameters, member access, and completion-oriented lookups

### Type-Inference Support

- Stable per-expression result slots
- Value-flow edges from declarations and assignments into symbols
- Ordered mutation and read metadata for member/index flow-sensitive inference
- Call-site metadata with callee, arguments, caller-scope markers, and parameter-hint support
- Expected-type sites for declarations, returns, and call arguments
- Loop binding metadata for `for` iteration inference
- Path/module-qualified information that downstream inference can consume directly

## Current Boundaries

- Path segments are preserved well enough for inference and navigation, but full path-to-target resolution is not yet complete in all cases.
- Field/member resolution is still intentionally lightweight and leaves more type-aware disambiguation to upper layers.
- Cross-file symbol resolution through imports/exports is not yet a fully native HIR capability.
- Interpolation/string-part ownership and object-literal shape metadata can still be strengthened for future inference passes.

## Next Steps

- Stronger path and module resolution that resolves more qualified names to concrete semantic targets
- Richer field/member resolution metadata, especially where object fields and callable members overlap
- Better cross-file semantic linkage for navigation and rename
- More explicit lower-layer metadata for interpolation ownership, object shapes, and other inference-heavy features
