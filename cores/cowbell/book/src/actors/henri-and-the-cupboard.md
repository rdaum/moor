# Let's make room for a cat

Your pet rock asks very little of you. Henri has higher expectations. He wants somewhere to sleep,
food in the cupboard, and a say in whether anyone pets him.

In this lesson, you will make Henri and furnish his corner of the world. Opening his food cupboard
will attract his attention. Feeding him will improve his mood. His couch will have room for
visitors, although he will have opinions about them.

This continues [Let's make a pet rock](../programming/pet-rock.md). It shares that lesson's
inspiration, [Yib's Pet Rock tutorial](https://lisdude.com/moo/yib_rock.html). You do not need to
bring your rock along. The examples here create a separate set of objects.

The rock lesson introduces properties, verbs, variables, and events. Here we use those ideas to make
several objects cooperate. You will write two verbs: one for petting and one for deciding which
gifts Henri accepts. Commands will supply his moods, rules, reactions, furniture, and schedule.

Names do not need a leading `a` or `the`. Cowbell supplies articles when a message asks for them.
See [names and grammar](../building/names-and-grammar.md) for aliases, plural names, and
room-listing choices.

## Before you begin

You need builder and programmer privileges, as in the rock lesson. You do not need wizard
privileges. Choose a room where you can leave a few objects and experiment.

Keep the example names while you follow the lesson. Commands such as `@reaction Henri` use those
names to find your objects. Another cat named Henri in the same room will make that confusing.

As before, `%` marks a command to type. `=` marks the response, and `~~` matches part of a response.
Leave those markers out. The collapsible test checks are for the test runner; you can skip them.

For a longer program, enter the named `@program` command. Paste the code, then finish with a period
on a line by itself. Code marked as an excerpt explains part of a program; the complete body is
separately identified when it is time to enter it.

<details>
<summary>Test setup: an ordinary programmer and a visitor</summary>

```moot
@wizard
; #90101.name = "Ada"; #90102.name = "Bert"; #90101.is_builder = true; #90101.authoring_features = $prog_features; return true;
true
@programmer
; return player.programmer && !player.wizard;
true
```

</details>

## 1. Meet Henri

Create a cat:

```moot
% @create $actor named Henri
~~Created
% @describe Henri as A black cat with green eyes and the firm conviction that this is his room.
~~Set description
```

The rock started from `$thing`. Henri starts from `$actor`, the prototype for people and other
characters. An actor can receive things and take a seat. Henri has no person connected to him. The
commands and reactions you write will supply his behaviour.

Give Henri his pronouns with the builder command:

```moot
% @set-pronouns Henri to he/him
=Pronouns for Henri set to: he/him
```

This command works on actors you own and needs no programmer privileges. For custom words, use
`@set-pronouns Henri to custom`. It asks you to fill in sentences, then shows a preview before
saving. Type `help @set-pronouns` for details. `@pronouns` sets your own pronouns.

For a quick look at an object's properties, use `#Henri.name`. `#` is a handy utility for finding
objects and inspecting them. Type `help #` for its other uses, or follow the
[object shortcuts tutorial](../programming/object-shortcuts.md).

New objects start in your inventory. Move Henri into the room:

```moot
% @move Henri to here
~~Moved Henri
```

You can now look at him. So far, he has no particular objection to anything. Let us correct that.

## 2. Ask before petting

The rock lesson introduced a `pet` command, a message, and an access rule. Henri uses the same
pieces. He also announces an **attempt** before checking his rule. This lets him remember an
approach even when he refuses the pet.

Add a place for his petting rule:

```moot
% @property Henri.pet_rule 0 rc
~~Added
```

A rule of `0` permits the operation. We will give Henri a reason to refuse later.

Write the messages:

```moot
% @set-message Henri.pet_msg {The actor} {agree actor name:pet|pets} {the d}.
~~Set message template
% @set-message Henri.pet_denied_msg {The d} hisses and keeps his distance.
~~Set message template
% @set-message Henri.pet_changed_msg Something changed. Try petting {the d} again.
~~Set message template
% @set-message Henri.pet_out_of_reach_msg Move closer to {the d} first.
~~Set message template
% @verb Henri:pet this none none rd
~~added
```

`{The actor}` names the person, and `{the d}` names the cat. These forms add an article for a common
noun, while preserving a name such as Henri. `agree` selects “pet” or “pets” to fit the actor. The
capitalized form also starts the sentence correctly. The first message says “You pet Henri.” to the
person and “Ada pets Henri.” to a visitor.

### An attempt and a successful action are different events

Suppose someone reaches toward a grouchy cat. Henri can notice the approach even if he refuses the
pet. We need to remember the attempt before we ask whether petting is allowed.

The command will do these things in order:

1. Check that the person and Henri are in the same room.
2. Send `on_pet_attempt` to Henri. Reactions to this event run now.
3. Ask his current `pet_rule` whether the pet is allowed.
4. If the rule refuses, show the refusal message and stop.
5. Otherwise, announce the pet and send `on_pet`.

That order matters. Later, an attempt will change Henri's mood. The rule must see his new mood
before it decides whether to allow that same pet.

Here is the new part of the program, shown as an excerpt:

```moo example
const context = ['Actor -> who, 'Location -> room];
this:fire_trigger('on_pet_attempt, context);
```

As in the rock lesson, the map holds the person and room. We save it in a variable called `context`
because both events need the same information. `fire_trigger` calls the reactions that listen for
`on_pet_attempt`. When they finish, the next instruction reads `this.pet_rule`. The verb later uses
the same context for `on_pet`, but only after the rule allows the action.

We keep the earlier checks for reach, ownership, and changes during rule evaluation. The rule
result's `'success` field becomes the yes-or-no value in `allowed`. If it is false, the verb sends
`pet_denied_msg` privately. If it is true, the verb announces `pet_msg` to everyone in the room.

Enter `@program Henri:pet`, then this complete body:

```moo verb=Henri:pet
"Pet a nearby cat if his current rule permits it.";
if (caller != player)
  raise(E_PERM, "Use the pet command to pet this cat.");
endif
if (this.owner != task_perms()[1])
  raise(E_PERM, "This cat and its pet verb need the same owner. Ask the owner to check the verb.");
endif
const who = player;
const room = who.location;
if (!valid(room) || !isa(room, $room) || this.location != room)
  who:inform_current(this:message_event(who, "pet_out_of_reach_msg", 'error));
  return false;
endif
const context = ['Actor -> who, 'Location -> room];
this:fire_trigger('on_pet_attempt, context);
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
this:fire_trigger('on_pet, context);
return true;
```

<details>
<summary>Test check: the pet command compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

There are two events. `on_pet_attempt` means someone nearby tried. `on_pet` means the rule allowed
it. An event name belongs to the object receiving it. Another cat can use these names without
sharing Henri's reactions or state.

The ownership check concerns the program's permissions. It does not require the visitor to own
Henri. Your verb runs with your permissions, even when someone else pets him. This example requires
you to own both the cat and the verb, just like the rock's final pet command.

The `before` check catches changes during a rule evaluation. A rule can call other code, which can
wait while the world changes. We must not announce success using a decision about an old location or
rule.

Try it:

```moot
% pet Henri
=You pet Henri.
```

## 3. Give him a mood

Henri's mood must last between commands. A local variable in `pet` would disappear when the verb
finishes, so we store the mood as a property through `@state`.

We will use a **choice** for the mood: one of a fixed set of words. This catches mistakes such as
setting it to `slepy` when we meant `sleepy`. We also need a number to count petting attempts.
Declare both values, then give his pet rule a condition:

```moot
% @state Henri mood choice grouchy sleepy playful
~~Declared mood
% @state Henri pet_attempts number 0
~~Declared pet_attempts
% @rule Henri pet NOT fact property_is object=this field=mood value=grouchy
~~Set
```

The first choice, `grouchy`, is the initial mood. The rule permits a pet only when Henri is not
grouchy. Read the rule from the inside out:

- `fact property_is object=this field=mood value=grouchy` asks whether this cat's mood is `grouchy`.
- `NOT` reverses the answer. A grouchy cat refuses; a cat in either other mood permits the pet.

`property_is` is a built-in question about an object's current property value. We use `this` for
Henri because the rule belongs to him. There is no cat named `this` to create or find. The rule
reads the property each time someone tries to pet him.

Count each attempt:

```moot
% @reaction Henri count_attempt after pet_attempt
~~Saved
% @reaction Henri count_attempt then add pet_attempts 1
~~Saved
% @reaction Henri count_attempt enable
~~Saved
```

Now give his patience a limit:

```moot
% @reaction Henri enough when pet_attempts >= 4
~~Saved
% @reaction Henri enough then set mood sleepy
~~Saved
% @reaction Henri enough then set pet_attempts 0
~~Saved
% @reaction Henri enough then emote "exhausts his capacity for outrage and settles into a sleepy loaf."
~~Saved
% @reaction Henri enough enable
~~Saved
```

This is a **threshold reaction**. It runs when the counter crosses from less than four to four or
more. Its steps change the mood, reset the counter, and describe the result.

An event reaction starts because something happened, such as a petting attempt. This threshold
reaction starts because remembered state crosses a boundary. Resetting the counter to zero lets it
cross that boundary again after four more attempts.

The threshold value is a choice we made for this cat. The `pet` verb knows nothing about four
attempts or sleepy cats. Those choices live in the reactions, where you can change them later.

An `emote` step supplies the object's name. The quoted text is a message template, so it can use
pronouns as well as plain words. The message stays outside the pet command's code.

A reaction can also have a **condition**. The event makes it eligible to run, and the condition
decides whether its steps fit the current situation. Henri needs one response while sleepy and
another while playful.

Add different responses for successful pets:

```moot
% @reaction Henri sleepy_pet after pet
~~Saved
% @reaction Henri sleepy_pet if mood is sleepy
~~Saved
% @reaction Henri sleepy_pet then emote "tolerates the petting with half-closed eyes."
~~Saved
% @reaction Henri sleepy_pet enable
~~Saved
% @reaction Henri playful_pet after pet
~~Saved
% @reaction Henri playful_pet if mood is playful
~~Saved
% @reaction Henri playful_pet then emote "bats lazily at the offered hand."
~~Saved
% @reaction Henri playful_pet enable
~~Saved
```

Try four pets:

```moot
% pet Henri
=Henri hisses and keeps his distance.
% pet Henri
=Henri hisses and keeps his distance.
% pet Henri
=Henri hisses and keeps his distance.
% pet Henri
=Henri exhausts his capacity for outrage and settles into a sleepy loaf.
=You pet Henri.
=Henri tolerates the petting with half-closed eyes.
```

The fourth attempt changes his mood before the rule is checked. That pet succeeds. The other three
attempts still count, but never produce a successful pet event.

Follow the fourth command through the pieces: `on_pet_attempt` increases the counter to four. The
threshold reaction makes Henri sleepy and resets the counter. The pet rule then sees a sleepy cat
and allows the pet. Finally, `on_pet` reaches the sleepy response. Nothing in the playful response
runs, because its condition is false.

You can change a state value directly while you experiment:

```moot
% @state Henri mood set playful
~~Set mood
% pet Henri
=You pet Henri.
=Henri bats lazily at the offered hand.
```

<details>
<summary>Test checks: state belongs to the cat, and a visitor can pet him</summary>

```moot
; const cat = $match:match_object("Henri", player); return {cat.mood, cat.pet_attempts};
{"playful", 1}
@attach pet_visitor nonprogrammer
@pet_visitor
% pet Henri
=You pet Henri.
=Henri bats lazily at the offered hand.
@programmer
@text Bert pets Henri.
@text Henri bats lazily at the offered hand.
```

</details>

## 4. Stock the cupboard

Create a cupboard and a key:

```moot
% @create $container named cupboard
~~Created
% @describe cupboard as A wooden food cupboard with a small brass lock.
~~Set description
% @create $thing named brass key
~~Created
% @describe brass key as A small brass key. It fits the food cupboard.
~~Set description
```

The cupboard already knows how to open, close, hold things, and use a lock. These behaviours come
from `$container`. We only need to choose its rules and messages.

Make a kind of food, then three meals of that kind:

```moot
% @create $thing named kibble
~~Created
% @describe kibble as A can of premium cat kibble. The label promises feline approval.
~~Set description
% @create kibble named breakfast
~~Created
% @create kibble named lunch
~~Created
% @create kibble named supper
~~Created
% put breakfast in cupboard
~~You put the breakfast in the cupboard.
% put lunch in cupboard
~~You put the lunch in the cupboard.
% put supper in cupboard
~~You put the supper in the cupboard.
% drop cupboard
~~You dropped the cupboard.
% @rule cupboard get never
~~Set
% @rule cupboard lock key is "brass key"
~~Set
% @rule cupboard unlock key is "brass key"
~~Set
% @set-message cupboard.open_msg {The actor} {agree actor name:open|opens} {the d}, releasing a familiar smell of cat food.
~~Set message template
% close cupboard
~~You close the cupboard.
% lock cupboard with brass key
~~You lock the cupboard with the brass key.
% open cupboard
~~The cupboard is locked.
```

`kibble` is now your own prototype. Its children share its description and inherited behaviour. They
remain separate objects. Henri will recognize descendants of this prototype as food, whatever you
name them.

The `get never` rule stops visitors from picking up the furniture. It does not prevent them from
opening it. The lock and unlock rules select your particular key. Another object named `brass key`
will not open this cupboard.

## 5. Dinner improves his mood

Use the ordinary `give` command to feed Henri: `give supper to Henri`. That command already checks
whether the giver carries the item and stands near the recipient. It also respects the item's drop
rule.

First, tell Henri which kind of food he accepts:

```moot
% @state Henri food_kind object kibble
~~Declared food_kind
% #Henri.food_kind.name
==> "kibble"
% @verb Henri:acceptable this none this rxd
~~added
```

`food_kind` remembers the kibble prototype itself. Renaming the prototype later will not break the
connection. `#Henri.food_kind.name` follows that reference and shows its name.

### Answer a question from another verb

Before moving an item, `give` calls the recipient's `acceptable` verb. This is a **method**: a verb
that other code calls, rather than a command a person types. It receives the proposed gift as an
argument and returns `true` or `false`.

This is a useful place for a small custom program. The existing gift operation asks this method for
its answer. Our answer needs to recognize a family of food objects. After writing that test once, we
can choose another food prototype through `@state` without changing the method.

Read the three lines before entering them. The first describes the method for readers. The second
names the argument, and the third returns the answer. There are no messages or movements here:
`give` uses this answer to decide whether it can proceed.

Enter `@program Henri:acceptable` and supply this body:

```moo verb=Henri:acceptable
"Accept only descendants of this cat's food prototype.";
const {item} = args;
return valid(this.food_kind) && isa(item, this.food_kind);
```

<details>
<summary>Test check: the acceptance method compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

`args` is MOO's list of the arguments supplied to a verb. `const {item} = args;` takes the one
argument from that list and names it `item`. The braces on the left describe the list we expect.
This method expects exactly one item.

`valid(this.food_kind)` asks whether the food prototype still exists. `isa(item, this.food_kind)`
asks whether the offered item is that prototype or one of its descendants. Unlike a method call, a
built-in function such as `isa` has no object and colon before its name.

`&&` means “and.” Both tests must be true for the method to return `true`. For `supper`, which we
created from `kibble`, the answer is true. For `brass key`, it is false. We test both below.

This replaces the actor's default acceptance method on Henri alone. Other actors keep their usual
behavior. The `food_kind` property lets us change Henri's preferred family of food later.

An object receives an `enter` event when something moves into it. For this event, `who` identifies
the arriving object. Add a reaction for arriving food:

```moot
% @reaction Henri dinner after enter
~~Saved
% @reaction Henri dinner then set mood sleepy
~~Saved
% @reaction Henri dinner then set pet_attempts 0
~~Saved
% @reaction Henri dinner then emote "eats the kibble and curls up, looking sleepy."
~~Saved
% @reaction Henri dinner enable
~~Saved
% give brass key to Henri
~~That recipient can't accept the item.
% unlock cupboard with brass key
~~You unlock the cupboard with the brass key.
% open cupboard
~~You open the cupboard, releasing a familiar smell of cat food.
% take supper from cupboard
~~You take the supper from the cupboard.
% give supper to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give the supper to Henri.
% pet Henri
=You pet Henri.
=Henri tolerates the petting with half-closed eyes.
```

The acceptance method decides what can enter. The reaction responds after the item arrives. Changing
Henri's mood is a response to the completed gift, not part of deciding whether to accept it.

The food now lives inside Henri. We describe it as eaten, but keep the object there. This lesson
does not destroy objects that someone else may own. A later game can decide what happens to empty
cans, leftovers, or a cat's inventory.

<details>
<summary>Test checks: refused food stays carried, and visitors can feed the cat</summary>

```moot
; const cat = $match:match_object("Henri", player); return {$match:match_object("brass key", player).location == player, cat.pet_attempts, cat.mood};
{true, 1, "sleepy"}
@attach dinner_visitor nonprogrammer
@dinner_visitor
% take lunch from cupboard
~~You take the lunch from the cupboard.
% give lunch to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give the lunch to Henri.
@programmer
@text Bert takes the lunch from the cupboard.
@text Henri eats the kibble and curls up, looking sleepy.
@text Bert gives the lunch to Henri.
; const cat = $match:match_object("Henri", player); return {length(cat.contents), cat.pet_attempts};
{2, 0}
```

</details>

## 6. Let the cupboard tell him

Henri currently notices food only when it reaches him. A real cat usually notices much sooner. Let
the cupboard tell him when someone opens it.

The cupboard and cat are separate objects. Opening the cupboard sends an `open` event to the
cupboard, not to every object in the room. We will add a cupboard reaction that forwards the news to
Henri, then give Henri his own response. This lets each object keep its own behavior.

Give Henri a response to a new event named `cupboard_open`:

```moot
% @reaction Henri ears after cupboard_open
~~Saved
% @reaction Henri ears if this is in place
~~Saved
% @reaction Henri ears then emote "swivels {p} ears toward the cupboard."
~~Saved
% @reaction Henri ears enable
~~Saved
```

For an emote, `{p}` means the reacting object's possessive pronoun: Henri swivels “his” ears. The
condition keeps him quiet when the cupboard opens in another room. `place` is the room where the
original event happened.

Now tell the cupboard to send that event when someone opens it:

```moot
% @reaction cupboard notify_cat after open
~~Saved
% @reaction cupboard notify_cat then trigger Henri cupboard_open
~~Saved
% @reaction cupboard notify_cat enable
~~Saved
```

`trigger Henri cupboard_open` tells Henri that the cupboard opened. His `ears` reaction responds to
that event. The original person and room travel with the event, so he can check whether he is
nearby. The command remembers this particular cat, even if you rename him later.

You own both ends of this connection. Sending a trigger can change its recipient's state, so the
sender needs the recipient owner's permissions. Merely knowing another cat's object number does not
give your cupboard control over it.

Try the connection:

```moot
% close cupboard
~~You close the cupboard.
% open cupboard
=Henri swivels his ears toward the cupboard.
=You open the cupboard, releasing a familiar smell of cat food.
```

Use `@reaction cupboard notify_cat show` to inspect the connection.

<details>
<summary>Test check: a cupboard in another room does not make Henri speak here</summary>

```moot
; const cat = $match:match_object("Henri", player); add_property(player, "tutorial_cat", cat, {player, ""}); move(cat, #90110); return true;
true
% close cupboard
~~You close the cupboard.
% open cupboard
=You open the cupboard, releasing a familiar smell of cat food.
@quiet
; move(player.tutorial_cat, player.location); return true;
true
```

</details>

## 7. His couch, apparently

Create a seat and leave it in the room:

```moot
% @create $sittable named couch
~~Created
% @describe couch as A worn brown couch. Black cat hair occupies most of the cushions.
~~Set description
% drop couch
~~You dropped the couch.
% @rule couch get never
~~Set
```

The `$sittable` prototype supplies sitting and standing. Give Henri a seat through the same action
interface that reactions use:

```moot
% @act Henri sit target=couch
~~Henri
~~Done: Henri sit.
```

<details>
<summary>Test check: Henri really took a seat</summary>

```moot
; const couch = $match:match_object("couch", player); return $match:match_object("Henri", player) in couch.sitting;
1
```

</details>

`@act Henri` chooses who acts; `sit` chooses the action; `target=couch` chooses the seat. You can
direct Henri because you own him. You cannot direct another player's character.

Give Henri a response when someone joins him:

```moot
% @reaction Henri company after couch_intruder
~~Saved
% @reaction Henri company if this is in place
~~Saved
% @reaction Henri company then emote "shifts pointedly away from the new arrival. This is HIS couch."
~~Saved
% @reaction Henri company enable
~~Saved
```

Connect the couch to the cat. This reaction has a condition: Henri must be sitting there, and the
new sitter must be someone else.

The world already records who sits on which seat. We can ask for that **fact**, just as the rock
lesson asked its visitor book whether someone had visited. Here the shared fact is called `sitting`.
Its two fields are `actor`, the sitter, and `seat`, the furniture.

The reaction's setting `cat` will refer to Henri. In `actor=cat`, the left side selects the fact's
field and the right side supplies the object to look for. In `seat=this`, `this` is the couch
because the reaction belongs to the couch. The final `AND actor is not cat` excludes Henri's own
arrival: the event's `actor` must be someone else.

```moot
% @reaction couch company after sit
~~Saved
% @reaction couch company with cat object Henri
~~Saved
% @reaction couch company if fact sitting actor=cat seat=this AND actor is not cat
~~Saved
% @reaction couch company then trigger cat couch_intruder
~~Saved
% @reaction couch company enable
~~Saved
% sit on couch
=You sit on the couch.
=Henri shifts pointedly away from the new arrival. This is HIS couch.
% stand from couch
=You stand up from the couch.
```

`with cat object Henri` gives this reaction a setting named `cat`. Here, `this` is the couch and
`actor` is the new sitter. The condition checks that Henri already sits there and the newcomer is
someone else. The final step sends the cat an event about his new company.

Food can persuade Henri to leave his seat. Add a response on the cat:

```moot
% @reaction Henri leave_seat after food_taken
~~Saved
% @reaction Henri leave_seat with seat object couch
~~Saved
% @reaction Henri leave_seat if this is in place AND fact sitting actor=this seat=seat
~~Saved
% @reaction Henri leave_seat then do stand target=seat actor=this
~~Saved
% @reaction Henri leave_seat then emote "perks up, suddenly interested in dinner."
~~Saved
% @reaction Henri leave_seat enable
~~Saved
```

In this reaction, `this` means Henri, because it belongs to him. `with seat object couch` saves the
couch as a setting named `seat`. The condition checks that Henri is nearby and sitting there.
`do stand target=seat actor=this` asks Henri to stand from that seat. The action performs the normal
standing checks and updates the world's sitting fact. It does more than print a sentence.

The cupboard can send this notification whenever someone takes something from it. Henri expects food
from this cupboard. He can be disappointed if you keep something else there.

```moot
% @reaction cupboard dinner_bell after take
~~Saved
% @reaction cupboard dinner_bell then trigger Henri food_taken
~~Saved
% @reaction cupboard dinner_bell enable
~~Saved
% take breakfast from cupboard
=Henri stands up from the couch.
=Henri perks up, suddenly interested in dinner.
=You take the breakfast from the cupboard.
% give breakfast to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give the breakfast to Henri.
```

That response crosses three objects: the cupboard reports the take, Henri reacts, and the couch
performs the stand. Each object keeps its own job. You can change Henri's response without changing
the cupboard's opening or taking code.

## 8. Give him something to do between visitors

This part is optional. Henri already responds to pets, food, and the furniture. A recurring
**schedule** lets him do something when nobody types a command.

Start with one idle reaction:

```moot
% @reaction Henri idle after idle
~~Saved
% @reaction Henri idle then emote "grooms one paw, then inspects the room with renewed disappointment."
~~Saved
% @reaction Henri idle enable
~~Saved
```

Send an `idle` event roughly every five minutes:

```moot
% @schedule Henri idle every 300 jitter 60
~~Scheduled Henri idle
% @schedule Henri idle
~~Henri idle: every 300
```

Times are in seconds. `jitter 60` varies the wait by up to a minute, so Henri does not groom himself
like clockwork. Leave that part out for a regular interval. The schedule sends the event; the
reaction supplies his response. You can change either one separately.

Repeating the start command replaces your schedule for that object and event. It does not start a
second copy. Schedules survive a server restart and stop if the object's ownership changes.

Keep the scene quiet while you finish the lesson:

```moot
% @schedule Henri idle stop
~~Stopped schedule for Henri idle.
% @schedule Henri idle
=No schedule for Henri idle.
```

You can use the start command again whenever you want. His other reactions keep working while the
idle schedule is stopped. `@schedules` lists your schedules.

<details>
<summary>Test check: the scheduled event reaches the reaction</summary>

```moot
% @schedule Henri idle every 0.1
~~Scheduled Henri idle
@text-within 5 Henri grooms one paw, then inspects the room with renewed disappointment.
% @schedule Henri idle stop
~~Stopped schedule for Henri idle.
```

</details>

## What to try next

You now have a small scene whose parts respond to each other. Henri's moods and responses live in
rules and reactions. His few verbs connect new operations to those facilities.

Try one change at a time:

- Change a reaction's message with `step`, then try the event again.
- Add a `curious` mood and decide what makes him curious.
- Give another cat its own cupboard and couch. Connect those objects to that cat, not to Henri.
- Add a condition to the cupboard's dinner bell so only kibble interests him.
- Give the idle reaction a condition, so he grooms only while he is awake.

`@reaction OBJECT NAME show` shows a reaction's definition. `preview` explains its planned steps
without performing them. `trace on` records what happens on the next event. These tools help you
find a wrong condition or object reference without guessing.

The [builder's guide](../behaviour/reactions.md) explains those commands and adds relations and
timers. The internal
[rule and reaction notes](https://github.com/timbran-project/moor/blob/main/cores/cowbell/manual/behavior-model.md)
include proposed records as well as current implementation details. Those proposals are not needed
for this lesson.
