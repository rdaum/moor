# Give your objects rules and reactions

You can make a box greet visitors, remember who opened it, or offer a timed puzzle. You do this with
commands inside the world. You do not need to write MOO code or have programmer privileges.

This guide starts with a box that announces a message. Later examples add a counter, club
membership, and a timer. Enter each command on its own line in your usual command input.

## Before you start

Your account needs builder privileges. A world administrator can grant them. The objects in these
examples belong to you. You can inspect and edit them with the same commands in Meadow or a terminal
client.

An **object** is a thing in the world: a room, a person, a box, or a chair. A **prototype** is an
existing kind of object from which you can create another object. For example, `$container` is
Cowbell's supplied kind of box. The dollar sign identifies a named object supplied by the world.

Commands that start with `@` are building tools. Ordinary commands such as `open cedarbox` let you
try what you built. Object names must identify one object you can find. Use double quotes around
names with spaces, such as `"cedar box"`.

## Three parts of an interaction

A **rule** controls whether an operation is allowed. A rule on a box can allow only club members to
open it. Refusing an operation leaves it undone.

A **reaction** describes a response to something that happened. Opening a box can cause a message,
increase a counter, or start a timer. A reaction has an event, an optional condition, and numbered
steps:

| Part      | Example                    | Meaning                                                |
| --------- | -------------------------- | ------------------------------------------------------ |
| Event     | `after open`               | Respond after someone opens this box.                  |
| Condition | `if visits < 2`            | Respond only while the visit counter is less than two. |
| Step      | `then announce "Welcome!"` | Show this message in the room.                         |

A **relation** stores facts, like a membership list. Rules and reactions can consult that list. You
do not need a relation for the first examples.

## Make a box announce a message

Create a box:

```text
@create $container named cedarbox
```

Create a reaction called `welcome`, then give it a step:

```text
@reaction cedarbox welcome after open
@reaction cedarbox welcome then announce "A cedar scent escapes."
```

Here, `cedarbox` is the object and `welcome` is your name for this reaction. `after open` chooses
the event. `then` adds a step. The quoted text is the message that people will see.

New reactions start disabled. You can finish and inspect them before they affect anyone.

```text
@reaction cedarbox welcome show
@reaction cedarbox welcome check
@reaction cedarbox welcome preview actor=me
```

`show` displays the event, condition, numbered steps, and current state. `check` looks for invalid
parts. `preview` shows what the steps would do with the current facts. It makes no changes and sends
no messages to other people.

Enable the reaction, then try the box:

```text
@reaction cedarbox welcome enable
close cedarbox
open cedarbox
```

Containers start open, so the first `close` prepares this example. Opening the box now produces the
cedar message. Asking to open an already open box does not produce another opening event.

### Change or stop the response

Replace the first step:

```text
@reaction cedarbox welcome step 1 announce "The box smells of fresh cedar."
```

`step 1` replaces an existing step. `then` adds another step at the end. Use `remove-step 1` to
remove the first step.

Stop the reaction:

```text
@reaction cedarbox welcome disable
```

`enable` starts it again. `remove` deletes the reaction. These changes leave the box itself in
place. An edit to an enabled reaction takes effect immediately. For several edits that must take
effect together, disable the reaction first.

## Name the participants

Conditions and steps can use names supplied by the event:

| Name    | Meaning                                                          |
| ------- | ---------------------------------------------------------------- |
| `this`  | The object that has the reaction or rule.                        |
| `actor` | The person or character who performs the operation.              |
| `place` | The room where the event happened.                               |
| `item`  | The item involved in taking, putting, or another item operation. |
| `key`   | The key involved in locking or unlocking.                        |
| `food`  | The food involved in Henri's feeding event.                      |
| `me`    | You, saved as a particular person when you enter the command.    |
| `here`  | Your current room, saved when you enter the command.             |

`actor` can be a different person on every opening. `me` keeps referring to you after someone else
opens the box. Each event supplies only the names relevant to that event.

List the events and their available names:

```text
@reaction events
```

Most events report a completed operation. Henri's `pet_attempt` also reports a nearby attempt he
refuses. The starting cupboard and couch send their own notifications to Henri.

## Remember a number or a choice

