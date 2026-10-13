# Rule and reaction data

A rule checks a condition, such as whether a visitor can open a cupboard. A reaction responds to
something that happened, such as a cupboard opening. It checks its condition and performs its
actions. Relations store facts that conditions can query, such as the members of a club.

For example: when the cupboard opens, if the cat is nearby, make the cat stand and announce a
message. The opening supplies the cupboard, the person who opened it, and the location. The
condition checks where the cat is. The actions request the stand operation and send the message.

This is a reference for core programmers. It describes the stored values and checks used by the
implementation. The [builder's guide](reactions.md) explains the command interface with working
examples. Builders do not need these internal records.

## Why describe these parts as data?

A condition tree preserves grouping such as `A AND (B OR C)`. Parsing, checking, evaluation, and
printing use the same tree. This prevents displaying a rule from changing its meaning.

Argument declarations describe what an operation needs and what a query returns. For example,
`occupants` needs a location and returns objects. Those declarations let tools explain available
choices and reject missing or incompatible arguments before saving a rule.

Values can have different sources. The opening event supplies its actor. A reusable reaction can
have a chosen cat as a setting. A counter records progress, and a query can find a matching visitor.
The internal names for these sources are input, parameter, state, and binding. The authoring
commands must explain the corresponding values where the builder uses them.

## Implementation status

Rules and reaction conditions use the condition tree described here. The condition parser, checker,
evaluator, and printer share that representation. World predicates have named inputs and outputs.

The definition and instance records below describe a proposed way to group reusable reactions and
keep their settings and counters separate. Their validator has tests, but the dispatcher does not
use those records. They are not another implemented kind of world object. Their final storage must
be justified by the reuse examples and can use the existing reaction and object facilities.

Container and item operations now share checked entry points. Actions have named arguments and
explicit results. Hosts can declare typed state and named, cancelable timers. Builders use
`@reaction`, `@state`, `@rule`, and `@facts`. These commands edit the existing signed reaction
values, state properties, and relation objects. They do not create definition or instance objects.

A reaction stores its parameter values and types, state types, selection mode, trace setting, and
ordered effects. Copying it installs a disabled snapshot on another host. Declared state starts from
its initial values on a new host. Existing compatible state on the destination is preserved.

The grouped definition and instance records below remain a proposal, not runtime storage. The
current commands reuse individual reactions and require no corresponding object hierarchy.

Henri's interaction recipe uses those same commands through the shared authoring service. Petting
supplies separate attempt and completion events. Feeding checks custody and access before its
completion event supplies the food. Reactions choose mood responses and change the declared state.
Henri's optional native idle schedules remain separate and disabled by default.

## Definition, instance, and authorization

A **definition** contains reusable behavior. An **instance** supplies one host's parameters and
mutable state. Installation records authority separately from both values.

All records use symbol keys. Lists preserve order. Unknown fields and missing required fields are
errors. Each value expression, condition, and effect can include a `source` record with `start` and
`end` positions. Positions count characters from one, with an exclusive end position.

A definition has these fields:

| Field        | Value                                   | Meaning                                         |
| ------------ | --------------------------------------- | ----------------------------------------------- |
| `name`       | Nonempty string                         | Name shown to the author.                       |
| `parameters` | Map of name to type                     | Values supplied when the behavior is installed. |
| `state`      | Map of name to `{type, initial}` record | Mutable values private to each instance.        |
| `reactions`  | Ordered list of reaction records        | Responses, in their execution order.            |

Names are symbols containing letters, digits, and underscores. A name starts with a letter or
underscore. Symbol names are case insensitive. `Actor` is a readability convention, not a separate
name from `actor`.

The supported types are `object`, `integer`, `float`, `boolean`, `string`, `symbol`, `list`, `map`,
and `any`. Integers and booleans are distinct. A typed argument cannot accept a statically unknown
`any` value. An `object` value is an object reference, including a UUID or anonymous reference.
Operations check validity and access when they use that reference.

An instance has exactly these fields:

| Field        | Value                | Meaning                                                                  |
| ------------ | -------------------- | ------------------------------------------------------------------------ |
| `definition` | Definition record    | The installed definition snapshot.                                       |
| `parameters` | Map of name to value | Exactly the declared parameters, with matching types.                    |
| `state`      | Map of name to value | Exactly the declared state fields, with matching types.                  |
| `revision`   | Positive integer     | Installation revision, changed when the definition or parameters change. |
| `enabled`    | Boolean              | Whether the instance can respond.                                        |

The runtime identifies an instance by its host and local installation name. A host's ordered list of
installation names determines order between instances. Installing a new name appends it. Replacing
an existing name retains its position.

An installation grant binds the host, installation name, definition, parameters, revision, owner,
and relevant authority. Mutable state is not part of the definition signature. Only the declared
state service changes that state. Generic state effects cannot name host properties or capability
storage. A state field named `owner` is local data and cannot change object ownership.

Definition records cannot contain authority claims. Validation accepts an explicit vocabulary, but
that vocabulary cannot authorize an implementation. The installer obtains the permitted vocabulary
from the core's trusted registration service and checks the caller's authority separately.

## Vocabulary and object support

A vocabulary contains `triggers`, `predicates`, and `actions` maps. Each map associates a name with
a description and typed inputs. Predicate records also declare typed outputs.

```moo
[
  'triggers -> ['opened -> [
    'description -> "A successful opening.",
    'inputs -> ['Self -> 'object, 'Actor -> 'object, 'Location -> 'object]]],
  'predicates -> ['nearby -> [
    'description -> "Check an object's current location.",
    'inputs -> ['object -> 'object, 'location -> 'object], 'outputs -> []]],
  'actions -> ['stand -> [
    'description -> "Ask an actor to leave its seat.",
    'inputs -> ['actor -> 'object]]]
]
```

Core registration attaches an implementation and an access contract to each entry. Only authorized
core programmers can register providers or action implementations. A name in an author's record does
not select an arbitrary verb. Object types advertise their supported triggers, policy slots, and
actions through this registered vocabulary. Help uses these same declarations.

Every trigger declares `Self`, the host, as an object input. Completed world operations normally
also supply `Actor` and `Location`. Item and key operations declare `Item` and `Key` where
applicable. Inputs such as `From` and `To` have one documented meaning per operation. The operation
establishes the values. An author cannot obtain execution authority by supplying an `Actor` value.

Attempted, denied, and completed operations use distinct registered triggers. A completed trigger is
emitted once, after its operation succeeds. Narrative events describe an outcome for viewers.
Rendering and delivering a narrative event cannot cause a behavior trigger.

## Value expressions

A value expression identifies where a value comes from:

| `kind`      | Other required field | Meaning                                                     |
| ----------- | -------------------- | ----------------------------------------------------------- |
| `literal`   | `value`              | An exact MOO value. Text does not imply object-name lookup. |
| `input`     | `name`               | A value supplied by this trigger or policy slot.            |
| `parameter` | `name`               | A value supplied when this behavior is installed.           |
| `state`     | `name`               | A declared state value from this instance.                  |
| `binding`   | `name`               | A value selected by an earlier condition.                   |

These namespaces are distinct. `['kind -> 'literal, 'value -> "Actor"]` is text.
`['kind -> 'input, 'name -> 'Actor]` reads the operation's actor.

## Conditions and selectors

The condition tree has five node kinds:

| `kind`      | Other required fields          | Meaning                                                       |
| ----------- | ------------------------------ | ------------------------------------------------------------- |
| `all`       | `children`: ordered conditions | Each child must match. Earlier bindings reach later children. |
| `any`       | `children`: ordered conditions | Return the distinct matches from all children.                |
| `not`       | `child`: condition             | Match only when the child has no match in a complete search.  |
| `compare`   | `op`, `left`, `right`          | Compare two value expressions.                                |
| `predicate` | `name`, `arguments`, `bind`    | Query a registered fact provider.                             |

An empty `all` matches once with the existing bindings. An empty `any` does not match. A comparison
uses `eq`, `ne`, `lt`, `le`, `gt`, or `ge`. Equality preserves value types and string case. Ordered
comparisons require matching numeric types or strings. String order is case sensitive.

Predicate `arguments` map declared input names to value expressions. The `bind` map connects
declared output names to local binding names. An omitted output is not exposed. Reusing a binding
name constrains that output to the existing value. It does not overwrite the binding.

For example, this selector returns occupants of the operation's location:

```moo
['kind -> 'predicate, 'name -> 'occupants,
  'arguments -> ['location -> ['kind -> 'input, 'name -> 'Location]],
  'bind -> ['object -> 'Visitor]]
```

A selector is a predicate with outputs. Its declared inputs identify its scope. The occupants
provider reads one location. A relation provider reads one named relation. Providers enforce read
permissions before supplying facts. Unavailable or inaccessible facts are errors, not evidence of
absence.

Only bindings produced on every `any` branch are available afterward. Those branches must agree on
their types. Negation cannot introduce bindings, including inside an alternative branch. Its
predicate inputs must already be available. Negation never turns an evaluation error or an
incomplete search into a match.

The kernel term functions handle matching, substitution, and positive inference. The MOO condition
service handles composition, comparisons, and negation. Query facts contain no reserved variable
markers. Source names distinguish facts from different relations or providers.

A query returns `['bindings -> {binding_maps}, 'complete -> boolean]`. An empty complete result
means no match. Execution failures raise errors with the failing node's path and source position. An
existence check can succeed after one match. Exhaustive selection and absence checks require a
complete result. Search limits cannot silently truncate those answers.

## Reactions and match selection

A reaction contains `name`, `trigger`, `condition`, `selection`, `effects`, and `enabled`. Its name
is unique within the definition. Its trigger names a vocabulary entry. Its condition is one
condition tree. Its effects are an ordered list.

Selection has `mode`, `limit`, and `order` fields:

| Mode   | Result handling                                                               | Required limit    |
| ------ | ----------------------------------------------------------------------------- | ----------------- |
| `once` | Execute once if the condition matches. Effects cannot read selected bindings. | `1`               |
| `one`  | Execute with exactly one distinct binding map. Several matches are an error.  | `1`               |
| `each` | Execute once per distinct binding map, in declared order.                     | `1` through `256` |

Zero matches execute no effects. `one` must check for a second result rather than request an
unchecked one-result prefix. `each` must detect overflow before it executes any effects.

`order` lists binding names. For `each`, it must include every exposed binding exactly once.
Ordering supports scalar values, with one declared type per binding. Object order uses runtime
identity, never the display string. `once` requires an empty order. Selection happens before the
first effect in that reaction. Each effect still checks current authority and operation state.

The proposed grouped record preserves definition order. Current installed reactions instead run in
property-name order within an event. Effects run in list order. A later reaction observes preceding
state changes. A denial or error stops the current reaction's remaining effects and ends that
dispatch. It cannot be reported as successful execution.

## Query implementation

Conditions and relation queries use the kernel's `term_match` builtin with case-sensitive matching
and duplicate removal. The MOO code supplies facts, checks access, and combines conditions. The
kernel does not inspect objects or call world verbs.

`$rule_engine:register_relation(name, relation, columns, schema)` exposes one relation as a named
query. `columns` lists its field names in tuple order. The schema describes input and output types.
A field belongs to one of those groups. The caller must own the relation or be a wizard. The
registration does not grant readers permission to access private facts.

For example, a membership relation can store `{club, member}` pairs. Its query takes a club and
returns members. The column names are `{'club, 'member}`. Its schema declares an object input named
`club` and an object output named `member`. A rule can use the selected member in later conditions.
The builder workflow will supply these declarations through commands and describe the fields by
name.

`relation:query_result(pattern, bindings, options)` returns `rows` and `complete`. It asks for one
extra distinct answer to detect overflow. `relation:query` requires a complete answer and raises
`E_QUOTA` on overflow. The query uses a bound scalar's index when possible and reads at most 4096
candidate records. A broader query raises an error asking for a narrower input scope. Permission,
invalid-data, and search-budget errors remain errors, including under negation.

Stored values that resemble term-variable markers remain data. Inputs use existing bindings, and
matching interprets variables only in the supplied pattern. A named relation query checks its
candidate tuples against the declared field types before matching.

Relations use exact value equality for tuple identity. Repeated assertion returns the same identity.
Legacy reads already remove duplicates from results. The first mutation collapses duplicate storage
and rebuilds indexes, retaining an original identity for each fact. Owners can call `repair()`
explicitly before that first mutation. Repair does not suspend or commit partial batches.

## Implemented authoring commands

The builder command service is `$builder_features:author(command, text, actor)`. It authenticates
the actor and requires builder rights and ownership. Commands and the `author_reaction` tool call
this same service. The command grammar produces the shared condition tree and effect values. It
never evaluates MOO code.

`@reaction OBJECT NAME after EVENT` creates a disabled reaction. `if`, `then`, `step`, and
`remove-step` edit its condition and ordered effects. `with` supplies a typed setting. `show`
describes all steps and their arguments. `check` validates the definition. `enable`, `disable`, and
`remove` change its installation. `copy to OBJECT` creates an independent disabled snapshot.

`select once` runs once after any match without exposing selected values to steps. `select one`
requires exactly one distinct match. `select each by NAME ...` processes all matches in an explicit
order. Searches must complete within 256 results. Text ordering preserves case; object ordering uses
identity. An incomplete search performs no effects.

`@state` declares numbers, text, flags, object references, or named choices. `@rule` edits an
existing operation policy with the same readable conditions. `@facts` names typed relation columns
and records or removes complete facts. Reaction `record` and `forget` steps use those schemas and
check write authority separately from query authority. Repeated insertion remains idempotent.

Help for events, steps, actions, and facts comes from the definitions used for validation. `preview`
uses canonical read-only world and relation queries. It rejects extension predicates and does not
invoke actions, message recipients, state transitions, or timers. It shows resolved action arguments
and simulates direct state changes. It does not expand reactions caused by those changes or promise
that future action permission checks will succeed.

`trace on` records the event inputs, condition result, selected values, attempted steps, and a
failure. Only the owner or a wizard can read the report. A report from an earlier installation is
marked as such. Reports and match sets are bounded. Tracing is optional.

## Implemented operations and timers

Container commands and actions share one operation for opening, closing, locking, unlocking, taking,
and putting. Taking an item directly from an open container uses that same operation. Both paths
check the container policy, the item policy, actor authority, reach, and custody. They recheck state
and policies after callbacks. A successful take emits one `on_get` event for the item and one
`on_take` event for its container. Denied operations emit neither completed event.

`view_rule` controls viewing contents separately from `take_rule`. A take rule can refer to the
selected item without breaking a look or a contents search. Passage conditions use `From` and `To`
for their endpoints. A condition without `Key` runs once without an inventory search. A condition
that uses `Key` tests carried items.

`$reaction:action_schemas()` supplies descriptions and named argument types. The supported actions
include container operations, getting and dropping items, sitting and standing, and wearing and
removing clothing. `perform_action(name, arguments, context)` returns a `status` of `applied`,
`denied`, or `error`. Denials include a reason. Errors retain the exception details. A reaction
stops its remaining effects on denial or error.

Wizards can register an action service with the same description and argument declarations. Its MOO
method receives `(arguments, context)` and returns the same result format. The service must enforce
its caller's authority. The declarations support help and validation; they do not grant permissions.
A preview can describe that call without executing the service.

An action effect stores named value expressions in its `arguments` field. For example, an unlock
action needs `target`, `actor`, and `key`. Each value can come from a literal, the event, declared
state, or a selected binding. `input.Actor` remains the original participant when a query also
produces a binding named `Actor`.

`declare_state(host, name, type, initial, choices)` declares an object's counter or state field.
`choices` can restrict a state to values such as `awake` and `sleepy`. `state_values` reads the
current values, and `state_types` supplies their types to `mk` when a condition or action refers to
state. `change` checks declared values before mutation and dispatches thresholds from the actual old
and new values. Core operations can also emit transitions for built-in state such as a container's
`open` field.

The schedule effect is `{'schedule, name, seconds, effect}`. Its name belongs to the host, so one
reaction can cancel a timer started by another. Scheduling that name again replaces the pending
task. `{'cancel, name}` cancels it. `timers(host)` shows names and due times to the owner.

Replacing or disabling an installation cancels its timers immediately. Re-enabling does not revive
canceled work. Before execution, a timer rechecks the host's owner, wizard status, location,
installation, and condition. Changed locations or invalid installations skip the pending effect.
Declared state and selected bindings are read again. The event's inputs remain the captured values.

An object can have 16 pending timers. Each delay is at most one day. A reaction can have 64 effects,
and nested effects have a limit of 16 levels. Explicit reaction chains stop after four dispatch
steps, before the VM's recursion limit, and report the chain. The dispatcher also limits the host to
64 installed reactions. These limits bound an invocation, not independent future world events.

Forking a timer commits prior effects. An error after that point cannot undo those effects. Within
one transaction, catching an exception does not undo earlier mutations. An operation that completed
before a later reaction failed must not be described as rolled back.

## Effects, state, and timers

| `kind`     | Other required fields        | Operation                                               |
| ---------- | ---------------------------- | ------------------------------------------------------- |
| `action`   | `name`, `arguments`          | Invoke a registered action with typed, named arguments. |
| `state`    | `name`, `op`, `value`        | Set, increment, or decrement a declared state field.    |
| `schedule` | `name`, `seconds`, `effects` | Replace a named timer with a delayed effect sequence.   |
| `cancel`   | `name`                       | Cancel this instance's named timer, if it exists.       |

Actions declare all required arguments. An action that controls another actor requires the caller to
control that actor or hold an explicit delegation. A rule match is not a write grant. Actions return
`['status -> 'applied, 'reason -> ""]` or a `denied` result with a reason. Missing actions, invalid
arguments, and implementation failures are errors.

State writes preserve the declared type. Arithmetic requires an integer or float state field and a
value of that same type. A state transition records its actual field, old value, and new value. An
unchanged value emits no transition. The `state_changed` trigger supplies `Self`, `Field`, `Old`,
and `New`. Field-specific validation uses the declared state type for `Old` and `New`.

Timer seconds are nonnegative integers. A timer belongs to a host, installation name, revision, and
timer name. Scheduling the same name replaces that instance's previous timer. Captured inputs and
selected bindings are immutable. State references read current state when the timer executes.

Disabling, replacing, removing, or transferring the behavior cancels its pending timers. Timer
execution rechecks the installation grant and revision. Actions then check current object validity,
reachability, and permissions. Message destinations are explicit arguments. A captured location
remains the original location unless the definition explicitly selects a current location.

The runtime uses the existing persistent scheduler. Each dispatch permits at most 32 nested trigger
deliveries, 256 selected matches per reaction, and 32 new timers. Exceeding a limit raises a
behavior error with the dispatch chain. Definitions contain at most 64 reactions, 1024 executable
nodes, and 16 levels of condition or timer nesting.

Effects are sequential, not an all-or-nothing transaction abstraction. mooR suspension and task
forks can commit earlier changes. A later denial or error stops further work but does not promise to
undo committed effects. Diagnostics identify the completed effects and the failing effect.

## Reuse and inheritance

Installing a definition creates local parameter and state values. Reusing that definition on a
second host creates another instance. Editing a template does not silently replace installed
snapshots or renew their authority.

Prototype inheritance can supply vocabulary and reusable templates. Executable instances require
explicit installation on each host. A child does not inherit another host's installation grant or
mutable state. An update replaces an instance with a new revision and requires authorization. The
update must supply values for its declared state, making preservation or reset explicit.

## Cupboard example in the proposed record format

The behavior counts openings and asks a nearby cat to stand. The operation supplies the opener and
location. Installation supplies the cat. The following example assumes the `opened`, `nearby`,
`stand`, and `announce` vocabulary entries from the executable contract fixture.

```moo
['name -> "Cupboard attracts a cat",
  'parameters -> ['cat -> 'object],
  'state -> ['openings -> ['type -> 'integer, 'initial -> 0]],
  'reactions -> {[
    'name -> 'greet, 'trigger -> 'opened, 'enabled -> true,
    'condition -> ['kind -> 'predicate, 'name -> 'nearby,
      'arguments -> [
        'object -> ['kind -> 'parameter, 'name -> 'cat],
        'location -> ['kind -> 'input, 'name -> 'Location]],
      'bind -> []],
    'selection -> ['mode -> 'once, 'limit -> 1, 'order -> {}],
    'effects -> {
      ['kind -> 'state, 'name -> 'openings, 'op -> 'increment,
        'value -> ['kind -> 'literal, 'value -> 1]],
      ['kind -> 'action, 'name -> 'stand,
        'arguments -> ['actor -> ['kind -> 'parameter, 'name -> 'cat]]],
      ['kind -> 'action, 'name -> 'announce,
        'arguments -> [
          'location -> ['kind -> 'input, 'name -> 'Location],
          'text -> ['kind -> 'literal, 'value -> "The cat looks interested."]]]}
  ]}]
```

For an installed instance, `parameters` contains the cat reference and `state` initially contains
`['openings -> 0]`. The installation must authorize control of that cat. It cannot obtain that
authority from the `cat` parameter alone.

On a successful opening, the operation supplies `Self`, `Actor`, and `Location`. If the cat is
elsewhere, the condition does not match and no effect runs. If the cat is nearby, the counter
increases, the stand action runs, and the message goes to the supplied location. If standing is
denied, the message does not run. The counter can already have changed.

The fixture in `tests/baseline/fixtures/behavior_scenarios.moo` validates this exact shape. It also
rejects unknown parameters, undeclared state, missing action arguments, unsafe negation, ambiguous
binding selection, and invalid timers. These checks establish the model contract. Subsequent
operation tests must prove the described execution behavior.

## Schema API

`$behavior_schema:validate(definition, vocabulary)` returns a valid definition unchanged. It raises
`E_INVARG` for invalid fields, names, types, or binding use. Structural limits raise `E_QUOTA`.

`$behavior_schema:instance(instance, vocabulary)` additionally validates actual parameters, state,
revision, and enabled status. `$behavior_schema:bindings(values, types)` checks complete runtime
input maps. These methods do not grant access or execute world code.

The condition service can use `condition(tree, scope, predicates)` and `expression(value, scope)`
for static checks. A scope has `input`, `parameter`, `state`, and `binding` type maps. Condition
validation returns definite binding types and the counted nodes. Errors contain the record path.
