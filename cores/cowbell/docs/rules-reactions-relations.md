# Rules, reactions, and relations: a builder's introduction

Cowbell is a shared world where people meet, explore rooms, and create things together. A room, a
person, a cupboard, and a cat are all **objects** in that world. Objects have names, descriptions,
owners, and things they can do.

As a builder, you can decide how your objects respond to people. You can let only club members open
a cupboard, or make a timed box puzzle. A cat can react differently as its mood changes. You can
make these interactions with in-world commands, without writing code or having programmer
privileges.

This introduction explains the parts through five tutorials:

1. [Make a cupboard greet people.](#tutorial-1-make-a-cupboard-greet-people)
2. [Keep a membership list and control access.](#tutorial-2-keep-a-membership-list-and-control-access)
3. [Count greetings and investigate a skipped response.](#tutorial-3-count-greetings-and-investigate-a-skipped-response)
4. [Build a timed puzzle.](#tutorial-4-build-a-timed-puzzle)
5. [Copy a response to another object.](#tutorial-5-copy-a-response-to-another-object)

## Before you start

Your account needs builder privileges. Your world administrator can grant them. Enter each command
on its own line in your usual command input, in Meadow or a terminal client.

Commands that start with `@` are building tools. Commands such as `open clubbox` let you try what
you build. Text in double quotes is a message or a name with spaces.

The first three tutorials build on the same cupboard. Follow them in order. The puzzle is a separate
example. The final tutorial copies the cupboard's response.

Use object names that are not already in use nearby. If you change an example's name, use your
replacement throughout its commands.

## Start with something you want to happen

Imagine a cupboard in a reading club. You want three things:

- Only club members can open it.
- Opening it produces a welcome message and increases an opening count.
- A list remembers which people belong to the club.

Each requirement has a place:

| Part           | What it does                                                | In the club                                |
| -------------- | ----------------------------------------------------------- | ------------------------------------------ |
| A **rule**     | Sets a condition for allowing an action.                    | The opener must be a club member.          |
| A **reaction** | Specifies a response to something that happens.             | Welcome the opener and increase the count. |
| A **relation** | Stores a set of facts that rules and reactions can consult. | Keep the membership list.                  |

You choose these parts and their contents. Cowbell supplies the ordinary actions, such as opening a
container, and carries out your instructions. You can change the membership list or welcome message
while the world is running.

## Rules: decide what is allowed

A rule answers a question before an action proceeds. For the cupboard, the question is “does this
person belong to the reading club?” A rule on a different cupboard can ask whether the person has
the right key.

A **condition** is a statement the system can check. Examples include “the person is a member,”
“this is the brass key,” and “the cat is in this room.” You can combine conditions. “Member AND
carries the key” requires both. “Member OR carries the key” accepts either.

Rules apply to particular actions. An opening rule controls opening. Taking something from an
already open cupboard is a separate action, with its own rule. Looking inside and putting items
inside also have separate rules. You choose which activities need restrictions.

An access rule works alongside the normal requirements of the action. A person still needs to be
within reach of the cupboard. A locked cupboard still needs unlocking.

## Reactions: describe the response

A reaction says **when to respond**, **whether a condition must hold**, and **what steps to take**.
For the cupboard, that can read:

> After someone opens this cupboard, announce “Welcome to the reading club,” then increase its
> opening count by one.

The opening is an **event**: something that happened in the world. The message and the counter
change are **steps**. Steps can also remember a fact, start a timer, or request an action such as
seating a character you own.

A reaction can have an additional condition. For example, a cupboard can announce its message only
while a nearby cat is present. If the cat is absent, the cupboard still opens. That reaction skips
its steps. The opening rule and the reaction's condition serve different purposes.

Most events report a successful action. An opening reaction runs after the cupboard opens, so it
cannot prevent that opening. Some objects offer more specific events. Henri, the supplied cat, has
separate events for a pet attempt and a successful pet. He can react to an attempt he refuses.

Reactions respond to the events their objects support. A condition such as “the cat is nearby” is
checked when the selected event happens. It does not continuously watch for the cat's arrival.

### The event tells you who and where

The same cupboard can respond to different visitors without naming each person in advance. Its
opening event identifies the opener and the room. In commands, these participants have names:

| Name    | Meaning in the opening example       |
| ------- | ------------------------------------ |
| `this`  | The cupboard with the reaction.      |
| `actor` | The person who opened it.            |
| `place` | The room where the opening happened. |

Other events supply other information. Taking an item supplies that item. Unlocking supplies the
key. The available names depend on the event.

The command word `me` names you when you write the instruction. It keeps referring to you later. By
contrast, `actor` refers to whoever performs each action. This distinction lets you choose between
“welcome the current visitor” and “allow only me.”

### Choose what to do with several matches

A condition can also find objects, such as characters in the room. You choose how the reaction uses
those matches:

- Respond once if anyone matches.
- Use one matching character, and report a problem if several match.
- Repeat the steps for each matching character in a chosen order.

No matches means the reaction skips its steps. This choice matters for counts and repeated actions.
Increasing a counter once for the room differs from increasing it once for each visitor.

### Tutorial 1: Make a cupboard greet people

Start with a cupboard that says something whenever someone opens it. Create the cupboard:

```text
@create $container named clubbox
```

`$container` is Cowbell's supplied kind of container. The dollar sign identifies a named object
supplied by the world. This command creates your own container called `clubbox`.

Create a reaction called `welcome`. Give it one step:

```text
@reaction clubbox welcome after open
@reaction clubbox welcome then announce "Welcome to the reading club."
```

Read the first command as “on clubbox, make a response called welcome, after an opening.” The second
adds a step: announce the quoted message to people in the room. `welcome` is your chosen name for
this response. One object can have several named reactions.

The new reaction starts disabled. Inspect it before you enable it:

```text
@reaction clubbox welcome show
@reaction clubbox welcome check
@reaction clubbox welcome preview actor=me
```

`show` lists your instructions. `check` reports `Valid: welcome` if the parts are valid. `preview`
shows the planned steps without performing them. Here, `actor=me` asks for a preview with you as the
opener. It does not actually open the cupboard or announce anything to the room.

Enable the reaction. Close and open the cupboard:

```text
@reaction clubbox welcome enable
close clubbox
open clubbox
```

New containers start open, so the first `close` prepares the test. The opening now produces “Welcome
to the reading club.” You also see the ordinary opening message. Opening an already open cupboard
produces no new opening event.

#### Change the message

Replace step 1, then try another opening:

```text
@reaction clubbox welcome step 1 announce "The club cupboard is ready."
close clubbox
open clubbox
```

You now see the replacement message. `step 1` replaces the first step. `then` adds a step at the
end. An edit to an enabled reaction takes effect immediately.

#### Stop and restart the response

Disable the reaction, then open the cupboard again:

```text
@reaction clubbox welcome disable
close clubbox
open clubbox
```

The cupboard opens without the greeting. Disabling the reaction leaves the cupboard and its ordinary
actions available.

Enable it again for the next tutorial:

```text
@reaction clubbox welcome enable
```

## Relations: remember facts about people and things

A relation stores facts as rows in a table. Each column has a name. A membership relation can look
like this:

| Person | Group     |
| ------ | --------- |
| Ada    | Readers   |
| Ben    | Readers   |
| Ada    | Gardeners |

Each row is one fact. The first row says that Ada belongs to Readers. Your cupboard's opening rule
looks for a row containing the opener and Readers. Several objects can consult the same membership
list, so a membership change can affect their next access checks.

The commands call these columns **fields**. You choose the fields the relation needs. A list of
visited places can name a person and a room. A borrowing list can name a person and an item. The
membership example needs a person and a group.

A relation keeps each fact once. Adding “Ada belongs to Readers” again leaves one such entry.
Removing that fact removes Ada's Readers membership. Her Gardeners membership remains.

This also means that a list of people who visited a room does not count repeat visits. It answers
“who visited?” A count answers “how many times?”

Some facts already come from the world. Cowbell knows where a cat is and whether a cupboard is open.
Your conditions can ask about those facts directly. You create a relation for additional information
you want the world to remember, such as club membership.

Changing a relation updates the stored facts. It does not automatically run every reaction that
mentions them. The next relevant action or event checks the updated facts.

### Tutorial 2: Keep a membership list and control access

Keep using `clubbox` from tutorial 1. This time, only members of the readers group can open it.

Create a relation to hold the membership list. Define the information each entry contains:

```text
@create $relation named members
@facts members define person:object group:text
```

`members` is your relation object. `person:object` means each entry refers to a person in the world.
`group:text` means it also contains a group name written as text. These field names belong to this
list. Another builder can make a separate list with the same fields. The lists share no facts.

Add yourself to the readers group. Set the cupboard's opening rule:

```text
@facts members add person=me group=readers
@rule clubbox open fact in members person=actor group=readers
close clubbox
open clubbox
```

The words `fact in members` tell the rule which list to consult. It looks for a membership entry
with the current opener as `person` and `readers` as `group`. You added that entry for yourself, so
the cupboard opens and greets you.

#### Try it without membership

Close the cupboard. Remove your membership, then try to open it:

```text
close clubbox
@facts members remove person=me group=readers
open clubbox
```

The cupboard refuses to open. There is no greeting, because no opening happened. You still own the
cupboard. Ownership does not bypass the rule you gave it.

Inspect the rule and membership list:

```text
@rule clubbox open show
@facts members
```

The rule still requires membership. The list has no facts. Restore your entry and try again:

```text
@facts members add person=me group=readers
open clubbox
```

The cupboard opens and greets you again. You changed its access by editing a fact, without changing
its rule or reaction.

This rule controls opening only. A separate `take` rule controls taking items from an open cupboard.

#### Remember who opened the cupboard

Add a visitor list. Give the reaction a setting named `visitors` that selects this list:

```text
@create $relation named visits
@facts visits define person:object box:object
@reaction clubbox remember after open
@reaction clubbox remember with visitors object visits
@reaction clubbox remember then record in visitors person=actor box=this
@reaction clubbox remember enable
close clubbox
open clubbox
@facts visits
```

The list now contains one fact that names you and `clubbox`. The `record` step supplies the person
from `actor` and the cupboard from `this`. The existing `welcome` reaction still announces its
message. Both reactions respond to the same opening.

The setting belongs to this reaction. To use a different list with the same fields, change
`with visitors object visits` to name that list. The steps need no changes. Copying a reaction keeps
its chosen list until you change the setting. This lets you deliberately share records or keep them
separate. Copying does not make a new relation.

Try a second visit:

```text
close clubbox
open clubbox
@facts visits
```

The list still has one fact. It remembers that you visited this cupboard, without a duplicate for
each opening. The next tutorial adds a number for something that needs counting.

## State: let an object remember its progress

**State** is information an object remembers about itself. The cupboard's opening count is state. So
is a cat's mood or a puzzle's progress.

You give each state value a name and a starting value. A count can start at zero. A puzzle can have
a choice of three words: `idle`, `armed`, and `solved`. Restricting it to those choices helps catch
accidental spelling mistakes.

Reactions can read and change state. An opening reaction can increase the count. A feeding reaction
can change a cat's mood to sleepy. Several reactions on the same object can use the same state
value.

The cupboard's count belongs to the cupboard. Ada and Ben both increase that same count when they
open it. To remember which person did something, a relation can connect the person to the cupboard.
The choice depends on what you want to remember.

A state change can also start a reaction. For example, a counter reaching four can make a cat
sleepy. That response happens when the count crosses the chosen point. It does not keep repeating
while the count stays at four.

### Tutorial 3: Count greetings and investigate a skipped response

Use the same cupboard, with your membership restored. Make it greet people on its next two openings,
then stop greeting. The cupboard will still open after that.

Give it a counter called `greetings`. Add a counting step and a condition to `welcome`:

```text
@state clubbox greetings number 0
@reaction clubbox welcome then add greetings 1
@reaction clubbox welcome if greetings < 2
@reaction clubbox welcome trace on
```

`number 0` declares a whole-number counter that starts at zero. `add greetings 1` increases it by
one. The condition `greetings < 2` means “the greetings counter is less than two.” With `trace on`,
Cowbell keeps a report of the reaction's most recent attempt. Each attempt replaces the previous
report.

The condition applies to the whole reaction. If it fails, neither the greeting nor the increase
happens. The counter therefore counts greetings, not every opening.

Close and open the cupboard three times:

```text
close clubbox
open clubbox
close clubbox
open clubbox
close clubbox
open clubbox
```

The first two openings produce the greeting. The third produces only the ordinary opening message.
The membership rule still allows the opening. The separate `remember` reaction still records the
visitor's fact.

Inspect the counter and the most recent attempt:

```text
@state clubbox
@reaction clubbox welcome trace
@reaction clubbox welcome preview actor=me
```

The counter reads `greetings: 2`. The trace includes `Condition: did not match`. The preview also
reports that no steps will run with the current state. This is the intended limit, rather than a
failed opening or a disabled reaction.

#### Make it greet again

Reset the counter and try another opening:

```text
@state clubbox greetings set 0
close clubbox
open clubbox
@state clubbox
```

The greeting returns, and the counter reads `greetings: 1`.

For the remaining tutorials, remove the two-greeting limit:

```text
@reaction clubbox welcome if always
```

`if` replaces the previous condition. With `always`, the reaction greets and counts every successful
opening. The cupboard's membership rule still applies.

## Follow one opening from start to finish

Suppose Ada approaches the club cupboard:

1. Ada tries to open it.
2. The opening rule looks for Ada's Readers membership in the relation.
3. If the entry is missing, the cupboard refuses to open.
4. If the rule and the normal opening checks pass, the cupboard opens.
5. The opening event starts its welcome reaction.
6. The reaction announces the message, then increases the cupboard's count.

The membership list supplies a fact, the rule checks access, and the reaction changes the world
after the opening. A second reaction can record Ada's visit in another relation. That new fact can
then be available to other rules and reactions.

## Timers: let a response happen later

A **timer** waits for a chosen number of seconds, then attempts a step. Timers let you make
interactions that depend on time as well as actions.

For example, a puzzle box can give someone thirty seconds to close it after opening it:

| What happens                         | What the puzzle does                                               |
| ------------------------------------ | ------------------------------------------------------------------ |
| Someone opens the box.               | Set its state to `armed` and start a reset timer.                  |
| Someone closes it while it is armed. | Cancel the timer, set its state to `solved`, and announce success. |
| The timer expires first.             | Set its state to `idle`. Closing it now does not solve the puzzle. |

You name a timer so another reaction can cancel it. Starting a timer with the same name on the same
object replaces the pending one.

A timer checks the originating reaction's condition again before it acts. Moving the object prevents
the pending timer from acting. Editing, disabling, or removing the reaction cancels its timers. The
following puzzle uses two reactions to start and cancel the same timer.

### Tutorial 4: Build a timed puzzle

This example uses a new box. You can follow it independently of the cupboard tutorials.

Create a box and give it three possible states:

```text
@create $container named puzzlebox
@state puzzlebox mode choice idle armed solved
```

The state is called `mode`. `choice` restricts it to the words you list. The first word, `idle`, is
its starting value.

Create an opening reaction with three steps:

```text
@reaction puzzlebox arm after open
@reaction puzzlebox arm then set mode armed
@reaction puzzlebox arm then schedule reset 30 set mode idle
@reaction puzzlebox arm then announce "Close the box within thirty seconds."
@reaction puzzlebox arm trace on
@reaction puzzlebox arm enable
```

Step 1 arms the puzzle. Step 2 starts a timer called `reset`. After thirty seconds, that timer sets
`mode` back to `idle`. Step 3 tells people what to do. The timer runs later. It does not delay the
message.

Add a second reaction for closing the box while it is armed:

```text
@reaction puzzlebox solve after close
@reaction puzzlebox solve if mode is armed
@reaction puzzlebox solve then cancel reset
@reaction puzzlebox solve then set mode solved
@reaction puzzlebox solve then announce "The box chimes. You solved it."
@reaction puzzlebox solve enable
```

These steps cancel the pending reset, mark the puzzle solved, and announce success. Without the
cancellation, the timer can later change a solved puzzle back to idle.

#### Solve it in time

Close the new box first. It starts idle, so this closing does not solve anything:

```text
close puzzlebox
```

Open it, inspect its state, then close it within thirty seconds:

```text
open puzzlebox
@state puzzlebox
close puzzlebox
@state puzzlebox
@reaction puzzlebox arm show
```

The first state report shows `armed`. Closing the box produces the success message. The second
report shows `solved`, and the reaction shows `Pending timers: none`.

If you take too long, the state returns to `idle` and closing produces no success message. Open the
closed box again to retry.

#### Let the timer expire

Use a shorter timer to test the other outcome. Change its message to match:

```text
@reaction puzzlebox arm step 2 schedule reset 3 set mode idle
@reaction puzzlebox arm step 3 announce "Close the box within three seconds."
open puzzlebox
```

Leave the box open for at least four seconds. Then enter:

```text
@state puzzlebox
@reaction puzzlebox arm trace
close puzzlebox
@state puzzlebox
```

The state is `idle`, and the trace includes `Timer reset: finished`. Closing the box produces no
success message. Its state stays idle because the `solve` condition no longer matches.

Restore the thirty-second version:

```text
@reaction puzzlebox arm step 2 schedule reset 30 set mode idle
@reaction puzzlebox arm step 3 announce "Close the box within thirty seconds."
```

The opening reaction has no `if mode is idle` condition. If you add that condition, the timer checks
it again before the reset. The condition fails because step 1 changes the mode to `armed`. The reset
then cannot happen.

## Reuse an idea on another object

You can copy a reaction to another object you own. The copy starts disabled, so you can inspect it
before it responds. Later edits to either reaction leave the other copy unchanged.

In the copy, `this` refers to the new object. A reference to a particular cat still names that cat.
A new object starts the copied reaction's state at its initial values. Compatible state already on
the destination keeps its current values.

A **setting** gives a name to a choice in the reaction. For example, `friend` can name the cat in a
cupboard's condition. That condition can then ask whether `friend` is in the room. Another copy can
choose a different cat without rewriting that condition.

The event supplies the current participants. Settings hold your chosen people, objects, or values.
State holds information that changes as the interaction progresses.

### Tutorial 5: Copy a response to another object

Return to the `welcome` reaction from tutorials 1–3. It now greets on every opening and counts its
greetings. Create another box and copy that reaction:

```text
@create $container named sparebox
@reaction clubbox welcome copy to sparebox
@reaction sparebox welcome show
@state sparebox
```

The copy starts disabled. Its counter reads `greetings: 0`, the starting value you declared. It does
not inherit the cupboard's current count.

Give the copy its own message. Enable it, then try it:

```text
@reaction sparebox welcome step 1 announce "Welcome to the spare box."
@reaction sparebox welcome enable
close sparebox
open sparebox
@state sparebox
@state clubbox
```

The spare box announces its own message and reaches `greetings: 1`. The original cupboard's count
stays at one, where tutorial 3 left it. The two counters are independent. The original reaction's
message also stays unchanged.

Copying `welcome` copies that reaction and its declared state. It does not copy the cupboard's
membership rule or its separate `remember` reaction. You can choose which other instructions the
spare box needs.

## Build, try, and understand what happened

You create these interactions through ordinary text commands. You can enter them in Meadow, the
browser interface, or through a text terminal connection. The command families correspond to the
parts described here:

| Command     | Purpose                                           |
| ----------- | ------------------------------------------------- |
| `@rule`     | Set or inspect a condition for an action.         |
| `@reaction` | Create, edit, inspect, copy, or stop a response.  |
| `@state`    | Declare or inspect an object's remembered values. |
| `@facts`    | Define and maintain facts in a relation.          |

New reactions start disabled. You can inspect their steps, check for mistakes, and preview them
before enabling them. An enabled reaction responds to future events. Enabling it does not replay
events that already happened.

These tools answer different questions:

| Tool      | What you learn                                                       |
| --------- | -------------------------------------------------------------------- |
| `show`    | What event, condition, settings, and steps did I choose?             |
| `check`   | Are the parts valid, with the required information?                  |
| `preview` | With the current facts, which steps does this reaction plan to take? |
| `trace`   | On its latest run, what matched, what completed, and what failed?    |

A preview changes no state and sends no messages to other people. It does not perform the actions or
guarantee their permission checks will pass. A trace records an actual attempt. You turn tracing on
before the interaction you want to examine.

Steps run in their numbered order. Reactions for the same event run in name order. A failed step
stops later steps. Earlier work can remain, including an already opened cupboard. Fixing a failed
welcome message does not undo the opening.

Disabling a reaction stops future responses and cancels its pending timers. It leaves the object,
its state, and already recorded facts in place.

## Work within a shared world

The authoring commands let builders edit objects they own. Other people can still use those objects
according to their rules. Owning a cupboard gives you control of its responses, but does not give
you control of every visitor.

A reaction uses the permissions of the person who owns its object. Naming a visitor as the
participant does not grant the reaction that visitor's permissions. An action still needs someone
permitted to perform it, a reachable target, and any required items. A reaction can direct a
character you own. It cannot make another player hand over their possessions.

The commands offer a defined set of events, conditions, and actions. Programmers can extend that set
in MOO, the world's programming language. Builders can then combine the available parts.

## Continue building

You can adapt these examples by choosing a different event, condition, or step. The
[builder's guide](../manual/reactions.md) covers more command options, including combined conditions
and named settings.

For an in-world introduction, enter `help @reaction`. The command `@reaction events` lists the
available events, and `@reaction steps` lists the kinds of response you can add.
