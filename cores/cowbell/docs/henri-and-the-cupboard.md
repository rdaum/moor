# Let's make room for a cat

Your pet rock asks very little of you. Henri has higher expectations. He wants somewhere to sleep,
food in the cupboard, and a say in whether anyone pets him.

In this lesson, you will make Henri and furnish his corner of the world. Opening his food cupboard
will attract his attention. Feeding him will improve his mood. His couch will have room for
visitors, although he will have opinions about them.

This continues [Let's make a pet rock](programming-with-rules-and-reactions.md). It shares that
lesson's inspiration, [Yib's Pet Rock tutorial](https://lisdude.com/moo/yib_rock.html). You do not
need to bring your rock along. The examples here create a separate set of objects.

## Before you begin

You need builder and programmer privileges, as in the rock lesson. You do not need wizard
privileges. Choose a room where you can leave a few objects and experiment.

Keep the example names while you follow the lesson. Commands such as `@reaction Henri` use those
names to find your objects. Another cat named Henri in the same room will make that confusing.

As before, `%` marks a command to type. `=` marks the response, and `~~` matches part of a response.
Leave those markers out. A command beginning with `;` runs a short piece of MOO code.

For a longer program, enter the named `@program` command. Paste the code, then finish with a period
on a line by itself. The collapsible test sections are for the project's test runner. You do not
enter those instructions.

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

We'll use `#` to give him his pronouns. Think of it as a builder's Swiss army knife for finding
objects and doing things with them. It can inspect properties, assign values, and call verbs. Type
`help #` inside the world for the reference, or follow the
[object shortcuts tutorial](object-shortcuts.md) for a guided tour.

Give Henri his pronouns:

```moot
% #Henri.pronouns = $pronouns:lookup("he/him")
~~.pronouns =
% #Henri for %#:pronouns_display()
==> "he/him"
```

`#Henri` finds your cat by name. `.pronouns` selects his pronoun property, and `=` assigns a value
to it. `$pronouns:lookup("he/him")` supplies that value.

The second command calls a method on the cat. After `for`, `%#` stands for the object you found.
Here, `pronouns_display()` returns the short label `"he/him"`. You can also read a property with
`#Henri.name`, or follow several properties with `#Henri.location.name`. Type `help #` for more
examples. Assignments and `for` require programmer privileges.

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

Add the rule and message properties:

```moot
% @property Henri.pet_rule 0 rc
~~Added
% @property Henri.pet_msg {} rc
~~Added
% @property Henri.pet_denied_msg {} rc
~~Added
% @property Henri.pet_changed_msg {} rc
~~Added
% @property Henri.pet_out_of_reach_msg {} rc
~~Added
% @verb Henri:message_bindings this none this rxd
~~added
```

A rule of `0` permits the operation. We will give Henri a reason to refuse later.

As with the rock, the message editor needs to know who each message can mention. Enter
`@program Henri:message_bindings`, then this code:

```moo verb=Henri:message_bindings
"Describe the participants in Henri's petting messages.";
const {property} = args;
if (!property:starts_with("pet_") || !property:ends_with("_msg"))
  return pass(@args);
endif
let participants = $sub_utils:standard_bindings("The person trying to pet the cat.");
participants['dobj] = ['kind -> 'object, 'description -> "The cat.", 'example -> 'host];
return participants;
```

<details>
<summary>Test check: the message declaration compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

Write the messages:

```moot
% @set-message Henri.pet_msg {nc} {pet|pets} {d}.
~~Set message template
% @set-message Henri.pet_denied_msg {dc} hisses and keeps his distance.
~~Set message template
% @set-message Henri.pet_changed_msg Something changed. Try petting {d} again.
~~Set message template
% @set-message Henri.pet_out_of_reach_msg Move closer to {d} first.
~~Set message template
% @verb Henri:pet this none none rd
~~added
```

`{nc}` names the person, and `{d}` names the cat. `{dc}` starts the cat's name with a capital
letter. The first message says “You pet Henri.” to the person and “Ada pets Henri.” to a visitor.

Enter `@program Henri:pet`, then this code:

```moo verb=Henri:pet
"Pet a nearby cat if his current rule permits it.";
caller == player || raise(E_PERM, "Use the pet command to pet this cat.");
this.owner == task_perms()[1]
  || raise(E_PERM, "This cat and its pet verb need the same owner. Ask the owner to check the verb.");
const who = player;
const room = who.location;
if (!valid(room) || !isa(room, $room) || this.location != room)
  who:inform_current($event:mk_error(who, @this.pet_out_of_reach_msg):with_dobj(this));
  return false;
endif
const context = ['Actor -> who, 'Location -> room];
this:fire_trigger('on_pet_attempt, context);
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

Henri needs to remember his mood and the number of attempts. Declare those two state values:

```moot
% @state Henri mood choice grouchy sleepy playful
~~Declared mood
% @state Henri pet_attempts number 0
~~Declared pet_attempts
% @rule Henri pet NOT fact property_is object=this field=mood value=grouchy
~~Set
```

The first choice, `grouchy`, is the initial mood. The rule permits a pet only when Henri is not
grouchy. `property_is` reads the named property; `NOT` reverses that condition.

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

An `emote` step supplies the object's name. The quoted text is a message template, so it can use
pronouns as well as plain words. The message stays outside the pet command's code.

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
% @create $thing named brasskey
~~Created
% @describe brasskey as A small brass key. It fits the food cupboard.
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
~~You put breakfast in cupboard.
% put lunch in cupboard
~~You put lunch in cupboard.
% put supper in cupboard
~~You put supper in cupboard.
% drop cupboard
~~You dropped cupboard.
% @rule cupboard get never
~~Set
% @rule cupboard lock key is brasskey
~~Set
% @rule cupboard unlock key is brasskey
~~Set
% @set-message cupboard.open_msg {nc} {open|opens} {d}, releasing a familiar smell of cat food.
~~Set message template
% close cupboard
~~You close cupboard.
% lock cupboard with brasskey
~~You lock cupboard with brasskey.
% open cupboard
~~Cupboard is locked.
```

`kibble` is now your own prototype. Its children share its description and inherited behaviour. They
remain separate objects. Henri will recognize descendants of this prototype as food, whatever you
name them.

The `get never` rule stops visitors from picking up the furniture. It does not prevent them from
opening it. The lock and unlock rules select your particular key. Another object named `brasskey`
will not open this cupboard.

## 5. Dinner improves his mood

Use the ordinary `give` command to feed Henri: `give supper to Henri`. That command already checks
whether the giver carries the item and stands near the recipient. It also respects the item's drop
rule.

First, tell Henri which kind of food he accepts:

```moot
% @property Henri.food_kind #-1 rc
~~Added
% #Henri.food_kind = $match:match_object("kibble", player)
~~.food_kind =
% #Henri.food_kind.name
==> "kibble"
% @verb Henri:acceptable this none this rxd
~~added
```

`$match:match_object("kibble", player)` finds your kibble prototype. `food_kind` stores the object
reference, so you can rename the prototype later without breaking the connection. The next command
follows that reference and reads the food's name.

Before moving an item, `give` asks the recipient's `acceptable` method whether it can accept it.
Enter `@program Henri:acceptable` and give that method a more particular opinion:

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

This replaces the actor's default acceptance method on Henri alone. `isa` asks whether the item is
the prototype itself or one of its descendants.

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
% give brasskey to Henri
~~That recipient can't accept the item.
% unlock cupboard with brasskey
~~You unlock cupboard with brasskey.
% open cupboard
~~You open cupboard, releasing a familiar smell of cat food.
% take supper from cupboard
~~You take supper from cupboard.
% give supper to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give supper to Henri.
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
; const cat = $match:match_object("Henri", player); return {$match:match_object("brasskey", player).location == player, cat.pet_attempts, cat.mood};
{true, 1, "sleepy"}
@attach dinner_visitor nonprogrammer
@dinner_visitor
% take lunch from cupboard
~~You take lunch from cupboard.
% give lunch to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give lunch to Henri.
@programmer
@text Bert takes lunch from cupboard.
@text Henri eats the kibble and curls up, looking sleepy.
@text Bert gives lunch to Henri.
; const cat = $match:match_object("Henri", player); return {length(cat.contents), cat.pet_attempts};
{2, 0}
```

</details>

## 6. Let the cupboard tell him

Henri currently notices food only when it reaches him. A real cat usually notices much sooner. Let
the cupboard tell him when someone opens it.

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

The cupboard needs a reaction that sends this event to Henri. The MOO interface can express that
step. Enter this whole line as one command:

```moot
% #cupboard for $reaction:install(%#, "notify_cat_reaction", $reaction:mk('on_open, 0, {{'trigger, $match:match_object("Henri", player), 'on_cupboard_open}})).enabled
==> true
```

Here is what the line does:

- `#cupboard` finds the cupboard; `%#` stands for it in the expression.
- `$match:match_object("Henri", player)` finds the cat.
- `$reaction:mk` makes a reaction for `on_open`. The `0` means it has no extra condition.
- The `trigger` step sends `on_cupboard_open` to Henri. The original participants and room travel
  with it.
- `$reaction:install` saves and enables the reaction on the cupboard you own.

You own both ends of this connection. Sending a trigger can change its recipient's state, so the
sender needs the recipient owner's permissions. Merely knowing another cat's object number does not
give your cupboard control over it.

Try the connection:

```moot
% close cupboard
~~You close cupboard.
% open cupboard
=Henri swivels his ears toward the cupboard.
=You open cupboard, releasing a familiar smell of cat food.
```

You can inspect this reaction with `@reaction cupboard notify_cat show`, even though you installed
it with MOO code. It is the same kind of reaction as the ones made with commands.

<details>
<summary>Test check: a cupboard in another room does not make Henri speak here</summary>

```moot
; const cat = $match:match_object("Henri", player); add_property(player, "tutorial_cat", cat, {player, ""}); move(cat, #90110); return true;
true
% close cupboard
~~You close cupboard.
% open cupboard
=You open cupboard, releasing a familiar smell of cat food.
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
~~You dropped couch.
% @rule couch get never
~~Set
```

The `$sittable` prototype supplies sitting and standing. Give Henri a seat through the same action
interface that reactions use:

```moot
; const couch = $match:match_object("couch", player); const cat = $match:match_object("Henri", player); return $reaction:perform_action('sit, ['target -> couch, 'actor -> cat])['status];
'applied
```

<details>
<summary>Test check: Henri really took a seat</summary>

```moot
@text Henri sits on the couch.
; const couch = $match:match_object("couch", player); return $match:match_object("Henri", player) in couch.sitting;
1
```

</details>

`target` names the seat and `actor` names the sitter. You can direct Henri because you own him. This
permission does not let you make another player's character sit down.

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

```moot
; const couch = $match:match_object("couch", player); const cat = $match:match_object("Henri", player); const condition = "sitting(actor: " + toliteral(cat) + ", seat: input.This) AND input.Actor != " + toliteral(cat); return $reaction:install(couch, "company_reaction", $reaction:mk('on_sit, condition, {{'trigger, cat, 'on_couch_intruder}})).enabled;
true
% sit on couch
=You sit on the couch.
=Henri shifts pointedly away from the new arrival. This is HIS couch.
% stand from couch
=You stand up from the couch.
```

The condition uses the MOO rule expression syntax. `input.This` is the couch receiving the sitting
event. `input.Actor` is the new sitter. `toliteral(cat)` writes the cat's object reference into that
expression. The shorter builder conditions use `this` and `actor` for the same inputs.

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

The cupboard can send this notification whenever someone takes something from it. Henri expects food
from this cupboard. He can be disappointed if you keep something else there.

```moot
% #cupboard for $reaction:install(%#, "dinner_bell_reaction", $reaction:mk('on_take, 0, {{'trigger, $match:match_object("Henri", player), 'on_food_taken}})).enabled
==> true
% take breakfast from cupboard
=Henri stands up from the couch.
=Henri perks up, suddenly interested in dinner.
=You take breakfast from cupboard.
% give breakfast to Henri
=Henri eats the kibble and curls up, looking sleepy.
=You give breakfast to Henri.
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
% @property Henri.idle_schedule 0 rc
~~Added
% @verb Henri:idle this none this rxd
~~added
```

Enter `@program Henri:idle`, then this code:

```moo verb=Henri:idle
"Send one idle event and choose the delay until the next one.";
caller_perms() == this.owner
  || raise(E_PERM, "Only the cat's owner can run its idle behaviour.");
if (valid(this.location) && isa(this.location, $room))
  this:fire_trigger('on_idle, ['Actor -> this, 'Location -> this.location]);
endif
return 240 + random(120);
```

<details>
<summary>Test check: the idle callback compiled</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

The scheduler will call this method for you. Its result chooses the next delay: more than four
minutes and at most six minutes. The method only sends the event. The reaction contains the prose,
so you can change Henri's idle response without rewriting the schedule.

Start it with this command:

```moot
; const cat = $match:match_object("Henri", player); if (!schedule_valid(cat.idle_schedule)) cat.idle_schedule = schedule_every(cat, "idle", 300.0, {}, ['adaptive -> true, 'pass_elapsed -> false]); endif return schedule_valid(cat.idle_schedule);
true
```

`300.0` sets the first wait to five minutes. `adaptive` tells the scheduler to use the method's
return value for later waits. `pass_elapsed` is false because our method takes no arguments. The
`schedule_valid` check prevents a second copy when you repeat the start command.

<details>
<summary>Test check: repeat the start without creating another schedule</summary>

```moot
; const cat = $match:match_object("Henri", player); const first = cat.idle_schedule; if (!schedule_valid(cat.idle_schedule)) cat.idle_schedule = schedule_every(cat, "idle", 300.0, {}, ['adaptive -> true, 'pass_elapsed -> false]); endif const info = schedule_info(cat.idle_schedule); return {cat.idle_schedule == first, info["authority"] == player, info["target"] == cat, info["adaptive"], info["pass_elapsed"]};
{true, true, true, true, false}
```

</details>

`@schedules` lists your schedules. Keep the scene quiet while you finish the lesson by stopping
Henri's schedule:

```moot
; const cat = $match:match_object("Henri", player); schedule_stop(cat.idle_schedule); cat.idle_schedule = 0; return true;
true
```

You can use the start command again whenever you want. The other reactions keep working while the
idle schedule is stopped.

<details>
<summary>Test checks: the actual callback, stale IDs, and a foreign caller</summary>

The test uses a short first wait, then checks that the ordinary programmer's callback really runs.
The callback itself still chooses its usual four-to-six-minute delay.

```moot
; const cat = $match:match_object("Henri", player); cat.idle_schedule = 999999; if (!schedule_valid(cat.idle_schedule)) cat.idle_schedule = schedule_every(cat, "idle", 0.1, {}, ['adaptive -> true, 'pass_elapsed -> false]); endif return true;
true
@text-within 5 Henri grooms one paw, then inspects the room with renewed disappointment.
; const cat = $match:match_object("Henri", player); let info = schedule_info(cat.idle_schedule); for attempt in [1..100] if (info["run_count"] > 0) break; endif suspend(0.01); info = schedule_info(cat.idle_schedule); endfor schedule_stop(cat.idle_schedule); return {info["run_count"] == 1, info["fault_count"] == 0};
{true, true}
; const cat = $match:match_object("Henri", player); return schedule_valid(cat.idle_schedule);
false
@wizard
; #90102.programmer = true; return true;
true
@nonprogrammer
; const cat = $match:match_object("Henri", player); try cat:idle(); return false; except (E_PERM) return true; endtry
true
@wizard
; #90102.programmer = false; return true;
true
@programmer
; const cat = $match:match_object("Henri", player); cat.idle_schedule = 0; return true;
true
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

The [builder's guide](../manual/reactions.md) explains those commands and adds relations and timers.
The [programmer reference](../manual/behavior-model.md) describes the MOO interfaces used here.
