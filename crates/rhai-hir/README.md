# rhai-hir

`rhai-hir` is the semantic lowering layer for `rhai-analyzer`.

Its job is to turn syntax trees into a more IDE-friendly model of:

- scopes,
- symbols,
- references,
- bodies and control-flow summaries,
- doc-attached type annotations.

It does not own parsing, project-wide module loading, or the final IDE/LSP query surface.

## HIR Coverage Checklist

This checklist tracks what the current lowering layer already models and what still needs to land before downstream features like goto-definition, diagnostics, and completion feel comfortable to build on.

### Core Lowering

- [x] file-level lowering entry point
- [x] stable IDs for scopes, symbols, references, and bodies
- [x] scope tree with parent/child relationships
- [x] top-level function lowering
- [x] local variable and constant lowering
- [x] import/export alias lowering
- [x] body nodes for functions, blocks, closures, and interpolations
- [x] source range mapping on all primary HIR entities
- [x] stable per-node body/expression identity beyond ranges

### Scopes and Symbols

- [x] function scopes
- [x] block scopes
- [x] loop scopes
- [x] closure scopes
- [x] interpolation scopes
- [x] parameter symbols
- [x] `for` binding symbols
- [x] `catch` binding symbols
- [x] reverse reference index on symbols
- [x] dedicated scope kinds for `catch`, `switch` arms, and other finer-grained regions
- [x] explicit shadowing / duplicate-definition metadata

### References and Name Resolution

- [x] name references
- [x] path-segment references
- [x] field references
- [x] lexical name resolution for local and parent scopes
- [x] forward resolution for functions
- [x] future locals are not resolved too early
- [x] `global::` roots do not become fake name references
- [ ] resolution for path segments into real symbols/modules
- [ ] resolution for field accesses when type/project info is available
- [x] explicit modeling for `this`
- [ ] project-wide / cross-file resolution
- [ ] import/export linkage and module graph aware resolution

### Rhai Module Semantics Alignment

- [x] explicit export lowering for global variables/constants and `export let` / `export const`
- [x] semantic rejection of explicit `export` targets that resolve to functions, params, aliases, or non-global bindings
- [x] implicit module export of top-level non-`private` functions
- [x] exclusion of `private fn` items from implicit module exports
- [x] module-graph output that merges explicit variable exports with implicit public-function exports
- [x] static semantic/type diagnostics for `import` expressions that can be proven not to evaluate to `string`
- [x] conservative static import-path evaluation for string literals, interpolated strings, simple string concatenation, block tail values, and `if` branches with matching string outcomes
- [x] global-level `import` aliases remain visible inside functions
- [ ] clearer distinction between “syntactically valid dynamic import” and “workspace-linkable static import”
- [ ] module-member resolution/querying that follows Rhai import visibility rules instead of analyzer-specific shortcuts

### Rhai Function Semantics Alignment

- [x] semantic rejection of nested function definitions happens earlier in syntax
- [x] functions do not capture outer variables
- [x] functions can still call other functions across file/global scope
- [x] functions can access global-level imported modules
- [x] typed-method lowering/metadata for `fn Type.method(...)` / `fn "Type".method(...)`
- [x] query support for contextual `this` typing inside typed and blanket methods
- [x] caller-scope call metadata for `foo!(...)` / `call!(...)`
- [x] full caller-scope call semantics and downstream query behavior

### Docs and Type Annotations

- [x] doc block attachment from syntax trivia
- [x] `@type` extraction on declarations
- [x] `@param` extraction on function parameters
- [x] `@return` extraction on function signatures
- [x] synthesized function type from doc tags
- [x] `@field` attachment to object-like declarations
- [x] attachment rules for more declaration kinds and edge cases
- [x] richer type-reference surface beyond the current minimal parser

### Bodies and Control Flow

- [x] body ownership for functions
- [x] nested-body stack during lowering
- [x] control-flow event recording for `return`
- [x] control-flow event recording for `throw`
- [x] control-flow event recording for `break`
- [x] control-flow event recording for `continue`
- [x] control-flow propagation through nested blocks
- [x] closure / interpolation boundaries for control-flow propagation
- [x] optional value ranges on control-flow events
- [x] explicit return-value / throw-value collections per body
- [x] loop-target / break-target modeling
- [x] unreachable-code or fallthrough summaries

### Query Surface for Downstream Crates

