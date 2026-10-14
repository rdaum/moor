# Program a portal that appears among the exits

A passage works well for two connected rooms. A lift, ferry, or portal can need its own code. It
still needs to tell visitors what they can do. Otherwise a room can contain a working portal that
nobody discovers.

We will build a blue arch that takes visitors to another room. A visitor will see it in the room
description and in `exits`, then type `enter portal` to use it. Later, we will close it for travel
while keeping it visible.

This lesson follows the programming ideas in [Let's make a pet rock](pet-rock.md). It assumes you
have met properties, arguments, return values, lists, and maps there. We will use them to answer two
questions for the room: “What travel can you offer?” and “Where does this choice lead now?” Each
question gets a method on the portal. The room will handle the actual movement.

You need builder and programmer privileges. Start in a room you own, inside an area where you can
build. The portal's owner does not need wizard privileges. `%` marks commands and `=` marks output.
Enter each named `@program` command before its code. Finish each editor session with `.`.

<details>
<summary>Test setup</summary>

```moot
@wizard
; move(#90103, #90108); #90103:set_owner(#90101); #90108:set_owner(#90101); #90101.is_builder = true; #90101.authoring_features = $prog_features; return true;
true
@programmer
```

</details>

## Make the object and its destination

```moot
% @rename here to Portal Workshop
~~Renamed
% @build Portal Landing
~~Created
% @create $thing named portal
~~Created
% @state portal source_room object here
~~Declared source_room
% @state portal destination_room object "Portal Landing"
~~Declared destination_room
% @state portal in_service flag yes
~~Declared in_service
```

The first setting remembers the room you are in. The second remembers Portal Landing, which the
command finds in your current area. `in_service` controls whether visitors can use the portal.

## Describe the option

First, the room needs a description of the available trip. The method `travel_options` supplies that
description as data. It returns a list of options, and each option is a map with named fields. Our
portal has one option, so the list will contain one map.

This method does not send text to the visitor itself. The room uses the returned values to make its
description, exit list, clickable links, and route hints. Keeping those values together lets all
those views describe the same trip.

Add the method:

```moot
% @verb portal:travel_options this none this rxd
~~added
```

The `x` in `rxd` lets other code call this verb as a method. This differs from the rock's `pet`
command, which people call by typing it.

The room supplies two arguments. `source` is the room whose exits it is describing, and `viewer` is
the person asking. `const {source, viewer} = args;` gives those arguments names. If the source is
not our saved `source_room`, the method returns `{}`, an empty list: no options here.

The returned map describes the trip with these fields:

| Field         | Value in this portal             | Purpose                                                    |
| ------------- | -------------------------------- | ---------------------------------------------------------- |
| `id`          | `"cross"`                        | Identifies this option when a visitor chooses it.          |
| `label`       | `"portal"`                       | Gives the exit list a short name to show.                  |
| `command`     | `"enter portal"`                 | Says what the visitor can type.                            |
| `destination` | `this.destination_room`          | Supplies the destination object saved by `@state`.         |
| `available`   | `this.in_service`                | Says whether the trip is currently offered.                |
| `reason`      | `"The portal is resting."`       | Explains an unavailable trip.                              |
| `description` | The sentence about the blue arch | Describes the exit in the room.                            |
| `ambient`     | `true`                           | Includes that sentence among the room's descriptive prose. |

The labels on the left of `->` are symbols; the values on the right can be text, objects, or
booleans. The outer braces make the list. The square brackets inside them make its one map.

Enter `@program portal:travel_options`, then this body:

```moo verb=portal:travel_options
"Describe this portal from its approved source room. Do not move the viewer.";
const {source, viewer} = args;
if (source != this.source_room)
  return {};
endif
return {['id -> "cross", 'label -> "portal", 'command -> "enter portal",
  'destination -> this.destination_room, 'available -> this.in_service,
  'reason -> "The portal is resting.",
  'description -> "A blue arch shimmers beside the wall.", 'ambient -> true]};
```

<details>
<summary>Test check: compilation</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

`id` stays the same when the label or state changes. `command` is what visitors type. `ambient` puts
the description into room prose, with a link to the same action used by `exits`. The normal sentence
mode preserves your wording. `description_kind` can select `literal`, `djot_inline`, or `template`.
The default is literal text.

The method runs during discovery, sometimes for a route through another room. It must not move
anyone, charge a fare, or alter travel state. `viewer` tells you who will read the result. It does
not grant control of that person. `caller_perms()` identifies whose permissions the caller uses.
Your method executes with its own verb owner's permissions, as usual.

## Handle the chosen action

When a visitor chooses `enter portal`, the room calls `use_exit` on our portal. It supplies the
visitor, the source room, and the selected option's `id`. We name those arguments `who`, `source`,
and `option`.

The method must decide using the current state. Someone can read the exit list while the portal is
open, then try to use it after we close it. The earlier description is not permission to travel
forever.

Our checks ask whether the source is correct, the visitor is still there, the chosen option is
`"cross"`, and the portal is in service. If any check fails, the method returns `false`. Otherwise,
it returns `this.destination_room`. The returned object tells the room where to take the visitor.

Add the method, then enter `@program portal:use_exit`:

```moot
% @verb portal:use_exit this none this rxd
~~added
```

```moo verb=portal:use_exit
"Choose a destination for the room's checked exit action.";
if (caller != $room)
  raise(E_PERM, "Use the portal through the room's exit action.");
endif
const {who, source, option} = args;
if (source != this.source_room || who.location != source || option != "cross" || !this.in_service)
  return false;
endif
return this.destination_room;
```

<details>
<summary>Test check: compilation</summary>

```moot
~~Now programming
~~lines programmed.
```

</details>

The room checks the actor and selected link before calling this method. The caller check keeps
direct calls outside that path from triggering your travel code. You do not need wizard privileges
or permission to move someone else's player.

Return a destination room to request movement, or `false` when this action does not move the actor.
The room then checks the source and registration again and calls native `move()` with the visitor's
authority. It also checks that a known destination still matches the displayed option. The
destination's normal entry checks remain in force.

The provider checks its own current state. A link can remain on screen after that state changes. If
your method suspends or calls code that can suspend, recheck its mutable conditions before
returning. Do not charge a fare or announce arrival here: the destination can still refuse entry.
Use a completed room-entry reaction for effects that require successful movement.

## Approve it as a room exit

The room needs its author's approval before treating the portal as an exit. This stops someone from
adding a new exit merely by dropping an object in your room. An approved object with these two
methods is called an **exit provider**: it provides the room's custom travel options.

You own the source room in this lesson, so approve the portal there:

```moot
% @exit-provider here add portal
~~Custom exit add:
% exits
~~Ways out
=
~~portal
% enter portal
```

You arrive at Portal Landing. The portal's blue-arch sentence and its entry in `exits` offer the
same action. An object does not become a room exit merely because someone drops it there. The portal
can also remain in its author's inventory: registration, rather than its location, supplies
approval.

The room itself can be a provider. This works for commands such as `press up` inside a lift. It uses
the same two methods. Its author approves it with `@exit-provider here add here`.

<details>
<summary>Test checks: travel and normal movement events</summary>

```moot
@wizard
; const portal = $match:match_object("portal", #90101); return {#90101.location == portal.destination_room, portal.source_room != portal.destination_room};
{true, true}
@programmer
@event data:state room_snapshot
```

</details>

## Add a response after arrival

Room entry reactions run after the move. At Portal Landing, add an editable welcome message and an
entry reaction:

```moot
% @reaction here welcome after enter
% @reaction here welcome then tell actor "Blue light fades behind you."
% @reaction here welcome enable
```

This uses the same message templates as other reactions. You can refer to the arriving person as
`{n}` and change the wording without editing the portal method. Native movement supplies the room's
`enter` and `exit` events. Do not send those events again from your portal.

## Unavailable and unknown destinations

An unavailable exit stays visible with its `reason`. The room offers no link and does not use it in
route planning. Change your portal's state and ask the room to refresh its panel:

```moot
% @state portal in_service set no
% @exit-provider "Portal Workshop" refresh portal
```

The portal's own code can make that call after changing state. A registered provider can refresh its
room, and a room author can refresh it too. This is only a notification. The method still checks
access and service state when someone uses it.

For a portal that chooses a destination during use, omit `destination` from its option. It remains
visible and usable. The route planner cannot treat it as a route to a particular room.

Return an empty list to hide all options. Return several maps for a lift with several destinations.
A call button can change the lift state, refresh its options, and return `false`. Its new ride
option can then return a destination. Each needs a distinct `id`, label, and command. `aliases`,
when supplied, is a list of complete alternative commands, such as `{"step through portal"}`.

Route hints include available custom exits with known destinations. Automatic walking follows
ordinary passages only. When a route needs this portal, the hint names the room and says to use
`enter portal`. It does not submit arbitrary commands on the visitor's behalf.

<details>
<summary>Test checks: unavailable discovery and routes</summary>

```moot
@wizard
; const portal = $match:match_object("portal", #90101); const options = $room:exit_options(portal.source_room, #90101); return {options[1]['caption], options[1]['descriptor], portal.source_room:transport_destinations(#90101)};
{"portal (The portal is resting.)", [], {}}
```

</details>

## Remove or replace a provider

Use `@exit-provider ROOM remove portal` to remove approval. `@exit-provider ROOM` lists the
providers and shows which need approval again. Replace ROOM with the room's name or identifier. You
must own the room or have its building permission to change its providers.

Re-registering creates a new approval. Saved links from the previous approval stop working. A change
to either object's ownership also requires approval again. Changing an option's destination rejects
links that still name the old destination. The server checks the actor's current room as well.

A provider error leaves the room's other exits visible. The error entry identifies the provider; the
server log retains the diagnostic. Fix the provider instead of suppressing the error.
