# Testing Snore Core

Run checks from the repository root with the repository's stable Rust toolchain. All worlds used by
these tests are disposable. The launcher state under `gen.monolith/` and `gen.run/` is not opened.

## Acceptance commands

```sh
make -C cores/snore check
make -C cores/snore test-wire
make -C cores/snore test-extraction
make -C cores/snore measure
cargo test -p moor-daemon snore_oauth_boundary
```

`check` runs its targets sequentially because the method and session suites share `gen.testsrc/` and
`gen.test.log`. Do not start separate make processes against those paths at the same time.

| Target                      | Coverage                                                                                                                        |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `check-style`               | Parser-backed explicit bindings, initial contracts, and assignments in conditions. This is not a correctness or security proof. |
| `test`                      | Method contracts, permissions, UUID lifecycle, utilities, storage, editors, and interleaved operations.                         |
| `test-headless`             | Commands through the compiler's headless scenario driver.                                                                       |
| `test-session`              | Default and game-player command scenarios with mock connections.                                                                |
| `test-harness`              | Runner assertions, builtin map keys, eval errors, and rejection of command exceptions.                                          |
| `roundtrip`                 | Fresh source import, export/reimport, and an exact export comparison.                                                           |
| `test-collection-roundtrip` | Execute a saved captured closure after export and reimport.                                                                     |
| `test-wire`                 | Real TCP login, guests, concurrent connections, output isolation, reconnect, and authoring.                                     |
| `test-extraction`           | Isolated destructive extraction, recovery after restart, and exported-world reimport.                                           |
| `measure`                   | Storage and interaction workload measurements; timing is environment-dependent.                                                 |

A suite that discovers no tests is a failure. The Makefile also rejects a method filter that selects
nothing and a build that produces no export.

For a single method, use its exact name:

```sh
make -C cores/snore test TEST_FILTER='#100:test_mail_delivery'
```

When the runtime is already built, `MOORC=/absolute/path/to/target/debug/moorc` skips cargo startup
for compiler-driven targets. Keep that binary current with the source being tested.

## Fixtures

The method suite lives on `TEST_HARNESS` (#100). Test principals and entry points use numbered IDs
because the current compiler harness does not discover UUID test objects. Scenarios create UUID
accounts and objects to exercise their actual lifecycle.

`tests/fixtures/` is copied over the shipped `src/` tree into `gen.testsrc/`. Fixture constants are
appended only to that overlay. Fixtures never enter a release export.

Keep regressions that exercise supported behavior, authority denials, and transaction boundaries.
Standalone runtime findings and old editor reproductions live in
[docs/runtime-reproductions](../../../docs/runtime-reproductions/README.md), outside these suites.
Their recorded failures are not accepted core-test results.

## Session scenarios

`cores/tools/session-runner` provides the shared session implementation and extra directives for both cores.
Pending output belongs to one transaction; committed output retains its
recipient and accumulates until drained. Presence is shared across sessions and forks. The mock
assigns one synthetic connection per player and retains it across task forks. It does not model
multiple connections per player, connection attributes, or elapsed time. TCP tests cover multiple
simultaneous connections.

| Directive                                  | Meaning                                                                             |
| ------------------------------------------ | ----------------------------------------------------------------------------------- |
| `@wizard`, `@programmer`, `@nonprogrammer` | Select the fixture principal                                                        |
| `; code`                                   | Evaluate code and check its result                                                  |
| `% command`                                | Submit a player command                                                             |
| `> code`                                   | Continue the preceding program line                                                 |
| `& code`                                   | Evaluate code and discard its result                                                |
| `>> text`                                  | Queue input for the selected player's next `read()`                                 |
| `@@ verb [object]`                         | Invoke a hook on `#0`, defaulting its player and argument to the selected principal |
| `=text`                                    | Require an exact match for the next output line                                     |
| `~~text`                                   | Require the next output line to contain this text                                   |
| MOO expression                             | Compare the command or eval result with this expression's value                     |

Queue input before the command that consumes it. `>>` without text queues an empty line. The driver
stops on the first failure and reports its source line and underlying error. `make test-harness`
checks these contracts and the runner's handling of expected errors and command exceptions. Hook
scenarios exercise core behavior; `make test-wire` checks actual login and TCP delivery.

## Validation snapshot

On 2026-09-25 (UTC), the acceptance commands above passed with the repository's stable toolchain:

- 105 method tests, two headless scenarios, 42 default sessions, and three game sessions.
- Seven runner unit tests, expected-error handling, and deliberate command-exception rejection.
- Real TCP login and authoring, four extraction scenarios, and extraction restart/export/reimport.
- Exact shipped export/reimport comparison and execution of a saved captured closure.
- The Snore daemon OAuth boundary regression and the measurement workloads.

The shipped export contains 95 objects, 1,620 verbs, 1,573 properties, and 464 property overrides.
The style check reports no findings across all 1,620 verb bodies. Both core-local Rust tools pass
Clippy with warnings denied. Formatting, launcher syntax, and documentation file links were checked.
Timing results are local measurements, not performance guarantees.

## Validation limits

The suites cover representative player and administrative workflows. They do not prove arbitrary
custom hooks safe, cover every MOO program, or establish serializable isolation. The object-count
quota alternative remains unavailable because native quota accounting is missing. Tests of the
supported byte-quota policy are not evidence for native object quotas.

The [transaction notes](../docs/transactions.md) describe partial-operation and extension limits.

## Connection output checks

The method suite checks missing-session behavior and unauthorized explicit delivery. Session-runner
unit tests check connection identity, output destinations, rollback, forks, and disconnects.

The TCP suite uses two connections to the same player. It checks current and explicit destinations,
line-list output, player-wide broadcasts, foreign and stale targets, gagging, caller attribution,
help and who listings, input prompts, and connection-local quit. Delivery markers on both streams
bound the output checked for leaks. These checks do not establish complete presence-hook coverage
for timeout, boot, or web reattachment paths.

The 2026-09-25 connection-output follow-up passed `make check` with 106 method tests, 42 default
sessions, three game sessions, and eight runner unit tests. The extended TCP suite and runner Clippy
also passed. The shipped style check reports 1,623 verb bodies and no findings.