**State** is information an object remembers and can change. A box can remember a visit count. A
puzzle can remember whether it is `idle`, `armed`, or `solved`.

Declare a counter, then change the welcome reaction to count its first two openings:

```text
@state cedarbox visits number 0
@reaction cedarbox welcome if visits < 2
@reaction cedarbox welcome then add visits 1
@reaction cedarbox welcome enable
```

`number 0` creates a whole-number counter starting at zero. The condition is checked before the
steps run. The message is step 1 and the increment is step 2. After two openings, this reaction
stops responding until the counter changes.

Read the current state:

```text
@state cedarbox
```

Reset the counter:

```text
@state cedarbox visits set 0
```

Other state types are `decimal`, `text`, `flag`, `object`, and `choice`. A flag is `yes` or `no`. A
choice accepts only the words you list, starting with the first word. For example:

```text
@state cedarbox mood choice quiet curious sleepy
```

You can also start a reaction when state crosses a threshold. `when visits >= 4` means the counter
changed from less than four to four or more. It does not keep firing while the counter stays above
four.

### Combine conditions

Use `AND` when both conditions must hold. Use `OR` when either condition is enough. `NOT` reverses a
condition. Parentheses let you group conditions.

```text
@reaction cedarbox welcome if actor is me AND visits < 2
@reaction conditions
```

`if` replaces the previous condition. `if always` removes that restriction. Conditions such as
`this is open`, `actor is in place`, and `actor is owned by me` describe current world facts.

## Copy a reaction

Create another box, then copy the welcome reaction:

```text
@create $container named sparebox
@reaction cedarbox welcome copy to sparebox
@reaction sparebox welcome show
@reaction sparebox welcome enable
```

The copy starts disabled. On a new object, its declared state starts at the initial values.
Compatible state already on the destination keeps its current values. Later edits to one reaction do
not change the other.

Each copy's `this` means its own box. An explicit object name or `me` keeps referring to the same
object or person. Review those choices before you enable a copy.

A **setting** names a choice that belongs to a reaction. For example, you can choose a particular
cat as `friend`, then use `friend is in place` as a condition. You can change the choice without
rewriting that condition. Use `with friend object Henri` to name the cat. `show` lists the settings.

## Allow club members to open a box

This example keeps a membership list. Each entry names a person and a group. A fact is one entry in
that list, such as “you belong to readers.”

Create the box and a relation to hold the list:

```text
@create $container named clubbox
@create $relation named members
@facts members define club_members person:object group:text
```

`members` is the object that stores the list. `club_members` is the fact name used in conditions.
`person:object` means each entry names a person in the world. `group:text` means it also contains a
group name. Fact names are shared across the world. Choose a different name if someone already uses
`club_members`.

Add yourself to the readers group, then set the opening rule:

```text
@facts members add person=me group=readers
@rule clubbox open fact club_members person=actor group=readers
close clubbox
open clubbox
```

The rule looks for an entry whose person is the opener and whose group is `readers`. The word `fact`
means “look for this stored fact.” Repeating the same `add` command keeps one entry.

Remove your membership, then try again:

```text
close clubbox
@facts members remove person=me group=readers
open clubbox
```

The box refuses to open. You can read the rule and the list with these commands:

```text
@rule clubbox open show
@facts members
```

Rules apply to particular operations. An `open` rule controls opening. To restrict taking items
while the box is already open, give `take` its own rule. `view` and `put` also have separate rules.

### Remember who opened it

A second relation can remember which people opened which boxes:

```text
@create $relation named visits
@facts visits define club_visits person:object box:object
@reaction clubbox log after open
@reaction clubbox log then record club_visits person=actor box=this
@reaction clubbox log enable
```

Restore your membership, then open the box:

```text
@facts members add person=me group=readers
open clubbox
@facts visits
```

The new fact names you and `clubbox`. Repeated visits by the same person keep one fact. This list
answers “who visited which box?” A counter is a better choice for “how many visits?”

## Build a timed puzzle

This box gives players thirty seconds to close it after opening it. Closing it in time solves the
puzzle. Waiting too long returns its state to `idle`.

Create the box and its three possible states:

```text
@create $container named puzzlebox
@state puzzlebox mode choice idle armed solved
```

Add a reaction that arms the puzzle and starts a timer:

