# Persistence stage 4: readable literal and source codecs

The compiler owns the codecs. They have no storage dependency. Fjall keeps its binary persistence
format and does not call these codecs on its write path.

## Public API

`read_persistent_literal` and `write_persistent_literal` exchange `Var` values and MOO text.
`read_persistent_source` and `write_persistent_source` exchange `ProgramType` values and MOO source.
Every operation takes a `SourceProfile` and returns a typed error.

Writers render into a temporary buffer and validate the result before writing to the caller's
buffer. Literal validation parses the value and compiles any lambda bodies. Source validation
recompiles the whole program. A profile, rendering, or validation failure leaves the caller's buffer
unchanged. An output sink can still fail during the final write.

`PersistentProgram` carries source, the complete profile, and a diagnostic compiler identifier. Its
`decode` method compiles the source. It has no durable bytecode cache.

Literal decoding has an empty constant context. It does not evaluate expressions, read files,
resolve object aliases, or accept include macros. Lambda source compiles without objdef constant
substitution. Normal language expressions inside a lambda remain executable code; decoding does not
run them.

The existing 64-level literal nesting limit also covers rich error payloads. Encoding checks nested
containers, captures, program constants, and nested lambda programs before rendering.

## Version 1 grammar

The literal version, source version, and compiler-profile version are each `1`.

| Value            | Canonical text                                                                                            |
| ---------------- | --------------------------------------------------------------------------------------------------------- |
| Integer          | Signed decimal, including both `i64` limits                                                               |
| Boolean          | `true` or `false`, distinct from integers                                                                 |
| None             | `None`                                                                                                    |
| Finite float     | Shortest round-tripping decimal with a decimal point or exponent; negative zero is `-0.0`                 |
| Non-finite float | `f"HHHHHHHHHHHHHHHH"`, exactly 16 uppercase hexadecimal IEEE-754 bits                                     |
| String           | Quoted UTF-8 with the escapes below                                                                       |
| Symbol           | `'name`, or `'"quoted spelling"` when the spelling is not an identifier                                   |
| Numbered object  | `#42`, `#-1`                                                                                              |
| UUID object      | `#048D05-1234567890`                                                                                      |
| Anonymous object | `#anon_048D05-1234567890`                                                                                 |
| List             | `{value, value}`                                                                                          |
| Map              | `[key -> value]`, with typed keys                                                                         |
| Binary           | `b"..."`, padded URL-safe base64                                                                          |
| Builtin error    | `E_TYPE`, with optional message and attached value                                                        |
| Custom error     | `e"exact spelling"`, with optional message and attached value                                             |
| Flyweight        | `<#42, .name = value, {contents}>`; arbitrary slot names use `."quoted name"`                             |
| Lambda           | `{parameters} => expression` or `fn (parameters) statements endfn`, followed by optional closure metadata |

The `f"..."` form accepts only infinities and NaNs. It preserves sign and NaN payload bits,
including signalling NaNs. Bare `f`, `e`, `inf`, and `nan` remain ordinary identifiers. Decimal
overflow is an error. Float construction preserves IEEE bits; arithmetic retains its separate range
checks.

Strings use `\x00` for NUL, `\n`, `\r`, `\t`, `\"`, and `\\` for their corresponding characters.
Other C0 controls and DEL use uppercase `\xNN`. Ordinary Unicode stays UTF-8. Readers also accept
the supported `\0` and four-digit `\uNNNN` escapes. Unknown escapes, incomplete escapes, raw NUL,
and unsupported eight-digit `\U` escapes are errors.

Errors distinguish an absent message from an empty message, and an absent payload from a `None`
payload. Examples are `E_TYPE`, `E_TYPE("")`, `E_TYPE(None, None)`, and
`e"MixedCase"("detail", [1 -> "value"])`. A custom code that spells a builtin name remains custom.
The two-argument error form contains literal data. The existing one-argument dynamic message form
continues to compile as an expression.

Object parsing checks representable identity bits rather than masking excess bits. Object keys for
the PostgreSQL adapter must additionally compare input text with `Obj::to_literal()` to reject
noncanonical keys. The general value parser permits alternate accepted spellings; writers emit
canonical text.

Quoted flyweight slot names can preserve names that collide with synthetic accessors, including
`delegate` and `slots`. Ordinary unquoted slot declarations retain their existing restrictions.
Objdef export does not generate object aliases named `NONE`, `TRUE`, or `FALSE`, which are literal
keywords. It keeps the objects and their file names.

## Closure binding identity

Closure metadata has this grammar:

```text
with captured [{binding: value, binding: value}, {}, {binding: value}] self binding
```

Both `captured` and `self` are optional within the suffix. Frame order is lexical depth. Empty
frames and `None` entries remain explicit. Duplicate bindings in one frame are errors, including
duplicate `None` entries.

The writer gives ambiguous lexical variables distinct source identifiers. It applies the same
substitution to parameters, body references, capture entries, and the recursive binding. Unnamed
frame slots receive identifiers that cannot collide with declared variables. Ordinary unambiguous
names remain readable.

