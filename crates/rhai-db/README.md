# rhai-db

`rhai-db` is the incremental analysis database for `rhai-analyzer`.

It sits between raw file/project inputs and the higher-level IDE facade, and is responsible for keeping semantic state reusable, cross-file aware, and cheap to query.

## Implemented Features

### Database and Snapshot Model

- In-memory application of file changes and project-configuration updates
- Stable file identity through `rhai-vfs`
- Immutable snapshots for read-heavy IDE workloads
- Explicit dependency slots for parse, HIR, indexing, and query-support inputs
- Incremental invalidation for text changes, project changes, and file removal

### Per-File and Workspace Analysis

- Cached parse/HIR/diagnostic results per file
- Cached document symbols, navigation targets, and completion support data
- Workspace symbol indexing and module-graph storage
- Cross-file import/export linkage and workspace dependency graphs
- Project-wide reference and rename planning inputs

### Rhai Module and Host Semantics

- Static-string module linking for Rhai imports such as `import "provider" as tools`
- Import/export behavior aligned with Rhai variable exports and implicit public-function exports
- Diagnostics for unresolved or non-string import expressions when analysis can prove the problem
- Builtin/global Rhai function inventory
- Builtin type-member inventory for standard receiver types such as `string`, `array`, `map`, `blob`, `timestamp`, `range`, and numeric/character primitives
- Project/host-provided modules, functions, constants, and types sourced from `rhai-project`

### Type Inference

- Expression and symbol type caches exposed through snapshot queries
- Literal, operator, block, branch, loop, assignment, path, and interpolated-string inference
- Local function return inference and local variable/value-flow propagation
- Cross-file seeding through imports, re-exports, direct imported calls, and workspace call-graph propagation
- Host overload selection and ambiguity tracking
- Mutation-aware and flow-sensitive inference for member/index reads and writes
- Object shape preservation instead of immediate fallback to broad map-style unions
- Expected-type propagation through declarations, calls, returns, object literals, and closures
- Closure, function-pointer, typed-method, imported-method, and caller-scope call inference
- Generic/applied-type substitution for host/module APIs, including receiver-driven specialization for generic host methods
- Receiver-aware builtin container semantics for arrays, maps/objects, and iterable-returning builtin methods
- Branch-local narrowing for nullable checks, `type_of(...)` guards, `switch type_of(...)`, and member/index reads

### IDE-Facing Query Surface

- Snapshot APIs for syntax/HIR, diagnostics, symbols, completion inputs, navigation, rename planning, and debug inspection
- Completion inputs that merge visible symbols, project symbols, object fields, builtin members, and host type members
- Imported typed-method lookup across linked workspace modules
- Query shapes designed for `rhai-ide` instead of raw storage walking

### Reliability and Observability

- Cache invalidation and incremental rebuild tracking
- Change-impact reporting for downstream schedulers
- Query-support warming and cache budgeting/eviction
- Unit tests for single-file, cross-file, and incremental behaviors
- Debug/performance inspection hooks for cached state and rebuild activity

## Current Boundaries

- Dynamic `import <expr>` cases that are valid at runtime but not statically linkable are still modeled conservatively.
- Narrowing/refinement rules are strong in common Rhai cases, but not yet exhaustive.
- Some builtin container/iterator transforms still collapse to broader result types than ideal.
- Regression coverage is broad, but not yet arranged as a full single-file/cross-file/incremental matrix for every inference family.

## Next Steps

### Type Inference

- Broader Rhai-specific narrowing and refinement rules
- More builtin container/iterator semantics, especially additional transforms and property-style members
- Deeper handling for runtime-only dynamic module boundaries
- Continued strengthening of regression coverage for major inference families

### Database and Workspace Modeling

- Clearer static-vs-runtime separation for dynamic imports and sub-modules
- More predictable degradation paths for queries that cross runtime-only module boundaries