```text
@reaction puzzlebox arm after open
@reaction puzzlebox arm then set mode armed
@reaction puzzlebox arm then schedule reset 30 set mode idle
@reaction puzzlebox arm then announce "Close the box before its light goes out."
@reaction puzzlebox arm trace on
@reaction puzzlebox arm enable
```

`reset` is your name for the timer. `30` is its delay in seconds. `set mode idle` is the step it
will perform. Starting another timer named `reset` on this box replaces the pending one.

Add a reaction that solves the armed puzzle:

```text
@reaction puzzlebox solve after close
@reaction puzzlebox solve if mode is armed
@reaction puzzlebox solve then cancel reset
@reaction puzzlebox solve then set mode solved
@reaction puzzlebox solve then announce "The box chimes. You solved it."
@reaction puzzlebox solve enable
```

Try it:

```text
close puzzlebox
open puzzlebox
close puzzlebox
@state puzzlebox
@reaction puzzlebox arm show
```

The state is `solved`, and no timer remains. Open it again and wait more than thirty seconds.
`@state puzzlebox` then shows `idle`. Closing it after that does not solve the puzzle. Opening it
starts a new attempt, including after a successful attempt.

Timers recheck the originating reaction's condition before they act. A condition such as
`if mode is idle` becomes false after the arming step. The timer then skips the reset. The arming
reaction therefore has no condition. Moving the object also prevents its pending timer from acting.
Disabling, editing, or removing a reaction cancels its timers.

## Understand a failed response

List an object's reactions with `@reaction cedarbox`. Use `show` on a particular reaction to read
its condition, steps, state, and pending timers.

To record the next attempt, turn on its **trace**:

```text
@reaction cedarbox welcome trace on
close cedarbox
open cedarbox
@reaction cedarbox welcome trace
```

The trace is a report of the latest run. It shows the event's participants, whether the condition
matched, and which steps completed. It also reports a timer that finished, failed, or skipped its
work. It does not keep a full history.

| What you see                 | What to do                                                                               |
| ---------------------------- | ---------------------------------------------------------------------------------------- |
| The reaction is disabled.    | Use `enable` after you finish editing.                                                   |
| The condition did not match. | Read the current state and facts. Use `preview actor=me` to try the condition.           |
| An action was denied.        | Read the failed step. Check the target's rule, the acting character, and their location. |
| An unknown state name.       | Use `@state OBJECT` to read the names. Declare a new value before using it.              |
| You do not own the object.   | Use an object you own, or ask its owner to make the change.                              |
| A timer skipped its work.    | Read the trace for a changed condition, selected object, or location.                    |

Steps run in numbered order. Reactions for the same event run in name order. A failed step stops
later steps. Earlier completed work can remain, so a trace matters even when the final step fails.

A preview does not perform actions or promise that they will be permitted. It does not run other
reactions that a state change would start. It cannot evaluate conditions supplied by custom MOO
code. Trying the actual interaction and reading its trace is the final check.

Reactions run with their object's owner's permissions. Naming another player as `actor` does not
give your reaction control over that player. Normal rules, location checks, and ownership checks
still apply to actions.

## Henri: a supplied example

Henri starts grouchy. He refuses the first three pet attempts and hisses once for each. The fourth
attempt makes him sleepy, resets his counter, and permits the pet. Feeding him accepted kibble also
makes him sleepy and resets the counter.

His reactions separate an attempted pet from a successful pet. The attempt counter can therefore
progress even while he refuses petting. Each mood response lives in one reaction.

Henri's owner or a world administrator can inspect his responses with `@reaction Henri` and his
state with `@state Henri`. For example, `@reaction Henri pet_sleepy show` shows the sleepy response.
The same `step`, `if`, `disable`, and `trace` commands used for your boxes apply to these reactions.

Henri also has optional background grooming and idle actions. Those use separate code and stay
disabled by default. They are not part of these interaction examples.

## Find more commands

`help @reaction` introduces the command. These lists describe the available parts:

```text
@reaction help
@reaction events
@reaction conditions
@reaction steps
@reaction actions
@reaction facts
```

`actions` lists operations that a step can request, such as opening a container or seating an owned
character. `facts` lists the available questions and the information each question needs. The lists
come from the same definitions the commands use to check your work.