The reader declares capture bindings in nested compiler scopes, compiles the body, and resolves each
binding in the resulting name table. It allocates the new capture slots from that table. It does not
copy old offsets. Recursive metadata names the actual binding; unknown or ambiguous self bindings
are errors. The number of capture frames is preserved, while slot offsets can change.

Optional defaults are executable conditional assignments in the compiled lambda body. Decompilation
retains their guards. Recompilation therefore preserves both omitted and supplied arguments,
including the runtime's existing treatment of a supplied zero. It does not duplicate default code.

Capture analysis runs during lowering, using the lambda's parameter scope ID as its lexical
boundary. The AST carries the resolved bindings into code generation. It includes default
expressions and lambda call targets. Named functions declare their recursive binding before lowering
their body.

The ordinary compiler accepts the same closure suffix as a scalar literal in program source. A
lambda that returns another lambda uses block form when needed to make suffix ownership explicit.

## Compiler compatibility policy

Version 1 supports exactly one database profile:

- Language: `moo`.
- Literal, source, and compiler-profile versions: `1`.
- Enabled: flyweights, booleans, symbols, custom errors.
- Disabled: unsupported-builtin rewriting and legacy type constants.

These settings are explicit in `SourceProfile`; they do not follow future `CompileOptions` defaults.
Readers reject any other profile before parsing. The originating package/build identifier is
informational and does not control compatibility.

A compiler change that cannot retain this profile's interpretation must introduce another profile
version. Keep the old implementation or require an explicit source conversion. Do not silently
reinterpret stored rows or change the database profile while source remains under the old profile.
Future PostgreSQL metadata must store the complete profile and check it before decoding rows.

## Suspended-task audit

`kernel::tasks::convert_task::moo_stack_frame_to_flatbuffer` stores each activation's own program,
program counter, environment, value stack, scope stack, and handler state. The inverse function
loads that program from the task record. It does not fetch or recompile the current world verb. Task
and suspended-task records also check `CURRENT_TASK_VERSION`.

This permits world verbs to use source persistence while suspended tasks retain their exact
execution image. It does not make old task bytecode compatible with arbitrary future VM changes. A
bytecode or frame-layout change must retain the task decoder or reject an incompatible task version.
The PostgreSQL backend qualification must still include suspend/restart/resume across verb
replacement. The task store remains separate from world persistence.

## Validation and measurement

The literal corpus uses recursive structural comparison, including float bits, symbol spelling,
error payload presence, typed map keys, and flyweight slots. It includes 4,096 deterministic float
patterns, 128 generated nested-value cases, malformed literals, profile rejection, and output-buffer
failure boundaries.

The runtime corpus compares original and reloaded closure behavior. Cases include required,
optional, and rest parameters; shadowing; multiple frames; captured `None`; nested lambdas;
recursive functions; and closures embedded in source. Source tests execute recompiled programs,
including exact non-finite float bits. Uninitialized `None` captures preserve the runtime's
`E_VARNF` behavior.

Regressions check the 64-level error nesting limit with uppercase, lowercase, and mixed-case codes.
Parameterless closures retain captures across sibling scopes and repeated persistence reloads.

The branch also includes the fix for [#554](https://github.com/timbran-project/moor/issues/554).
Compiled lambdas record their entry scope count, including empty enclosing scopes. Stored program
format 6 preserves that layout. Execution and storage tests cover parameterless lambdas without
captures. Affected older bytecode needs recompilation.

`cargo run --release -p moor-compiler --example persistence-codec-cost` measures rendering,
decoding, and validated encoding separately. It reports three samples of 1,000 operations after
warmup. These are codec CPU costs, not database throughput or durability measurements. Results are
recorded in `docs/benchmarks/persistence-stage-4-codec-cost.json`.

| Case                             | Text bytes | Render, µs | Decode, µs | Validated encode, µs |
| -------------------------------- | ---------: | ---------: | ---------: | -------------------: |
| Integer                          |          2 |      0.011 |      0.070 |                0.105 |
| 1,024-element list               |      5,034 |      8.365 |     72.963 |               86.117 |
| Closure with capture and default |        238 |      2.114 |     21.416 |               24.957 |
| Program with lambda              |        130 |      2.302 |     11.586 |               14.369 |

These are medians from one CPU-pinned run on aarch64 before the rebase that included #554. The
artifact records the measured source hashes. They are examples for sizing codec work, not workload
throughput targets.

The combined check after rebasing onto the #554 fix passed **1,624 tests**, with six existing tests
ignored:

```sh
cargo test -p moor-compiler -p moor-schema -p moor-db -p moor-kernel -p moor-daemon -p moor-objdef -p moor-var -p moor-vm --lib --tests
cargo clippy --workspace --all-targets --all-features
cargo +nightly fmt -- --check --config reorder_imports=true,imports_indent=Block,imports_layout=Mixed
```

Workspace Clippy passed. Its only notice concerns the existing `proc-macro-error2` dependency's
future Rust compatibility. Rust formatting, document formatting, and `git diff --check` passed.

The stage 3 Fjall qualification remains in `docs/persistence-stage-3.md`. Its comparison is against
steps 1–2. The original pre-refactor performance gate remains separate and open.
