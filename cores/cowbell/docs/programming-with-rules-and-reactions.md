# Let's make a pet rock

Let's make you a pet rock. First, you'll create it and give it a description. Then you'll make it
respond when you pet it. We'll add a few things for it to say, teach it to count, and give it a way
to remember visitors.

You don't need to know MOO code yet. We'll start small and explain the pieces as we use them. You
will write a little code, but many later changes use commands that require no programming.

This lesson takes inspiration from [Yib's Pet Rock](https://lisdude.com/moo/yib_rock.html), a
beginner's introduction to programming on LambdaMOO. The commands here are for Cowbell, a ready-made
world for the [mooR server](../../../README.md).

## Before you begin

You'll need an account in a Cowbell world, with both builder and programmer privileges. Ask the
person who runs your world if you need those privileges. Pick a room where you can experiment
without interrupting other people.

Follow the sections in order. They all work on the same rock. Keep the name `petrock` while you
follow the lesson, so the later commands can find it.

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
% @create $thing named petrock
~~Created
```

You now own an object called `petrock`. It starts in your inventory.

`@create` makes a new object. `$thing` is Cowbell's starting point for ordinary things: it already
knows how to be picked up and dropped. Your rock gets that behavior from `$thing`. Later, we'll use
your rock as the starting point for another one.

Before we go further, try a handy tool:

```moot
% #petrock.name
==> "petrock"
```

`#` is a builder's Swiss army knife for finding objects and doing things with them. `#petrock` finds
your rock; `.name` reads its name property. Later, we'll use the same command to change values and
call verbs. Type `help #` for help inside the world. The
[object shortcuts tutorial](object-shortcuts.md) gives a fuller tour.

Give it a description:

```moot
% @describe petrock as A smooth stone with an expression of patient indifference.
~~Set description
```

The description is what people read when they look at the rock. Feel free to change it later. For
now, let's give it something to do.

## 2. Pet the rock

We need two things: a message to show people, and a command that shows it.

An object keeps information in **properties**. Its name and description are properties. We'll add
one called `pet_msg` for the message:

```moot
% @property petrock.pet_msg {} rc
~~Added
```

The dot in `petrock.pet_msg` means “the `pet_msg` property on petrock.” `{}` starts it as an empty
list. The `rc` flags let people read the property and let future child objects own their copy. Keep
those flags as shown for this lesson.

Before we write the message, Cowbell needs to know who it can mention. Here, that is the person
petting the rock and the rock itself. We tell the editor once so it can catch mistakes in later
edits.

MOO calls an object's pieces of code **verbs**. Add a verb that describes these participants:

```moot
% @verb petrock:message_bindings this none this rxd
~~added
```

The editor calls this verb. You do not type its name as a command. The `x` flag lets other code call
it.

Type `@program petrock:message_bindings`, then enter this code. Finish with a line containing only a
period (`.`).

```moo verb=petrock:message_bindings
"Describe the participants in the pet command's messages.";
const {property} = args;
if (!property:starts_with("pet_") || !property:ends_with("_msg"))
  return pass(@args);
endif
let participants = $sub_utils:standard_bindings("The person petting the rock.");
participants['dobj] = ['kind -> 'object, 'description -> "The rock being petted.",
  'example -> 'host];
return participants;
```

<details>
<summary>Test check: the participant declaration compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

`property` is the name of the message property being edited. Our petting messages start with `pet_`
and end with `_msg`. They identify the person petting the rock as `actor`, and the rock as `dobj`.
The `example` entry tells previews to use the rock for `dobj`.

Other messages use `$thing`'s declarations through `pass(@args)`. This keeps inherited messages,
such as the pickup message, working as before. The declaration describes the participants. The `pet`
verb below will supply them when someone uses the command.

Now write the message:

```moot
% @set-message petrock.pet_msg {nc} {pet|pets} {d}.
~~Set message template
```

The text looks peculiar because it has blanks for Cowbell to fill in:

| Part          | What it supplies                                                                  |
| ------------- | --------------------------------------------------------------------------------- |
| `{nc}`        | The person petting the rock: “You” to that person, or their name to someone else. |
| `{pet\|pets}` | The verb form that fits: “You pet” or “Ada pets.”                                 |
| `{d}`         | The thing being petted.                                                           |

One message can now read correctly to everyone in the room. We also keep it separate from the code,
so changing the wording won't require reprogramming the command.

### Add the pet command

Some verbs implement commands that people type. Let's add `pet`:

```moot
% @verb petrock:pet this none none rd
~~added
```

The colon in `petrock:pet` means “the `pet` verb on petrock.” `this none none` tells the command
parser to accept `pet petrock`, with no further arguments. `r` makes the code readable, and `d`
makes programming errors visible.

The verb exists, but it needs instructions. Type `@program petrock:pet`, then enter this code.
Finish with a line containing only a period (`.`).

```moo verb=petrock:pet
"Pet a nearby rock and let its reactions respond.";
caller == player || raise(E_PERM, "Use the pet command to pet this rock.");
const who = player;
const room = who.location;
if (!(this.location in {who, room}))
  return false;
endif
room:announce($event:mk_info(who, @this.pet_msg):with_dobj(this));
this:fire_trigger('on_pet, ['Actor -> who, 'Location -> room]);
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
a missing semicolon, quote, or bracket. You can enter `@program petrock:pet` again to replace the
body. An error here usually means a small typing mistake.

Now try your new command:

```moot
% pet petrock
=You pet petrock.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@programmer
```

</details>

There: you have a pet rock. It hasn't moved, which seems appropriate. You see “You pet petrock.”
Someone else in the room sees “Ada pets petrock,” with your name in place of Ada's.

### What did that code do?

`player` is the person who typed the command. `this` is the object running the verb: your rock.
`who` and `room` are names we give to that person and their current room. A `const` declaration
saves a value under a name so later lines can use it.

The `if` checks that the rock is in the person's inventory or room. If it isn't, `return false` ends
the command. Otherwise, `room:announce(...)` sends the petting message to the room. The `@` before
`this.pet_msg` supplies the message's pieces, and `with_dobj(this)` identifies the rock for the
`{d}` substitution.

`fire_trigger` tells the rock that someone petted it. We call this **event** `on_pet`. Other
behavior can respond to that event. Nothing responds yet. Let's change that.

`caller` is the object that called this verb. For a typed command, Cowbell sets it to `player`. The
check `caller == player` rejects calls from another object. We wrote this version for use through
the `pet` command. Other players can use that command too.

`condition || raise(...)` stops with an error when the condition is false. `E_PERM` names a
permission error, and the text after it explains what went wrong. Here, the error tells a programmer
who called the verb from another object to use the command instead.

## 3. Give it a memory

Let's have the rock count how often someone pets it, then make a remark.

A **reaction** is a set of steps that runs after an event. Our event is `on_pet`. The steps will add
one to a counter and announce a message.

First, make the counter and the message:

```moot
% @state petrock pets number 0
~~Declared
% @property petrock.greeting_msg {} rc
~~Added
% @set-message petrock.greeting_msg The rock remains impressively still.
~~Set message template
```

`@state` gives the rock a value that it remembers between commands. Here, `pets` is a number that
starts at zero. The message property works just like `pet_msg`.

Next, install the reaction. `#petrock` finds your rock by name. After `for`, you can run a MOO
expression with `%#` standing for the object you found. Enter this command; it returns `true`:

```moot
% #petrock for $reaction:install(%#, "greeting_reaction", $reaction:mk('on_pet, 0, {{'increment, 'pets, 1}, {'announce, 'greeting_msg}}, $reaction:state_types(%#))).enabled
==> true
```

The reaction command is longer, so let's take it apart:

- `#petrock` finds your rock. `%#` supplies that object to the expression.
- `$reaction:mk(...)` makes a reaction for `on_pet`. The `0` means it has no extra condition.
- `{'increment, 'pets, 1}` adds one to the counter.
- `{'announce, 'greeting_msg}` announces the message stored in `greeting_msg`.
- `$reaction:state_types(%#)` lets Cowbell check that the counter is a number.
- `$reaction:install(...)` puts the reaction on your rock and authorizes it to run there.

The leading apostrophe in `'pets` makes it a **symbol**: a name the code uses to identify something.
Quoted text such as `"petrock"` is a **string**. A list uses braces, as in the two steps above. You
don't need to memorize all this syntax at once.

Try the rock, then look at its counter:

```moot
% pet petrock
=You pet petrock.
=The rock remains impressively still.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@text The rock remains impressively still.
@programmer
```

</details>

```moot
% @state petrock
=pets: 1
```

The `pet` command still sends its usual message. The reaction adds the remark and increases the
counter. You have added a response without rewriting what petting means.

### Change its mind

Suppose the rock needs a different line. Change the message and pet it again:

```moot
% @set-message petrock.greeting_msg The rock accepts the compliment in silence.
~~Set message template
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

You changed the response without editing either the verb or the reaction. The reaction refers to the
message by name, so it reads the new wording each time.

The counter belongs to this reaction too. If you disable the reaction, both the remark and its
counting step stop. The `pet` command itself still works.

## 4. Ask for a reminder

Your rock is patient, but perhaps it would appreciate another visit. Let's send the petter a private
reminder three seconds after petting.

Give the reminder its own message:

```moot
% @property petrock.reminder_msg {} rc
~~Added
% @set-message petrock.reminder_msg Your rock still appreciates the attention.
~~Set message template
```

Install a second reaction:

```moot
% #petrock for $reaction:install(%#, "reminder_reaction", $reaction:mk('on_pet, 0, {{'schedule, 'reminder, 3, {'tell, 'Actor, 'reminder_msg}}})).enabled
==> true
```

The step `schedule` creates a **timer**. Its name is `reminder`, and `3` is the delay in seconds.
When it runs, `tell` sends `reminder_msg` to `Actor`, the person who petted the rock. It does not
announce the reminder to everyone in the room.

Pet the rock, then wait a few seconds:

```moot
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: the petter gets the reminder; the visitor does not</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
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
% @reaction petrock reminder disable
~~disabled
```

Disabling a reaction also cancels its pending timers. Moving the rock can prevent a pending timer
from acting, and editing the reaction cancels its timers.

## 5. A rock with boundaries

Perhaps your rock only wants attention from you. A **rule** answers whether an action is allowed.
Our command must ask the rule before it announces a successful pet.

Give the rock a place for that rule and messages for refused attempts:

```moot
% @property petrock.pet_rule 0 rc
~~Added
```

```moot
% @property petrock.pet_denied_msg {} rc
~~Added
% @property petrock.pet_out_of_reach_msg {} rc
~~Added
% @property petrock.pet_changed_msg {} rc
~~Added
% @set-message petrock.pet_denied_msg {dc} declines your attention.
~~Set message template
% @set-message petrock.pet_out_of_reach_msg {dc} is out of reach.
~~Set message template
% @set-message petrock.pet_changed_msg The situation changed. Try again.
~~Set message template
```

A `pet_rule` of zero means there is no special restriction. `{dc}` means the rock's name with a
capital letter, for the start of a sentence.

Now replace the pet verb with this version. Enter `@program petrock:pet` again, paste the complete
body, and finish with a period on its own line. The existing reactions and counter will stay in
place.

```moo verb=petrock:pet
"Pet a reachable rock, subject to its access rule.";
caller == player || raise(E_PERM, "Use the pet command to pet this rock.");
this.owner == task_perms()[1]
  || raise(E_PERM, "This rock and its pet verb have different owners. Ask the rock's owner to check the verb.");
const who = player;
const room = who.location;
if (!valid(room) || !isa(room, $room)
    || !(this.location in {who, room}))
  who:inform_current($event:mk_error(who, @this.pet_out_of_reach_msg):with_dobj(this));
  return false;
endif
const policy = this.pet_rule;
const before = {this.owner, this.location, who.location, policy};
const allowed = policy == 0 || $rule_engine:evaluate(policy,
  ['This -> this, 'Accessor -> who])['success];
if (!equal(before, {this.owner, this.location, who.location, this.pet_rule}))
  who:inform_current($event:mk_error(who, @this.pet_changed_msg):with_dobj(this));
  return false;
endif
if (!allowed)
  who:inform_current($event:mk_error(who, @this.pet_denied_msg):with_dobj(this));
  return false;
endif
room:announce($event:mk_info(who, @this.pet_msg):with_dobj(this));
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

The new part asks `$rule_engine` to evaluate `pet_rule`. It supplies `This` (the rock) and
`Accessor` (the person trying to pet it). If the rule refuses, the command shows `pet_denied_msg`
and stops before firing `on_pet`.

### Who can pet it, and whose permissions does it use?

MOO runs a verb with its verb owner's permissions. That owner can be different from the person who
typed the command. If Ada owns the rock and its `pet` verb, Bert can still pet it. During Bert's
command, `player` is Bert, but the verb runs with Ada's permissions.

The new ownership check compares `this.owner`, the rock's owner, with `task_perms()[1]`, the person
whose permissions the code uses. For Ada's rock, both are Ada, even when Bert pets it. The access
rule decides whether Bert is allowed to pet it.

`fire_trigger` checks permissions because it can start reactions that change the rock's state or
write to its visitor book. It requires the rock owner's permissions or wizard permissions. For this
example, we require the verb and rock to have the same owner. This catches an ownership mismatch
before the command announces a successful pet. It is a restriction of our example, not a general
requirement for MOO verbs.

A child owned by someone else still inherits the original verb and its owner's permissions. That
child fails our ownership check. The error asks its owner to check the verb. We'll use a child you
own later in this lesson.

The `E_PERM` errors concern how the verb is called or who owns it. An ordinary refusal to let
someone pet the rock gets the message in `pet_denied_msg`, with no programming error.

The reach check makes sure the person can reach the rock. The `before` comparison catches changes to
ownership, location, or the rule while the rule runs, and asks the person to try again.

### Try an owner-only rule

Allow only yourself:

```moot
% @rule petrock pet actor is me
~~Set pet rule
% @rule petrock pet show
~~pet: actor is
```

Here, `actor` means the person attempting the action. `me` saves your identity when you write the
rule. It does not change to mean each new visitor.

Put the rock in the room so Bert can reach it:

```moot
% drop petrock
=You dropped petrock.
```

<details>
<summary>Test setup: select the visitor</summary>

```moot
@nonprogrammer
@text Ada dropped petrock.
```

</details>

Ask Bert to type:

```moot
% pet petrock
=Petrock declines your attention.
```

<details>
<summary>Test check: a refused pet does not increase the count</summary>

```moot
@programmer
% @state petrock
=pets: 3
```

</details>

Bert gets a refusal, with no remark and no extra count. A reaction follows a successful action; the
rule decides whether that action can happen at all.

Let everyone pet the rock again:

```moot
% @rule petrock pet clear
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
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: Ada sees the pet, and the count increases</summary>

```moot
@programmer
@text Bert pets petrock.
@text The rock accepts the compliment in silence.
% @state petrock
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

Create the book and tell a reaction to write in it:

```moot
% @create $relation named rockvisitors
~~Created
% @facts rockvisitors define person:object rock:object
~~Defined fields
% @reaction petrock remember after pet
~~disabled
% @reaction petrock remember with visitors object rockvisitors
~~Saved
% @reaction petrock remember then record in visitors person=actor rock=this
~~Saved
% @reaction petrock remember enable
~~enabled
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
% @facts rockvisitors
~~Facts in rockvisitors
=Fields: person:object, rock:object
~~person=Ada
```

`@facts ... define` names the fields. Both have type `object`, because they hold references to
things in the world rather than names typed as text.

`@reaction petrock remember after pet` creates a disabled reaction. The
`with visitors object rockvisitors` line gives it a setting called `visitors`, pointing to our book.
The `record` step writes the current person (`actor`) and rock (`this`) into that book. `enable`
lets the reaction start responding.

Pet the rock again. It should remember one visit from you, rather than make a duplicate row:

```moot
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
; const rock = $match:match_object("petrock", player); const visitors = $match:match_object("rockvisitors", player); return $relation:tuples(visitors) == {{player, rock}};
true
```

The check at the end compares the book's contents with one row containing you and your rock.

You can also tell the reaction to run only for a new visitor:

```moot
% @reaction petrock remember if NOT fact in visitors person=actor rock=this
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
% @add-message petrock.remarks_msg_bag The rock considers this a satisfactory encounter.
~~Added
% @add-message petrock.remarks_msg_bag The rock maintains its geological composure.
~~Added
```

The `_msg_bag` suffix identifies the property as a message bag. Install a reaction that refers to
it, just as the greeting referred to `greeting_msg`:

```moot
% #petrock for $reaction:install(%#, "remark_reaction", $reaction:mk('on_pet, 0, {{'announce, 'remarks_msg_bag}})).enabled
==> true
```

Pet the rock. You should see one of the two new remarks, along with its usual greeting.

```moot
% pet petrock
=You pet petrock.
=The rock accepts the compliment in silence.
~~The rock
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets petrock.
@text The rock accepts the compliment in silence.
~~The rock
@programmer
```

</details>

```moot
; const rock = $match:match_object("petrock", player); return length(rock.remarks_msg_bag:entries());
2
% @reaction petrock remark disable
~~disabled
```

The last command disables this extra reaction so it won't distract us in the next section. A random
choice can repeat, so hearing the same remark twice is normal.

We've used a little MOO code to install reactions that read named message properties. Builders can
then change those messages with `@set-message` and `@add-message`. They don't need to edit the code
we wrote.

## 8. A pebble of its own

Your rock can now serve as a **prototype**: an object that another object gets behavior from. This
is how your rock got ordinary item behavior from `$thing`. Let's make a child called `pebble`.

Reset the parent's counter first so it's easy to compare the two:

```moot
% @state petrock pets set 0
=Set pets to 0.
% @create petrock named pebble
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
=You pet pebble.
=The rock accepts the compliment in silence.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets pebble.
@text The rock accepts the compliment in silence.
@programmer
```

</details>

```moot
% @state pebble
=pets: 1
% @state petrock
=pets: 0
```

The pebble has one pet. The parent still has zero. Each has its own remembered state.

Give the pebble a different greeting:

```moot
% @set-message pebble.greeting_msg The pebble seems unusually sociable.
~~Set message template
% pet pebble
=You pet pebble.
=The pebble seems unusually sociable.
```

<details>
<summary>Test check: what the other person sees</summary>

```moot
@nonprogrammer
@text Ada pets pebble.
@text The pebble seems unusually sociable.
@programmer
```

</details>

```moot
% #petrock.greeting_msg for $sub_utils:decompile(%#)
==> "The rock accepts the compliment in silence."
% @state pebble
=pets: 2
% @state petrock
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
