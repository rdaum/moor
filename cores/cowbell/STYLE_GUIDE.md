# Cowbell MOO programming style

This guide applies to Cowbell source and test fixtures. Use the
[mooR book source](../../book/src/SUMMARY.md) for language and runtime semantics.
Cowbell retains flyweights, structured events, feature composition, LLM tools, and web clients.

## Bindings and values

Declare local bindings explicitly. Use `const` for unchanged bindings and `let` for mutable bindings.
Keep each binding in the smallest useful scope. Loop and exception bindings use their language syntax.
A declaration inside a branch or loop creates a new binding. Update an outer accumulator with an
assignment. Do not redeclare it inside the loop.
Prefer argument scattering when the argument contract fits its required and optional bindings.
The scatter supplies the argument-count check. Destructure arguments after the initial docstring:

```moo
const {target, text, ?urgent = false} = args;
```

Use `true` and `false` for predicates and flags. Keep integers for counts, indexes, and integer builtin options.
Check the builtin contract before changing a numeric option. For example, `match` and `rmatch` require integer case flags.
Do not change an established sentinel or return value without updating its callers and tests.
Objects and flyweights are false in MOO conditions. Use explicit type and validity checks for optional
object references. Do not use `object || fallback` to select an object. Convert mixed numeric types
explicitly when an operation requires matching numeric types.

Use symbols for fixed record keys and identifiers. Use strings for user text.
Use maps for named records and lists for ordered values. Document positional tuples at their interface.
Keep flyweights where their prototype behavior or compact values serve the contract.
Do not replace structured events with formatted text inside reusable operations.

Use comprehensions for simple transformations and filters. Use loops for effects and early termination.
Prefer an existing builtin after checking its semantics. Parse literal data with `fromliteral()`, rather than `eval()`.
MOO string equality ignores case. Use `strcmp()` when the contract requires case-sensitive comparison.

## Layout and contracts

Use two spaces per indentation level. Keep ordinary lines within 100 columns where practical.
Use `snake_case` for local names, properties, methods, and files. Keep core object constants uppercase.
Preserve established command names and aliases. An internal `_` prefix does not restrict access.
Keep core attribution in `LICENSE`. Do not add per-file copyright notices to exported objdef sources.

Start each method or command with a string-literal docstring. State its purpose and result.
Document argument defaults, absence results, errors, authority, and transaction boundaries where relevant.
Command docstrings state usage and behavior. Comments explain intent or a constraint, rather than repeating code.
Remove historical commentary from rewritten bodies. Keep claims about performance tied to measurements.

Use objdef `method` for callable methods. Its default argspec is `this none this`; its default flags are `rxd`.
Keep `d` enabled in new and rewritten code. Explain explicit flags that differ from those defaults.
Use meaningful argspecs for command verbs. Give an `x` flag a reason in the public or dispatch contract.
Keep command parsing and output separate from reusable operations.

## Guards and errors

Prefer early returns and shallow control flow. Prefer short-circuit guards for validation and single conditional actions:

```moo
valid(target) || raise(E_INVARG, "Target does not exist.");
length(messages) == 0 && return {};
urgent && target:tell(event);
```

`&&` and `||` have equal precedence in MOO and associate left to right.
Thus `a || b && c` means `(a || b) && c`. Use parentheses to make dependent checks clear:

```moo
(typeof(limit) == TYPE_INT && limit > 0) || raise(E_INVARG, "Invalid limit.");
```

Keep each guard focused on one requirement and consequence. Use `if` branches for several statements.
Do not assign variables inside conditions. Name complex predicates instead of chaining unrelated effects.

Raise exceptions for failed method operations. Document normal absence results separately.
Catch expected errors at command boundaries and translate them into useful output.
Propagate unexpected errors with their diagnostics. Catch specific errors instead of `ANY` by default.
Use error-catching expressions only for narrow, documented fallbacks.
Do not turn failed authorization, malformed input, or unexpected defects into apparent success.

Delivery fanout can isolate a broken recipient. Preserve diagnostics and distinguish expected denials from unexpected defects.
LLM tool handlers must preserve operation errors. A tool response must not claim that a failed mutation succeeded.

## Principal, target, and authority

Document the caller, effective principal, target, and allowed operation for each privileged interface.
Check authorization before effects. Verb ownership and target ownership are separate facts.
`player` identifies task context; it does not prove authority. Use `caller_perms()` and the actual operation contract.
An ordinary activation starts with its verb owner's permissions. Capture the incoming principal
before a nested call can replace that context. Invoke authorization decisions on canonical helpers
with an explicit subject and actor. A target's overridable helper cannot authorize a privileged effect.
Ownership of an object does not establish the provenance of its inherited or overridden verbs.

