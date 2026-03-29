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

### Rhai Module Semantics Alignment

- [x] workspace import linkage driven by static string module paths such as `import "provider" as tools`
- [x] import/export indexes aligned with explicit variable exports plus implicit top-level public-function exports
- [x] diagnostics that keep syntactically valid but unresolved dynamic/bare imports visible to IDE consumers
- [x] removal of analyzer-specific “import exported symbol by name” workspace behavior
- [x] static import-expression diagnostics when analysis can prove the module path is not `string`
- [x] current static import-path reasoning covers literal strings, interpolated strings, simple concatenation, block tail values, and `if` branches with consistent string results
- [x] module-qualified member/call queries for statically linked imports such as `tools::helper()` and `tools::VALUE`
- [x] nested module-qualified member/call queries for statically linked sub-modules such as `tools::sub::helper()` and `tools::sub::VALUE`
- [x] unaliased `import "module";` keeps regular module members out of bare-name visibility while still allowing imported typed methods
- [ ] richer handling for dynamic `import <expr>` cases that are valid Rhai but cannot be linked statically in the workspace
- [ ] deeper module-member/query behavior for dynamic sub-modules and other Rhai runtime visibility edge cases

### Host and Project Semantics

- [x] cached host-provided module/function/type inventory from `rhai-project`
- [x] external signature index assembly for downstream type-aware queries
- [x] custom syntax / reserved-symbol policy inputs from project configuration
- [x] workspace-scoped view of engine options and enabled capabilities
- [x] clean boundary between file-local HIR facts and project/engine-provided facts
- [x] builtin/global Rhai function inventory for analyzer-known functions like `blob`, `timestamp`, and `Fn`

### Type Inference

- [x] builtin/host signature knowledge available as inference seeds
- [x] per-file inferred type cache for expressions and symbols
- [x] snapshot query APIs for `expr -> inferred type` and `symbol -> inferred type`
- [x] builtin result typing for `blob`, `timestamp`, and `Fn`
- [x] local function return inference from explicit `return` value flows
- [x] local variable / alias propagation through initializer and assignment flows
- [x] intra-file parameter type propagation from resolved call arguments into local function parameters
- [x] literal-driven inference for core Rhai literals (`int`, `float`, `string`, `char`, `bool`)
- [x] operator-driven inference for core unary/binary expressions
- [x] implicit tail-expression return inference for functions / closures
- [x] fallthrough-aware result joins for `if`, `switch`, block, function, and closure bodies
- [x] field / index / member-aware inference for object literals, maps, arrays, and host-method access patterns
- [x] import/export seed propagation and direct imported-call parameter seeding across files
- [x] host-overload resolution beyond simple name/arity matching
- [x] explicit inference coverage for every lowered Rhai expression kind, including `assign`, `paren`, `path`, `interpolated string`, `while`, `loop`, `for`, and `do`
- [x] mutation-aware value-flow tracking for member/index writes such as `obj.field = expr` and `arr[i] = expr`
- [x] nested/compound mutation tracking for non-trivial lvalues such as `root.child.field = expr`, `obj.field += 1`, and `arr[i] ??= value`
- [x] mixed nested member/index mutation tracking for chains such as `root.items[i].value += 1`
- [x] flow-sensitive symbol-state inference across branches and loops for read positions instead of only joining final expression results
- [x] truthy nullable narrowing for branch-local reads such as `if value { ... }` and `if !value { ... } else { ... }`
- [x] nullable narrowing for equality checks against unit-typed expressions, including negation and simple `&&` / `||` guard composition
- [ ] broader narrowing / refinement rules for `is`-style checks and other Rhai-specific control-flow guards
- [x] shape-preserving object typing beyond `map<string, union<...>>` so field lookups stay precise after aliasing and partial updates
- [x] expected-type propagation from declarations, parameter annotations, return positions, and selected call signatures into child expressions
- [x] closure inference that can derive parameter and return types from expected function signatures and higher-order call sites
- [x] first-class `Fn` / function-pointer inference that tracks referenced local, builtin, and externally indexed callee signatures instead of only the opaque `Fn` type
- [x] script-defined method-call inference for `this`, typed methods, and blanket-method fallback on local calls such as `value.bump(...)`
- [x] imported typed-method inference for bare module imports so calls like `value.bump(...)` can resolve through linked workspace modules
- [ ] generic/applied-type substitution and type-argument inference for host APIs and future analyzer-known abstractions
- [x] loop binding inference for arrays, strings, and numeric ranges, including optional counter bindings in `for` expressions
- [ ] broader builtin container/iterator semantics for method-based iterables and collection transforms
- [x] path-qualified and module-qualified call inference through `foo::bar`, imports, re-exports, and alias chains
- [x] iterative workspace call-graph propagation across local calls, imported exports, re-exports, and recursive strongly-connected components when type information can flow through the cycle
- [ ] ambiguity tracking so incompatible candidate types can be surfaced distinctly from plain `unknown`
- [ ] regression coverage that exercises each inference rule with single-file, cross-file, and incremental-update scenarios

### Query Surface for Downstream Crates

- [x] direct database APIs for `parse(file_id)` and `hir(file_id)`
- [x] direct database APIs for file diagnostics
- [x] direct database APIs for project-aware diagnostics that account for workspace imports / exports
- [x] query helpers for imported global typed-method lookup across linked workspace modules
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
