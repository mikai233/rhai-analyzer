# rhai-syntax

`rhai-syntax` is the syntax front-end for `rhai-analyzer`.

It is responsible for turning Rhai source into:

- tokens with stable ranges
- a resilient syntax tree
- syntax errors that remain useful while code is incomplete

It does not do name resolution, type inference, or project/host modeling.

## Implemented Features

### Lexing and Parsing

- Core Rhai tokens, trivia, comments, shebangs, and documentation comments
- Literal forms including numeric, string, raw string, back-tick string, and character literals
- Core operators, assignment operators, ranges, safe-navigation operators, and separators
- Error-tolerant parsing with range-preserving recovery around expressions, delimiters, and statement boundaries

### Expression and Statement Coverage

- Names, literals, parens, blocks, arrays, object maps, indexing, and property access
- Unary/binary expressions, assignments, comparisons, boolean logic, and ranges
- Calls, closures, function pointers, interpolation internals, and control-flow expressions
- `if`, `switch`, `while`, `loop`, `for`, `do`/`until`, `try`/`catch`, `break`, `continue`, `return`, and `throw`
- Top-level items such as `let`, `const`, `import`, `export`, `fn`, and `private fn`

### Rhai Semantics Alignment

- Global-only function definitions, with nested `fn` rejected early
- Method-call syntax and Elvis method-call syntax such as `object.method(...)` and `object?.method(...)`
- Typed method declarations such as `fn Type.method(...)` and `fn "Custom-Type".method(...)`
- Caller-scope call syntax such as `foo!(...)` and `call!(...)`, with invalid forms rejected
- `import <expr> [as alias]` grammar, including dynamic import expressions
- `export <name> [as alias]`, `export let`, and `export const`, with non-identifier targets rejected
- Global-level restriction for `export`

## Current Boundaries

- Semantically suspicious but grammatically valid `import <expr>` forms are only partially diagnosed here; richer validation lives in higher layers.
- Reserved-symbol policy and host-defined custom syntax are intentionally left open because they depend on project configuration.

## Next Steps

- Better recovery/messages around valid-but-suspicious import expressions
- Reserved symbol and custom-operator hooks driven by project configuration
