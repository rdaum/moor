# Capability Security Guide

Cowbell uses signed capability flyweights for explicit delegation. Ordinary ownership, role checks,
and MOO read/write permissions still apply. A tool request does not itself grant access to its
target.

## Capability references

`$root:issue_capability()` returns a flyweight whose delegate is the target object. Its `token` slot
contains a PASETO V4.Local token. Anyone who holds a copy can exercise its encoded authority. Treat
these flyweights as bearer credentials.

The server key authenticates the target, capability symbols, token id, and optional expiration and
`run_as` principal. Only wizard activations can use the server-key token builtin. The issuance verb
first requires the actual caller to own the target or be a wizard.

A privileged consumer captures its incoming principal before nested calls. It calls a canonical root
helper with the explicit subject. It does not ask the target's overridable authorization method
whether that target permits a privileged effect.

```moo
const principal = caller_perms();
const {subject, new_name} = args;
const {target, perms, grants} =
  $root:_check_permissions_with_grants_as(subject, principal, 'set_name_aliases);
set_task_perms(perms, grants);
target.name = new_name;
```

This example belongs in a privileged activation. The helper checks owner/wizard authority or
validates a bearer token before it returns target-specific runtime grants. The consumer reduces
permissions before the effect. Existing `$root:set_name_aliases()` also handles aliases and
validates its input.

For a capability-aware operation that does not need mapped runtime grants, use the canonical helper:

```moo
const principal = caller_perms();
const {subject} = args;
const {target, perms} = $root:_check_permissions_as(subject, principal, 'dig_from);
set_task_perms(perms);
```

The subject must remain the flyweight when the request uses bearer authority. Replacing it with its
raw delegate loses the token. `_challenge_subject(subject, required_caps, key)` validates the token
without a callback to the delegate. It checks authentication, target binding, expiration,
revocation, and the requested capability subset. Ordinary callers use the existing consumer verbs
rather than implement their own token decoder.

## Issuance, storage, and revocation

`$root:issue_capability(target, caps, ?expiration, ?run_as, ?key)` accepts an owner or wizard
caller. An explicit `run_as` must equal the issuer's incoming principal or the current `player`. The
default token has no explicit `run_as`, and challenge returns `$hacker` for that case.

`$root:grant_capability(target, caps, grantee, category)` stores the flyweight in the grantee's
`grants_<category>` map. The category is a symbol. The grantee must have that grant bucket.
`grantee:find_capability_for(target, category)` retrieves the stored flyweight.

```moo
const principal = caller_perms();
const {target_obj} = args;
const cap = principal:find_capability_for(target_obj, 'room);
const subject = typeof(cap) == TYPE_FLYWEIGHT ? cap | target_obj;
const {target, perms} = $root:_check_permissions_as(subject, principal, 'dig_from);
set_task_perms(perms);
```

`$root:revoke_capability(target, grantee, category)` removes the stored grant and records its token
id. Copied flyweights then fail challenge. Merging a stored grant also revokes the replaced token
id. Deleting a local copy does not revoke other copies. Direct bearer tokens remain valid until
their expiration, recorded revocation, or server key rotation. Key rotation invalidates tokens made
with that key.

`$grant_utils:format_denial(target, category, caps)` supplies the existing builder denial message.
Authorization must precede mutations and publication of events. A denied operation must preserve
protected state.

## Actor permissions and tool requests

LLM and agent tools use the authenticated request principal. A supplied actor must match the actual
incoming principal unless an authenticated wizard activation explicitly delegates the request. Agent
ancestry, an object's owner, `player` context, and billing identity do not prove delegation. An
owner can transfer an ordinary object to a wizard, so that ownership alone does not establish trust.

Tool handlers reduce permissions before target inspection, mutation, and untrusted callbacks. They
retain existing explicit room/area capabilities. They do not create blanket read, write, code,
object, or privileged-builtin grants for arbitrary requested targets. Owned private and foreign
public data remain accessible under ordinary MOO permissions. Foreign private data requires its
existing authorized access path. Full object dumps require the builtin's dump authority.