- [x] `scope(id)` access
- [x] `symbol(id)` access
- [x] `reference(id)` access
- [x] `body(id)` access
- [x] `doc_block(id)` access
- [x] `find_scope_at(offset)`
- [x] `symbol_at(range)`
- [x] `reference_at(range)`
- [x] `references_to(symbol)`
- [x] `definition_of(reference)` helper
- [x] `visible_symbols_at(offset)` helper
- [x] `body_of(symbol)` helper
- [x] direct APIs tailored for IDE queries instead of raw structure walking

### Diagnostics Readiness

- [x] enough symbol/reference data for basic unresolved-name checks
- [x] enough scope data for basic duplicate/shadowing checks
- [x] enough body data for control-flow-aware diagnostics
- [x] actual semantic diagnostic passes
- [x] unused-symbol tracking
- [x] duplicate-definition diagnostics
- [x] unresolved import / export diagnostics
- [x] doc-type consistency diagnostics

### Completion and Navigation Readiness

- [x] local goto-definition groundwork
- [x] local find-references groundwork
- [x] robust single-file goto-definition helpers
- [x] robust single-file find-references helpers
- [x] visible-symbol completion source
- [x] function-signature / parameter-hint support
- [x] member completion support
- [x] project-aware symbol completion

### Type Inference Readiness

- [x] stable per-expression identity beyond source ranges
- [x] explicit expression result slots that inference can attach types to
- [x] explicit initializer / assignment edges from symbols to expressions
- [x] call-site recording with callee and argument ranges
- [x] argument-to-parameter mapping on resolved calls
- [x] explicit return-value / throw-value collections per body
- [x] basic control-flow merge points for `if` / `switch` / loops
- [x] doc-attached declaration types usable as inference inputs
- [x] function doc signatures usable as inference inputs
- [x] host/project-provided symbol signatures pluggable into HIR consumers
- [x] enough expression/query APIs for `expr -> inferred type` lookups downstream
- [x] literal kind/value-class modeling so downstream inference can distinguish `int` / `float` / `string` / `bool` / `char`
- [x] unary operator kind modeling for operator-driven result typing
- [x] binary operator kind modeling for operator-driven result typing
- [x] richer expression payloads or child-expression edges so downstream inference can reason about array items, object members, index bases, and tail expressions
- [x] direct call argument `ExprId` links in call-site records so parameter propagation does not need to recover expressions from ranges
- [ ] path-segment and module-qualified resolution that inference can trust for `foo::bar` lookups
- [x] field/index write modeling for symbol-receiver mutations like `obj.x = value` and `arr[i] = value`
- [x] nested/compound write modeling for paths like `root.child.x = value` and `obj.x += 1`
- [x] mixed member/index write modeling for chains such as `root.items[i].value += 1`
- [ ] explicit expected-type sites for arguments, returns, declarations, closures, and other contexts that push types into child expressions
- [x] loop/iterator binding edges so `for x in value` can seed element types into `x`
- [ ] richer interpolation / string-part ownership so embedded expressions participate cleanly in type queries
- [ ] object-literal shape metadata that can preserve per-field precision when downstream inference grows beyond map-style fallback typing

### Project Rename Readiness

- [x] local definition and reference tracking
- [x] single-file goto-definition and find-references helpers
- [x] stable file-backed symbol identity suitable for cross-file references
- [x] explicit top-level/indexable symbol extraction
- [ ] import/export linkage that resolves renamed symbols through module boundaries
- [ ] project-wide / cross-file reference resolution
- [x] rename-safe classification of editable symbol occurrences versus plain text
- [x] rename preflight checks for duplicate definitions and scope collisions
- [x] alias-aware rename planning for imports / exports
- [x] direct HIR handoff shape for project-wide rename edit planning

### Indexing and Workspace Search Support

- [x] explicit top-level/indexable symbol extraction
- [x] lightweight file symbol index output from HIR
- [x] container metadata for symbols such as owning function/module
- [x] exported-symbol metadata for workspace indexing
- [x] import/export data shaped for module-graph indexing
- [x] stable enough symbol identity for cross-file index refreshes
- [x] direct support APIs for document symbols and workspace symbols
- [x] clear handoff shape from `rhai-hir` to future `rhai-db` / `rhai-project` indexing

## Notes

- `rhai-hir` is intentionally still single-file focused. Project-wide resolution and host-environment semantics should plug in from higher layers such as `rhai-project` and future database/query infrastructure.
- The current model is already strong enough to start building local navigation and semantic diagnostics experiments, but it is not yet the full downstream-ready semantic layer.
- Once the remaining name-resolution and query-surface items are in place, `rhai-ide` should be able to build goto-definition, references, diagnostics, and basic completion much more comfortably.
