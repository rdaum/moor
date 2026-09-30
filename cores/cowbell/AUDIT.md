# Cowbell audit scope

The maintained checks cover the core source graph and the functional boundaries below. Regression
fixtures use verified wizard, programmer, and ordinary-player principals. LLM scenarios use local
fake responses. No model service is necessary for these checks.

| Area                   | Reviewed boundary                                                                                                                    | Maintained evidence                                                    |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Source and build       | Recursive source discovery, nested additions/deletions, fixture isolation, stable objdef export                                      | `check`, `roundtrip`, `test-build-inventory`                           |
| Style                  | Explicit declarations, initial docstrings, assignments in conditions, verb-body coverage, no new recorded debt                       | `check-style`, `tests/style/converted.txt`, `tests/style/baseline.tsv` |
| Authority and features | Actual caller versus target ownership, direct calls, hostile inherited authorization decisions, role flags                           | Principal, authority, builder, and capability headless scenarios       |
| Native scheduling      | Creator permissions, housekeeping lifecycle/grouped sweep, Henri adaptive cadence, stale exported IDs, committed cancellation        | Native scheduler headless scenarios and `test-wire`                    |
| Relations              | Reader principal, query arguments, index deduplication, absent/malformed input                                                       | Relation headless scenarios and method tests                           |
| Rules and reactions    | Captured request authority, predicate/effect boundaries, delayed effects and errors                                                  | Rule authority and reaction headless scenarios                         |
| World actions          | Container guards, movement hooks, target validity across suspension, room events                                                     | World, object, builder, and session scenarios                          |
| Messaging and clients  | Event audiences, private note policy, mail/direct messages, connections and subscriptions                                            | Messaging, event, and session scenarios                                |
| LLM and agent tools    | Actor spoof denial, owned/public access, foreign private denial, callbacks across suspension, NPC identity, single-use room requests | LLM headless scenarios and tool method tests                           |
| Runtime connections    | Two actual connections for one player, world state and native schedule state after restart, checkpoint export                        | `test-wire`                                                            |

This matrix records a bounded review. It does not promise coverage for all combinations of user
code, feature composition, clients, or third-party tools. Import success establishes syntax and
graph consistency. Behavioral claims require the corresponding runtime scenario.

## Remaining debt

The style baseline retains findings outside converted units. The ratchet rejects new findings and
increased counts. Converted units must have zero findings. The syntax checker does not prove
security or runtime behavior.

Room request authorizations use the existing persistent revocation map. Each consumed job adds one
token id. Retention and cleanup optimization remain deferred.

Explicit capability tests cover selected building and mutation contracts. The tool tests do not
exhaust every area/room grant combination. Broader delegation requires a separately defined target
and action contract.

Browser rendering and Meadow transport require separate client checks. Local fake-model tests cover
core dispatch and authority. They do not evaluate external provider availability or model quality.

Unexpected delivery errors can still occur in user-defined recipients and hooks. Existing fanout
isolation and diagnostics remain part of those contracts. Adding arbitrary third-party code does not
inherit a general sandbox from one permission reduction.