Queued room requests bind the requester to the room, query, and context. Each authorization can
execute once. A room owner cannot replace the requester with a foreign principal. Autonomous
observers act as the NPC itself. Their billing identity can be different from their tool principal.

## Current delegation surfaces

Area/room building uses explicit capabilities for `add_room`, `dig_from`, `dig_into`,
`create_passage`, and `remove_passage`. Passage descriptions use source-room `dig_from` authority.
Stored grant lookup preserves the flyweight until the capability-aware operation consumes it.

Root mutation helpers map validated capabilities to specific runtime operations. Current mappings
include move, recycle, description, name/aliases, owner, thumbnail, and API-key changes. An
unsupported symbol fails instead of supplying broad property or code authority.

Programmer commands retain programmer checks and normal builtin permissions. Inspection commands
retain read/debug permissions. Cowbell does not provide a general bearer capability for arbitrary
property writes or arbitrary code execution. New delegation requires an explicit target and action
contract, denial tests, and tests for copied, revoked, and modified tokens.

## Reaction installation authority

`$reaction:install(host, property, reaction)` requires the host owner or a wizard. It signs the
exact reaction definition, host, property, owner, and wizard status with the existing server PASETO
key. Dispatch runs approved effects as that recorded owner. It does not adopt authority from a later
ownership transfer. State effects cannot write built-in object properties.

The installed value must still match the signed definition on the original property. A copied token
cannot authorize another host or property. Edits, replacement, disabling, and ownership changes stop
old delayed effects. Every effect rechecks the installation, including after a condition yields.
Builder commands use this same installation service under the builder's permissions.

Shipped objects install reactions through the same API from their `initialize` methods. The initial
room initializes its contents once through the configured server startup hook. Later restarts
preserve builder edits. The reaction engine has no list of trusted world objects or alternate
authorization path.

Direct `fire_trigger` requests require the host owner or a wizard. Ordinary visitors cause reactions
through world operations such as opening a container. Native movement callbacks establish entry and
exit events. `$reaction:change` captures old and new values during mutation, then checks thresholds.
Callers cannot supply a fabricated transition. Context participants do not grant execution
authority. Internal dispatch methods check their callers' verb definitions, not just method names or
the `caller` object.

Existing custom and inherited reactions require installation on each host before they run. Reinstall
approved reactions after definition or ownership changes. A server signing-key change also requires
reinstallation. Generic `reaction:execute` calls retain the caller's authority and do not consume
installation grants.

## Runtime boundaries

`set_task_perms()` changes the current activation. A nested wizard-owned verb starts another
privileged activation. Each privileged consumer must authenticate its incoming request. Ordinary
actor operations and untrusted callbacks require permission reduction. An explicit capability path
can retain necessary framework permissions for its bounded mutation, such as an area's protected
passage relation. This does not grant the actor general framework authority. Permission reduction
does not sandbox the whole call tree.

The runtime does not interpret Cowbell capability tokens. Core helpers validate them and map
supported operations to runtime grants. The runtime then enforces those grants. Flyweights are
immutable. A copy retains the same bearer token and authority.

A suspension commits the current transaction. Request principals must remain local across that
boundary. Mutable actor or billing properties cannot replace the authenticated requester after
resumption.

## Tests and audit limits

`tests/headless/headless_capability_scenarios.moo` covers issuance, grants, revocation, copied
bearer denial, modified tokens, merges, setup capabilities, and selected command wrappers.
`tests/headless/headless_authority_scenarios.moo` covers direct calls and inherited authorization
overrides. `tests/headless/headless_llm_scenarios.moo` uses local fake models for actor spoofing,
private access, callback interleaving, room request replay, and NPC identity. It makes no external
model requests.

These cases cover the maintained contracts. They do not prove all future feature combinations or
third-party tools. The [audit matrix](AUDIT.md) records the bounded review and remaining debt.

Consumed room requests use the existing persistent token revocation map. Each consumed job records
one token id. Retention and cleanup optimization remain deferred.

## Credit

This implementation draws on Quantum-Vacuum's capability implementation for ColdMUD frobs in the
1990s. Those values serve a role similar to mooR flyweights.
