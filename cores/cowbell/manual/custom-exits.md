# Program a portal that appears among the exits

A passage works well for two connected rooms. A lift, ferry, or portal can need its own code. It
still needs to tell visitors what they can do. Otherwise a room can contain a working portal that
nobody discovers.

This lesson builds a portal with two methods. `travel_options(source, viewer)` describes the ways
out. `use_exit(actor, source, option_id)` chooses the destination when someone uses an option. The
room uses that description for room prose, `exits`, Meadow links, and route hints.

You need builder and programmer privileges. Start in a room you own, inside an area where you can
build. The portal's owner does not need wizard privileges. `%` marks commands and `=` marks output.
The code blocks with `verb=...` name the method to program. Finish each editor session with `.`.

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
% @build Portal Landing
~~Created
% @create $thing named portal
~~Created
% @property portal.source_room #-1 rc
~~Added
% @property portal.destination_room #-1 rc
~~Added
% @property portal.in_service true rc
~~Added
```

Set its source to your current room and its destination to Portal Landing:

```moot
; const portal = $match:match_object("portal", player); portal.source_room = player.location; portal.destination_room = $match:resolve_in_scope("Portal Landing", player.location.location.contents); return portal.destination_room.name;
"Portal Landing"
```

## Describe the option

```moot
% @verb portal:travel_options this none this rxd
~~added
```

Program `portal:travel_options`:

```moo verb=portal:travel_options
"Describe this portal from its approved source room. Do not move the viewer.";
const {source, viewer} = args;
source == this.source_room || return {};
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
not grant control of that person. In `caller_perms()`, you can inspect the querying principal. Your
method executes with its own verb owner's permissions, as usual.

## Handle the chosen action

Add and program `portal:use_exit`:

```moot
% @verb portal:use_exit this none this rxd
~~added
```

```moo verb=portal:use_exit
"Choose a destination for the room's checked exit action.";
caller == $room || raise(E_PERM, "Use the portal through the room's exit action.");
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

The room's author registers the portal:

```moot
; const portal = $match:match_object("portal", player); $room:register_exit_provider(portal.source_room, portal); return true;
true
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
the same two methods and registers itself with `$room:register_exit_provider(room, room)`.

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
; const portal = $match:match_object("portal", player); portal.in_service = false; $room:exit_options_changed(portal.source_room, portal); return true;
true
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

Call `$room:unregister_exit_provider(room, provider)` with the room author's authority to remove
approval. Registration requires `dig_from` authority on the room. A capability can be supplied as
the first argument instead of the raw room object.

Re-registering creates a new approval. Saved links from the previous approval stop working. A change
to either object's ownership also requires approval again. Changing an option's destination rejects
links that still name the old destination. The server checks the actor's current room as well.

A provider error leaves the room's other exits visible. The error entry identifies the provider; the
server log retains the diagnostic. Fix the provider instead of suppressing the error.
