# rhai-syntax

`rhai-syntax` is the syntax front-end for `rhai-analyzer`.

Its job is to turn Rhai source text into:

- tokens with stable text ranges,
- a resilient syntax tree,
- syntax errors that survive incomplete code.

It does not do name resolution, type inference, or host-environment modeling.

## Syntax Coverage Checklist

This checklist is based on the Rhai Book language reference and appendix pages for version 1.24.0.

### Lexer

- [x] whitespace and line comments
- [x] nested block comments
- [x] identifiers and `_`
- [x] integer and float literals
- [x] string, raw string, back-tick string, and character literals
- [x] core keywords
- [x] punctuation and separators
- [x] core operators, assignment operators, ranges, safe-navigation tokens
- [x] doc comments, shebangs, and trivia handling polish
- [ ] reserved symbols and custom-operator hooks

### Expressions

- [x] names and basic literals
- [x] parenthesized expressions
- [x] blocks
- [x] function calls
- [x] binary arithmetic expressions
- [x] unary expressions
- [x] assignment and compound assignment
- [x] comparisons and boolean operators
- [x] ranges
- [x] indexing and safe indexing
- [x] property access and safe property access
- [x] arrays
- [x] object maps
- [x] string interpolation internals
- [x] closures / function pointers
- [x] `if` as expression
- [x] `switch`
- [x] `while` / `loop` / `for`
- [x] `do` / `until`
- [x] `break` / `continue` / `return` / `throw`
- [x] `try` / `catch`

### Statements and Items

- [x] `let`
- [x] `const`
- [x] `import` / `export` / `as`
- [x] function definitions
- [x] `private` modifier on functions
- [x] `import` statements with arbitrary expression module specifiers
- [x] `export` statements restricted to plain identifiers plus `export let` / `export const`

### Rhai Function Syntax Alignment

- [x] top-level `fn name(params) { ... }` definitions
- [x] global-only function definitions (reject nested `fn`)
- [x] method-call syntax such as `object.method(args...)`
- [x] Elvis method-call syntax such as `object?.method(args...)`
- [x] typed method definitions such as `fn Type.method(args...) { ... }`
- [x] quoted typed method definitions such as `fn "Custom-Type".method(args...) { ... }`
- [x] caller-scope function-call syntax such as `foo!(args...)` / `call!(fn_name, args...)`
- [x] syntax-level rejection/recovery for invalid caller-scope forms like `object.method!()` and `module::func!()`

### Rhai Module Syntax Alignment

- [x] `import <expr> [as alias]` grammar, including dynamic module-path expressions
- [x] `export <name> [as alias]` grammar for plain identifier targets
- [x] `export let ...` / `export const ...` shorthand declarations
- [x] reject non-identifier export targets such as paths, calls, and other expressions
- [x] reject `export` statements outside global level
- [ ] targeted recovery/messages for `import` expressions that are syntactically valid but semantically suspicious

### Parser Quality

- [x] `TextRange` on all tokens/nodes
- [x] basic error recovery for missing expressions
- [x] richer recovery around delimiters and statement boundaries
- [x] precedence coverage matching Rhai built-ins
- [x] snapshots for representative valid programs
- [x] snapshots for representative broken programs

## Notes

- The current parser covers the core Rhai language surface needed for IDE work, but it does not yet model host-defined custom syntax.
- `rhai-syntax` owns tokenization, resilient parsing, and typed AST wrappers. Name resolution and host-environment semantics belong in higher layers.
- Reserved-symbol policy and custom-operator extensibility are intentionally left open because they likely need project-level configuration from `rhai-project`.
