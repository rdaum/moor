# Let's make a pet rock

Let's make you a pet rock. First, you'll create it and give it a description. Then you'll make it
respond when you pet it. We'll add a few things for it to say, teach it to count, and give it a way
to remember visitors.

You don't need to know MOO code yet. We'll start small and explain the pieces as we use them. You
will write a little code, but many later changes use commands that require no programming.

This lesson takes inspiration from [Yib's Pet Rock](https://lisdude.com/moo/yib_rock.html), a
beginner's introduction to programming on LambdaMOO. The commands here are for Cowbell, a ready-made
world for the [mooR server](../../../README.md).

Names do not need a leading `a` or `the`. Cowbell supplies articles when a message asks for them.
See [names and grammar](names-and-grammar.md) for aliases, plural names, and room-listing choices.

## Before you begin

You'll need an account in a Cowbell world, with both builder and programmer privileges. Ask the
person who runs your world if you need those privileges. Pick a room where you can experiment
without interrupting other people.

Follow the sections in order. They all work on the same rock. We will give it the short alias `rock`
to keep the commands easy to type.

In the examples, `%` marks a command to type. `=` marks the response you should see. Leave those
markers out when you type. A line starting with `~~` shows only part of a response, such as
`Created`. Your object's number will differ from ours.

Some sections invite another person to try your rock. We call its owner Ada and the visitor Bert.
Use your own accounts. If you're working alone, you can still follow the owner's steps.

<details>
<summary>How this page tests its own examples</summary>

The project's test runner follows these same blocks in a fresh world. The collapsible test checks
are instructions for that runner. You do not type them into the MOO. They select Ada or Bert, check
what each person sees, and check the rock's stored values.

This setup names the test accounts and gives Ada builder and programmer privileges. It leaves her
wizard flag off. The lesson itself creates and programs the rock.

```moot
@wizard
; #90101.name = "Ada"; #90102.name = "Bert"; #90101.is_builder = true; #90101.authoring_features = $prog_features; return true;
true
@programmer
; return player.programmer && !player.wizard;
true
```

</details>

## 1. Make a rock

Type this command:

```moot
% @create $thing named "pet rock",rock
~~Created
```

You now own an object called `pet rock`. It starts in your inventory. The comma adds `rock` as an
**alias**: another name you can use to refer to the same object. An object has one main name and can
have several aliases.

Object names can contain spaces. We will use the shorter alias `rock` in commands throughout this
lesson. Messages will still call it `pet rock`.

An object has **properties** and **verbs**. A property holds something the object remembers, such as
its name or description. A verb holds instructions for something the object can do. Your rock
already has properties and verbs, because it starts from an existing object.

`@create` makes a new object. `$thing` is Cowbell's starting point for ordinary things: it already
knows how to be picked up and dropped. Your rock gets that behavior from `$thing`. Later, we'll use
your rock as the starting point for another one. Names such as `$thing` refer to objects supplied by
Cowbell. Keep the `$` when you type them.

That starting object is called a **prototype**, and the new object is its **child**. The rock can
use verbs from its prototype without needing a separate copy of each program. We will return to that
idea after your rock has some behavior worth sharing.

Before we go further, try a handy tool:

```moot
% #rock.name
==> "pet rock"
% #rock.aliases
==> {"rock"}
```

`#` is a builder's Swiss army knife for finding objects and doing things with them. `#rock` finds
your rock by its alias. The dot means “get this property from that object.”

The `.name` property holds the main name, `"pet rock"`. This is a **string**: a piece of text. The
`.aliases` property holds a **list** of other names, here `{"rock"}`. Braces mark a list; commas
separate its entries when there is more than one. An object without aliases has an empty list, `{}`.

An alias does not create another object or change its main name. Both `rock` and `pet rock` refer to
this same stone. Later, `pet rock` will use the alias: `pet` is the command and `rock` is its
object.

Type `help #` for help inside the world. The [object shortcuts tutorial](object-shortcuts.md) gives
a fuller tour.

Give it a description:

```moot
% @describe rock as A smooth stone with an expression of patient indifference.
~~Set description
```

The description is what people read when they look at the rock. Feel free to change it later. For
now, let's give it something to do.

## 2. Pet the rock

We need two things: a message to show people, and a command that shows it.

First, write the message that people will see:

```moot
% @set-message rock.pet_msg {The actor} {agree actor name:pet|pets} {the d}.
~~Set message template
```

`pet_msg` is a name for this message. `@set-message` creates it on the rock. Keeping the wording
separate from the command lets you change it later without changing the code.

The text looks peculiar because it has blanks for Cowbell to fill in:

| Part                           | What it supplies                                                                  |
| ------------------------------ | --------------------------------------------------------------------------------- |
| `{The actor}`                  | The person petting the rock: “You” to that person, or their name to someone else. |
| `{agree actor name:pet\|pets}` | The verb form that fits: “You pet” or “Ada pets.”                                 |
| `{the d}`                      | The identified object: “the pet rock.”                                            |

One message can now read correctly to everyone in the room. This is a **message template**: it
describes the sentence before we know who will pet the rock. The verb will supply that person when
it uses the message. Cowbell already treats the rock as the object described by `{d}`.

### Add the pet command

MOO calls an object's pieces of code **verbs**. A verb can implement a command that people type.
Let's add `pet`:

```moot
% @verb rock:pet this none none rd
~~added
```

The colon in `rock:pet` means “the `pet` verb on the object called `rock`.” The three fields
`this none none` describe the words after `pet`: this object, no preposition such as `with`, and no
second object. That gives us `pet rock`. `r` makes the code readable, and `d` makes programming
errors visible.

### Read a few instructions before entering the program

The verb exists, but its body is empty. The **body** is the sequence of instructions that runs when
someone types `pet rock`. MOO runs those instructions from top to bottom.

For this command, MOO gives us two useful names. `player` refers to the person who typed the
command. `this` refers to the rock whose verb is running. These names refer to objects, not to text
containing their names. If Bert pets your rock, `player` means Bert and `this` still means your
rock.

The next examples explain pieces of the program. You do not need to enter them separately. The
complete body follows them.

```moo example
const who = player;
const room = who.location;
```

A **variable** is a name for a value used by the running program. `const who = player;` creates one
called `who` and gives it the value of `player`. The equals sign assigns the value on its right to
the name on its left. `const` means this variable will keep that value during this run. The
semicolon ends the instruction.

The next line reads the person's `location` property and names that room `room`. Here, a property
holds another object, rather than text. These variables last only during this run of the verb. The
rock's properties remain after the command finishes.

We want a person to pet a rock they carry or can reach in the room. A **condition** asks a question
with a yes-or-no answer. MOO calls those answers `true` and `false`.

```moo example
if (!(this.location in {who, room}))
  return false;
endif
```

`{who, room}` is a **list** containing two objects: the person and their room. `in` asks whether the
rock's location is in that list. `!` means “not,” so the condition is true when the rock is
somewhere else. The parentheses group the question that `!` reverses.

An `if` runs the indented instructions only when its condition is true. `endif` marks their end.
`return false;` stops the verb and reports that it did not succeed. Returning a value does not print
it. If the rock is within reach, this `if` does nothing and the program continues.

Next, we need to prepare our message and send it:

```moo example
const message = this:message_event(who, "pet_msg");
room:announce(message);
```

A colon means “call this verb on that object.” Calling a verb asks it to run its instructions. The
values inside parentheses are its **arguments**: information we give it to do its job. Commas
separate arguments.

`this:message_event(who, "pet_msg")` asks the rock to prepare its `pet_msg` message for this person.
That verb returns the prepared message, which we save in `message`. We then pass that message to the
room's `announce` verb. The room delivers it to the people there.

Notice the difference between `who` and `"pet_msg"`. `who` supplies the object saved in a variable.
`"pet_msg"` supplies the literal name of the message property we want to use.

### Enter the complete program

There is one additional check at the start. `caller` identifies the object that called the verb.
Cowbell sets it to `player` for a typed command. `caller != player` means “the caller is different
from the player.” We reject that case because this verb expects to run as a typed command. This does
not restrict petting to the rock's owner. Other players can type `pet rock` too.

`raise` stops the program with an error. `E_PERM` is the name of a permission error, and the text
explains the problem. It is useful when a programmer calls this command verb in the wrong way.

Type `@program rock:pet`, then enter the complete body below. Finish with a line containing only a
period (`.`). The first line is a description for people reading the code. It is a string on its
own, so it does not send a message to anyone.

```moo verb=rock:pet
"Pet a nearby rock.";
if (caller != player)
  raise(E_PERM, "Use the pet command to pet this rock.");
endif
const who = player;
const room = who.location;
if (!(this.location in {who, room}))
  return false;
endif
const message = this:message_event(who, "pet_msg");
room:announce(message);
return true;
```

<details>
<summary>Test check: the verb compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

Cowbell should report that it programmed the lines. If it reports an error, check the named line for
a missing semicolon, quote, or bracket. You can enter `@program rock:pet` again to replace the body.
An error here usually means a small typing mistake.

Now try your new command:

```moot
% pet rock
=You pet the pet rock.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@programmer
```

</details>

There: you have a pet rock. It hasn't moved, which seems appropriate. You see “You pet the pet
rock.” Someone else in the room sees “Ada pets pet rock,” with your name in place of Ada's.

Read the body again with that result in mind. It checks the caller, names the person and room,
checks reach, and sends the message. The final `return true;` reports success and ends the verb.

If you lose track of what a name means, look for its declaration. For example, `message` gets its
value from `message_event`, and the next line passes that value to `announce`. Following values
through a few lines is often more useful than trying to memorize every verb name.

## 3. Give it a memory

Let's have the rock count how often someone pets it, then make a remark.

The pet command currently prints a message and finishes. To add other behavior, we need it to notify
the rock that a pet happened. A **reaction** will then respond by increasing a counter and making a
remark.

### Tell the rock what happened

We also want to let the rock respond to a pet. We will send it an **event**: a notification that
something happened, together with information about who did it and where.

```moo example
this:fire_trigger('on_pet, ['Actor -> who, 'Location -> room]);
```

`fire_trigger` is the verb that delivers this notification to the rock. Its first argument names the
event, `on_pet`. The leading apostrophe makes `'on_pet` a **symbol**, a fixed label used by code. A
symbol has no closing apostrophe. It is not a variable and does not change from run to run.

The second argument is a **map**. A map associates labels with values. Square brackets enclose it,
and `->` separates each label from its value. Here, `'Actor` identifies the person who petted the
rock, and `'Location` identifies the room. Together, these details are the event's **context**. Use
the labels `Actor` and `Location` as shown. `fire_trigger` expects these names for those details.

Sending the event does not itself print anything. We have not given the rock a response yet. Next,
we will add a reaction that listens for this event.

Add the event to the end of the pet verb, just before `return true;`. Enter `@program rock:pet` and
replace the body with this version. Everything before `fire_trigger` is the same program you just
tried.

```moo verb=rock:pet
"Pet a nearby rock and let its reactions respond.";
if (caller != player)
  raise(E_PERM, "Use the pet command to pet this rock.");
endif
const who = player;
const room = who.location;
if (!(this.location in {who, room}))
  return false;
endif
const message = this:message_event(who, "pet_msg");
room:announce(message);
this:fire_trigger('on_pet, ['Actor -> who, 'Location -> room]);
return true;
```

<details>
<summary>Test check: the event version compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

### Give the event a response

A reaction is a saved set of steps that runs after an event. Ours will add one to a counter and
announce a message. First, make the counter and the message:

```moot
% @state rock pets number 0
~~Declared
% @set-message rock.greeting_msg The rock remains impressively still.
~~Set message template
```

`@state` gives the rock a value that it remembers between commands. Here, `pets` is a number that
starts at zero. The message property works just like `pet_msg`.

We will call the reaction `greeting`. That is its name on this rock, so we can find and edit it
later. It is separate from the event name, `pet`. Several reactions can listen for the same event.

Now choose the event and the two steps:

```moot
% @reaction rock greeting after pet
~~Saved
% @reaction rock greeting then add pets 1
~~Saved
% @reaction rock greeting then announce message greeting_msg
~~Saved
% @reaction rock greeting enable
~~Saved
```

`after pet` responds to the `on_pet` event sent by our command. `add pets 1` increases the counter.
`announce message greeting_msg` uses the message we just wrote. It reads the current wording each
time, so later edits take effect without changing the reaction.

Each command edits the same reaction, `greeting`, on the same object, `pet rock`. Each `then` adds a
step to the end. The counter therefore changes before the announcement runs.

New reactions start disabled, so an unfinished reaction cannot respond halfway through editing.
`enable` starts this one after we finish its steps. These steps become part of the rock's saved
behavior. You do not need to type the setup commands each time someone pets it.

Try the rock, then look at its counter:

```moot
% pet rock
=You pet the pet rock.
=The rock remains impressively still.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock remains impressively still.
@programmer
```

</details>

```moot
% @state rock
=pets: 1
```

The `pet` command still sends its usual message. The reaction adds the remark and increases the
counter. You have added a response without rewriting what petting means.

### Change its mind

Suppose the rock needs a different line. Change the message and pet it again:

```moot
% @set-message rock.greeting_msg The rock accepts the compliment in silence.
~~Set message template
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

You changed the response without editing either the verb or the reaction. The reaction refers to the
message by name, so it reads the new wording each time.

Before you pet the rock a third time, predict which messages you will see and what `pets` will
contain. Follow the command through the event and both reaction steps to explain your prediction.
You can use `@state rock` to inspect the value whenever you need it.

The counter belongs to this reaction too. If you disable the reaction, both the remark and its
counting step stop. The `pet` command itself still works.

## 4. Ask for a reminder

Your rock is patient, but perhaps it would appreciate another visit. Let's send the petter a private
reminder three seconds after petting.

Give the reminder its own message:

```moot
% @set-message rock.reminder_msg Your rock still appreciates the attention.
~~Set message template
```

This time we will use `actor`, the reaction's name for the person who caused the event. It comes
from the `'Actor` value that our pet verb put in the event's context. The reaction can use that
person without knowing their name in advance.

Install a second reaction:

```moot
% @reaction rock reminder after pet
~~Saved
% @reaction rock reminder then schedule reminder 3 tell actor message reminder_msg
~~Saved
% @reaction rock reminder enable
~~Saved
```

The step `schedule` creates a **timer**. Its name is `reminder`, and `3` is the delay in seconds.
When it runs, `tell` sends `reminder_msg` to `actor`, the person who petted the rock. It does not
announce the reminder to everyone in the room.

Pet the rock, then wait a few seconds:

```moot
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: the petter gets the reminder; the visitor does not</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock accepts the compliment in silence.
@programmer
@text-within 5 Your rock still appreciates the attention.
@nonprogrammer
@quiet
@programmer
```

</details>

You should receive “Your rock still appreciates the attention.”

There is one timer called `reminder` on this rock. Petting again before it runs replaces the pending
reminder. It does not queue a separate reminder for every pet.

Stop the reminder before continuing:

```moot
% @reaction rock reminder disable
~~disabled
```

Disabling a reaction also cancels its pending timers. Moving the rock can prevent a pending timer
from acting, and editing the reaction cancels its timers.

## 5. A rock with boundaries

Perhaps your rock only wants attention from you. A **rule** answers whether an action is allowed.
Our command must ask the rule before it announces a successful pet.

Give the rock a place for that rule and messages for refused attempts:

```moot
% @property rock.pet_rule 0 rc
~~Added
```

```moot
% @set-message rock.pet_denied_msg {The d} declines your attention.
~~Set message template
% @set-message rock.pet_out_of_reach_msg {dc} is out of reach.
~~Set message template
% @set-message rock.pet_changed_msg The situation changed. Try again.
~~Set message template
```

`@property` adds a named property to the rock. Its initial value is `0`. The `rc` flags let people
read the property and give children their own ownership of inherited values. You own the rock, so
you can change this property. The `@rule` command will store the chosen rule there.

A `pet_rule` of zero means there is no special restriction. `{dc}` means the rock's name with a
capital letter, for the start of a sentence.

### Ask a question before announcing success

The earlier verb always permits a reachable pet. We will insert a decision before its message and
event. The rest of the world can keep using `pet rock` while we change how that decision works.

A rule and a reaction have different jobs. The rule asks, “May this person pet the rock?” A reaction
says, “Someone petted the rock; now do these steps.” If the rule says no, there is no successful pet
for our reactions to respond to.

This is the decision we will add. Read this excerpt first; the complete replacement follows.

```moo example
const policy = this.pet_rule;
let allowed = true;
if (policy != 0)
  const result = $rule_engine:evaluate(policy, ['This -> this, 'Accessor -> who]);
  allowed = result['success];
endif
```

`policy` saves the rule from the rock. `allowed` starts as `true`, because a zero rule means
unrestricted access. We use `let` for `allowed`, instead of `const`, because we may change its
value.

`policy != 0` asks whether there is a rule to evaluate. `$rule_engine` is Cowbell's object for
answering rule questions. Its `evaluate` verb takes the rule and a map of the objects involved.
`'This` identifies the rock; `'Accessor` identifies the person requesting access.

The result is another map. `result['success]` reads its yes-or-no answer, and the assignment puts
that answer in `allowed`. These square brackets read one map entry. The brackets with arrows in the
preceding line create a map.

If `allowed` is false, the verb sends the refusal message to `who` and returns before reaching the
successful message. `inform_current` sends a private message to that person's current connection.
`message_event` accepts an optional third argument, `'error`, to mark a refusal.

### Keep the decision about the current situation

The complete version also records the rock's owner, both locations, and the rule in a list named
`before`. After evaluating the rule, it compares those saved values with the current properties.
`equal` compares the two lists. If they differ, the verb asks the person to try again.

Why check twice? Rules can call other code, and that code can wait. During a wait, someone can move
the rock or change its rule. A decision made before that change must not authorize a pet afterward.
Our rule in this lesson does not wait, but the verb also needs to work with later rules.

The ownership check at the start has a different purpose. A verb runs with its owner's permissions,
even when a visitor types the command. `this.owner` is the rock's owner; `task_perms()[1]` reads the
first item in the permission information, the person whose permissions this code uses. For this
example, they must be the same person. This permits visitors to pet your rock while its code updates
state that you own. It also catches a mismatch before announcing a successful pet.

Enter `@program rock:pet` again, paste this complete body, and finish with a period on its own line.
This replaces only the verb's instructions. Your messages, reactions, and counter stay in place.

```moo verb=rock:pet
"Pet a reachable rock, subject to its access rule.";
if (caller != player)
  raise(E_PERM, "Use the pet command to pet this rock.");
endif
if (this.owner != task_perms()[1])
  raise(E_PERM, "This rock and its pet verb have different owners. Ask the rock's owner to check the verb.");
endif
const who = player;
const room = who.location;
if (!valid(room) || !isa(room, $room)
    || !(this.location in {who, room}))
  who:inform_current(this:message_event(who, "pet_out_of_reach_msg", 'error));
  return false;
endif
const policy = this.pet_rule;
const before = {this.owner, this.location, who.location, policy};
let allowed = true;
if (policy != 0)
  const result = $rule_engine:evaluate(policy, ['This -> this, 'Accessor -> who]);
  allowed = result['success];
endif
if (!equal(before, {this.owner, this.location, who.location, this.pet_rule}))
  who:inform_current(this:message_event(who, "pet_changed_msg", 'error));
  return false;
endif
if (!allowed)
  who:inform_current(this:message_event(who, "pet_denied_msg", 'error));
  return false;
endif
room:announce(this:message_event(who, "pet_msg"));
this:fire_trigger('on_pet, ['Actor -> who, 'Location -> room]);
return true;
```

<details>
<summary>Test check: the replacement verb compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

The reach check now also uses `valid(room)` and `isa(room, $room)`. `valid` asks whether the object
still exists. `isa` asks whether it comes from the room prototype. `||` means “or,” so any failed
reach check is enough to send `pet_out_of_reach_msg` and stop.

The new part asks `$rule_engine` to evaluate `pet_rule`. It supplies `This` (the rock) and
`Accessor` (the person trying to pet it). If the rule refuses, the command shows `pet_denied_msg`
and stops before firing `on_pet`.

### Who can pet it, and whose permissions does it use?

MOO runs a verb with its verb owner's permissions. That owner can be different from the person who
typed the command. If Ada owns the rock and its `pet` verb, Bert can still pet it. During Bert's
command, `player` is Bert, but the verb runs with Ada's permissions.

`fire_trigger` checks permissions because it can start reactions that change the rock's state or
write to its visitor book. It requires the rock owner's permissions or wizard permissions. That is
why our command requires the rock and verb to have the same owner. The access rule still decides
whether Bert may pet it.

A child owned by someone else still inherits the original verb and its owner's permissions. That
child fails our ownership check. The error asks its owner to check the verb. We'll use a child you
own later in this lesson.

The `E_PERM` errors concern how the verb is called or who owns it. An ordinary refusal to let
someone pet the rock gets the message in `pet_denied_msg`, with no programming error.

### Try an owner-only rule

Allow only yourself:

```moot
% @rule rock pet actor is me
~~Set pet rule
% @rule rock pet show
~~pet: actor is
```

Here, `actor` means the person attempting the action. `me` saves your identity when you write the
rule. It does not change to mean each new visitor.

Put the rock in the room so Bert can reach it:

```moot
% drop rock
=You dropped the pet rock.
```

<details>
<summary>Test setup: select the visitor</summary>

```moot
@nonprogrammer
@text Ada dropped the pet rock.
```

</details>

Ask Bert to type:

```moot
% pet rock
=The pet rock declines your attention.
```

<details>
<summary>Test check: a refused pet does not increase the count</summary>

```moot
@programmer
% @state rock
=pets: 3
```

</details>

Bert gets a refusal, with no remark and no extra count. A reaction follows a successful action; the
rule decides whether that action can happen at all.

Let everyone pet the rock again:

```moot
% @rule rock pet clear
~~Restored default access
```

Ask Bert to try again:

<details>
<summary>Test setup: select the visitor</summary>

```moot
@nonprogrammer
```

</details>

```moot
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: Ada sees the pet, and the count increases</summary>

```moot
@programmer
@text Bert pets the pet rock.
@text The rock accepts the compliment in silence.
% @state rock
=pets: 4
```

</details>

Bert can now pet Ada's rock, and its reaction runs. The ownership check still passes. You can change
the rule whenever you like. The verb still asks the same question.

## 6. Remember who visited

A counter tells us how many pets the rock received. It doesn't tell us who visited. For that, let's
give it a visitor book.

We'll use a **relation**: an object that stores rows of related values. Our rows will have two
fields, `person` and `rock`. One row means “this person visited this rock.” A relation keeps each
row only once, even if someone visits repeatedly.

Think of the book as a table. After Ada and Bert each pet the same rock, it can contain:

| person | rock     |
| ------ | -------- |
| Ada    | pet rock |
| Bert   | pet rock |

Each row records a relationship between two particular objects. Names help us read the table, but
the stored values identify the objects themselves. Renaming Ada or the rock does not erase their
visit. A second pet by Ada does not create a second copy of the first row.

Create the book and tell a reaction to write in it:

```moot
% @create $relation named visitor book
~~Created
% @facts "visitor book" define person:object rock:object
~~Defined fields
% @reaction rock remember after pet
~~disabled
% @reaction rock remember with visitors object "visitor book"
~~Saved
% @reaction rock remember then record in visitors person=actor rock=this
~~Saved
% @reaction rock remember enable
~~enabled
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
% @facts "visitor book"
~~Facts in visitor book
=Fields: person:object, rock:object
~~person=Ada
```

`@facts ... define` names the fields. Both have type `object`, because they hold references to
things in the world rather than names typed as text.

`@reaction rock remember after pet` creates a disabled reaction. The
`with visitors object "visitor book"` line gives it a setting called `visitors`, pointing to our
book. The `record` step writes the current person (`actor`) and rock (`this`) into that book.
`enable` lets the reaction start responding.

Pet the rock again. It should remember one visit from you, rather than make a duplicate row:

```moot
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
% @facts "visitor book"
~~Facts in visitor book
~~Fields:
~~person=Ada
```

The visitor book now shows one row for you and your rock. Petting it again does not add a duplicate.

You can also tell the reaction to run only for a new visitor:

```moot
% @reaction rock remember if NOT fact in visitors person=actor rock=this
~~Saved
```

Read this as “run if there is not already a fact for this person and this rock.” The counter can
still increase on later pets, because counting belongs to the separate greeting reaction.

Another rock could use the same book, or its own book with the same field names. The `visitors`
setting makes that choice explicit. A field named `person` does not make unrelated visitor books
share their records.

## 7. Give it a little variety

The same remark every time gets familiar. A **message bag** holds several messages, and Cowbell
chooses one when a reaction uses the bag.

Add two possible remarks:

```moot
% @add-message rock.remarks_msg_bag The rock considers this a satisfactory encounter.
~~Added
% @add-message rock.remarks_msg_bag The rock maintains its geological composure.
~~Added
```

The `_msg_bag` suffix identifies the property as a message bag. Install a reaction that refers to
it, just as the greeting referred to `greeting_msg`:

```moot
% @reaction rock remark after pet
~~Saved
% @reaction rock remark then announce message remarks_msg_bag
~~Saved
% @reaction rock remark enable
~~Saved
```

Pet the rock. You should see one of the two new remarks, along with its usual greeting.

```moot
% pet rock
=You pet the pet rock.
=The rock accepts the compliment in silence.
~~The rock
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pet rock.
@text The rock accepts the compliment in silence.
~~The rock
@programmer
```

</details>

```moot
% @get-message rock.remarks_msg_bag
~~2 entries
~~Template
~~The rock considers this a satisfactory encounter.
~~The rock maintains its geological composure.
% @reaction rock remark disable
~~disabled
```

The last command disables this extra reaction so it won't distract us in the next section. A random
choice can repeat, so hearing the same remark twice is normal.

You can change the wording with `@set-message` and add more choices with `@add-message`. The
reaction keeps using the same named message bag.

## 8. A pebble of its own

Your rock can now serve as a **prototype**: an object that another object gets behavior from. This
is how your rock got ordinary item behavior from `$thing`. Let's make a child called `pebble`.

Reset the parent's counter first so it's easy to compare the two:

```moot
% @state rock pets set 0
=Set pets to 0.
% @create rock named pebble
~~Created
```

<details>
<summary>Test check: the inherited reaction starts inactive</summary>

```moot
; const pebble = $match:match_object("pebble", player); return $reaction:_authority(pebble, "greeting_reaction", pebble.greeting_reaction) == 0;
true
```

</details>

The pebble inherits the pet verb, message properties, and reaction definitions. Inherited code is
shared: we don't need to type the pet verb again.

Its greeting reaction still needs to be enabled on the new object. Enable it, pet the pebble, and
compare the counters:

```moot
% @reaction pebble greeting enable
~~enabled
% pet pebble
=You pet the pebble.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pebble.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
% @state pebble
=pets: 1
% @state rock
=pets: 0
```

The pebble has one pet. The parent still has zero. Each has its own remembered state.

Give the pebble a different greeting:

```moot
% @set-message pebble.greeting_msg The pebble seems unusually sociable.
~~Set message template
% pet pebble
=You pet the pebble.
=The pebble seems unusually sociable.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets the pebble.
@text The pebble seems unusually sociable.
@programmer
```

</details>

```moot
% @get-message rock.greeting_msg
~~The rock accepts the compliment in silence.
% @state pebble
=pets: 2
% @state rock
=pets: 0
```

The pebble uses the inherited verb but its own message. Its parent keeps the original greeting. You
now have two objects that share behavior without sharing every choice.

The other inherited reactions remain inactive until you enable them too. Enabling the greeting does
not also enable the visitor book or reminder.

## Where to go next

You have made an object, added a command, changed its messages, installed reactions, set an access
rule, kept visitor records, and made a child object.

Try changing one choice at a time. Give the pebble a different remark. Set a rule that only lets you
pet it. Use a separate visitor book for it. Predict what will happen, then try it and inspect the
result. That is a useful way to learn what each part does.

For a larger project, [make room for Henri](henri-and-the-cupboard.md). You will build a cat, a food
cupboard, and a couch, then teach those objects to respond to each other.

The [rules and reactions introduction](rules-reactions-relations.md) has more examples, including a
cupboard and a timed puzzle. The [command guide](../manual/reactions.md) lists the builder commands
in more detail. Yib's original tutorial goes on to a rock-eating creature; you can read it for
another set of experiments.

If something goes wrong, keep the command you typed and the complete error message. Note which
object you used and what you expected to happen. Those details make it much easier for another
programmer to help.

<details>
<summary>Notes for sharing this as a prototype</summary>

The examples work for objects and children that you own. Other people can pet them. A child owned by
someone else inherits the original verb's permissions and fails our ownership check. Removing that
check does not give the verb the child's owner permissions. The `fire_trigger` permission check
still applies.

The `pet` command also does not register a reaction action named `pet`. Making `then do pet ...`
available requires a registered action, with its own argument and permission checks. For a shared
prototype, both entry points can call the same checked operation.

A copied reaction keeps its chosen relation reference and starts disabled. To give the copy its own
visitor book, create a relation with the same fields and change its `visitors` setting. Copying a
reaction does not copy its stored facts or grant access to another owner's relation.

For the data structures used by MOO programmers, see the
[programmer reference](../manual/behavior-model.md).

</details>