Where the contract permits it, reduce permissions before invoking untrusted callbacks or user-owned behavior.
`set_task_perms()` changes the current activation. A later wizard-owned call creates another privileged activation.
Permission reduction does not sandbox the whole call tree. Narrow grants do not automatically propagate through ordinary calls.
See the [permission reference](../../book/src/the-moo-programming-language/task-permissions-and-capability-grants.md).

Feature installation supplies behavior; it does not grant programmer or wizard authority.
Keep role checks at command and method boundaries. Check direct calls, inherited behavior, and feature dispatch.
Treat LLM requests and web client requests as untrusted input. Their routing context does not establish authorization.
Document which principal each tool uses and which targets it may modify.
Use the authenticated request actor for tool work. Preserve that actor in a local across suspensions.
Billing identity is separate. Require existing explicit capabilities for delegated access. Do not mint
blanket target grants from a tool request. Authenticate queued requests before state effects or callbacks.
The actual command dispatcher uses a `caller_perms()` sentinel. Use its verified caller/player
contract only at that boundary. A callable method must still validate its actual incoming principal.

Treat user object identifiers as opaque. Preserve numbered, UUID, anonymous, and flyweight contracts where the interface accepts them.
Do not infer control from object-number ordering. Avoid numeric scans for UUID user objects.

## Transactions and performance

**Every `suspend()` commits the current transaction, including `suspend(0)`.**
Execution resumes in a new transaction. Later errors cannot undo earlier committed changes.
`fork` also commits the parent transaction before child dispatch. The parent then continues in a new transaction.
`read()` and helpers that suspend or fork also cross transaction boundaries.
`suspend_if_needed()` commits when its threshold causes suspension.

Keep related mutations in one transaction. Put necessary boundaries between complete units with defined intermediate states.
Document every intentional boundary and helpers that can introduce one.
After resumption, reread state and revalidate authority, target validity, and other assumptions.
Local variables can retain values from the previous transaction.

Remove unnecessary suspension instead of treating it as a routine scheduler yield.
Use measured batch sizes and configured tick and time limits for long operations.
Keep work bounded without adding commits between mutations that must be atomic.
Ordinary notification output is buffered until commit and discarded on rollback.
A caught exception does not itself undo mutations already made in the current activation.

Keep external effects and retry behavior explicit. Distinguish committed world state from delayed delivery or external tool results.
Avoid shared bookkeeping writes on every command. Measure concurrency, transaction retries, persistence costs, and response time.

## Events, tools, and clients

Preserve event prototype behavior, structured payloads, subscriptions, reactions, and feature composition.
Document event schemas and delivery scope. Keep authorization checks before event publication and target mutation.
Use the established output interface so that gagging, attribution, and client routing remain effective.
Choose whether output goes to one connection, a player, or an event audience deliberately.

Keep text rendering at the appropriate client or output boundary. Web clients may consume structured values.
The `notify` content type is a symbol; metadata is a map when rich notification support is enabled.
Check host configuration and caller control before using rich notifications.
Do not replace retained structured output with a text-only policy.

Use local functions for helpers confined to one method. Document callback arguments, results, and error propagation.
Keep LLM tool schemas aligned with actual arguments and result contracts.
Test denied operations as well as successful ones. Use isolated fixtures with explicit cleanup.
Import success does not establish runtime behavior.

## Automated checks and recorded debt

`make check-style` recursively compiles source and audits each verb body with the compiler syntax tree.
The checker reports implicit locals, missing initial docstrings, and assignments inside conditions.
It checks body coverage against the compiled objdef graph. It does not prove permission safety or behavioral correctness.

`tests/style/baseline.tsv` records starting findings by relative file, verb declaration, kind, detail, and count.
Line numbers are excluded so that unrelated line shifts do not reset the record.
The baseline check rejects new findings or increased counts.
`tests/style/converted.txt` lists source units that must pass a separate strict check. Removed findings need no replacement allowance.
Do not regenerate the baseline to accept new debt. Reduce its entries after verified cleanup.

Use `--strict --check relative/file.moo:method_name` to require zero findings in a converted method.
Repeat `--check` for several methods. A directory or file selection checks all its methods.
The checker still compiles the whole source tree for constant resolution and body coverage.
An empty selection fails. Strict conversion checks supplement the baseline. A recorded finding is not approval for new code.

The shared checker is in `../tools/style-audit`; do not duplicate its crate for Cowbell.
Record baseline changes deliberately with `--write-baseline FILE`, then review the affected records.
