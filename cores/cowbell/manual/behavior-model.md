# Behavior model

A behavior describes how an object responds to world operations. Its conditions read facts. Its
effects request actions or change declared local state. A builder combines these parts without
writing MOO verbs.

This chapter defines the data contract for Cowbell's behavior implementation. It is intended for
core programmers and authors of tools. The builder manual will introduce the corresponding text
commands through examples.

## Implementation status

`$behavior_schema` validates the definitions and instances described here. Validation is pure: it
does not run predicates, resolve object names, install authorization, or execute effects.

The existing rule parser and reaction dispatcher still use their previous representations. They will
move to this contract in the condition, query, operation, and authoring stages. Runtime rules in
this chapter specify that target implementation. They are not claims about the old dispatcher.

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

Reactions run in definition order. Effects run in list order. A later reaction observes preceding
state changes. A denial or error stops the current reaction's remaining effects and ends that
dispatch. It cannot be reported as successful execution.

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

## Worked cupboard definition

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
