# Build rooms and connect them

A room is a place people can visit. A passage connects two rooms. Each end has its own name and
description, while both ends share the door's open and locked state. An area groups rooms and
controls who can build there. A passage can cross an area boundary.

Start with a tour: from the First Room, type `examples`. Each example room explains what to try. The
gallery has ordinary exits, stairs, a one-way chute, a keyed door, and portals. Type `out` from the
gallery to return. The workshop also has an unlocked route through the courtyard.

This lesson creates a small workshop. You need builder privileges and permission to build in your
current area. You do not need programmer privileges. Use an empty room you own as the entrance.

A line starting with `%` is a command to type. Leave out the `%`. A line starting with `=` is the
expected response. `~~` shows part of a response whose object number varies between worlds.

<details>
<summary>Test setup</summary>

The session runner supplies an entrance and area owned by the builder. The builder's programmer flag
is off. These setup commands are not part of the lesson.

```moot
@wizard
; move(#90103, #90108); #90103:set_owner(#90101); #90108:set_owner(#90101); #90101.is_builder = true; #90101.programmer = false; #90103.name = "Workshop Entrance"; return true;
true
@programmer
```

</details>

## Connect two rooms

Create a room called Workbench:

```moot
% @build Workbench
~~Created "Workbench"
```

Connect your entrance to it:

```moot
% @dig east to Workbench
~~Dug passage:
```

`east` is the name at this end. Cowbell supplies `west` at the other end. For names without an
opposite direction, supply both names: `@dig stairs|landing to Workbench`.

Give visitors something to see at the entrance:

```moot
% @describe --literal east as A brass door stands beside the tool rack.
=Set ambient description for 'east' passage.
% @set-passage east departure through the brass door
=Set east departure phrase.
% @set-passage east arrival from the workshop entrance
=Set east arrival phrase.
```

The first sentence becomes part of the room description. Meadow links that sentence to the exit. The
departure and arrival phrases describe movement. They do not replace the room description. Use
`@describe --djot` for inline formatting, or `--template` for substitutions such as `{direction}`.
Literal text can contain braces without becoming a template.

An exit with descriptive prose does not also appear in the room's extra travel sentence. Other open
routes appear as “You can go north or to the workshop.” An authored departure phrase takes
precedence. For other names, such as `examples`, the sentence uses the destination's name. The links
still run the exit's command. If a custom exit withholds its destination, the sentence gives its
command name.

The room panel and `exits` command still list every route and its current state. Closed and locked
doors show their state beside the prose. Unavailable routes show the reason there.

`@passage east` shows the configuration and previews the prose and movement messages.
`@edit-description east` opens the description editor. `exits` lists ways out, including closed
doors.

## Add a door and key

The connection starts open. Turn it into a door, then create its key:

```moot
% @set-passage east is_door true
=Set east is_door to true.
% @create $thing named workshopkey
~~Created
% @set-passage east access key is workshopkey
=Set east access condition.
% lock east
=You lock the door "east".
```

The access condition compares the supplied key with `workshopkey`. The door looks through your
inventory for a matching key. A named key must also be in your inventory.

A condition does not close or lock an open door. `lock east` does both. Anyone can use an open,
unlocked passage. Locks control this connection, rather than every possible way into the room.

Try opening it with your key:

```moot
% open east with workshopkey
=You unlock and open the door "east".
```

You can also use conditions such as `actor is me`, `always`, or a relation condition from the
[reaction guide](reactions.md). `me` means the person who wrote the condition. Passage conditions
can refer to `actor`, `key`, `from`, and `to`.

Both sides share the access condition. Changing it requires building authority at both ends.
`@set-passage east access clear` removes the condition and unlocks the passage.

To change the door's wording, use `open_msg`, `close_msg`, `lock_msg`, or `unlock_msg`:

```moot
% @set-passage east close_msg The brass door settles into its frame.
=Set east close_msg.
% close east
=The brass door settles into its frame.
% @set-passage east close_msg clear
=Set east close_msg.
% open east
=You open the door "east".
```

These are message templates. They can name the actor, `{direction}`, and `{passage}`. An invalid
binding leaves the old message intact. `clear` restores the standard wording. Use a room reaction
for an additional response, such as a bell: see
[door reactions](reactions.md#respond-when-someone-opens-a-passage-door).

## Walk both ways

```moot
% east
```

You are now in Workbench. The same door is called `west` here. Give this side its own description:

```moot
% @describe --literal west as The brass door leads back to the entrance.
% exits
% west
```

Back at the entrance, `@passage east` shows the shared state and this side's text. Editing the east
side's description does not replace the west side's description.

<details>
<summary>Test checks: travel, descriptions, and the access condition</summary>

```moot
@wizard
; const p = $room:find_passage(#90103, "east"); return {#90101.location == #90103, p.side_a_description, p.side_b_description, p.is_open, typeof(p.unlock_rule) == TYPE_FLYWEIGHT};
{true, "A brass door stands beside the tool rack.", "The brass door leads back to the entrance.", true, true}
; const view = #90103:look_self():capture(#90101); const text = $document:render(view['description], 'text_plain)['body]; return index(text, "A brass door stands beside the tool rack.") > 0;
true
@programmer
```

</details>

## Add a second route and a one-way exit

Two passages can connect the same rooms. Give them different names:

```moot
% @dig --allow-parallel stairs|down to Workbench
% stairs
% down
% @build Chute Landing
% @dig oneway chute to Chute Landing
% chute
```

The chute has no return side. Give visitors another way back. From Chute Landing, create a ramp:

```moot
% @dig --allow-parallel ramp|alcove to Workshop Entrance
% ramp
```

Closing the east door does not close the stairs. These are separate connections with separate state.

<details>
<summary>Test checks: independent connections and a usable return route</summary>

```moot
@wizard
; return {#90101.location == #90103, length($room:passages(#90103)), $room:find_passage(#90103, "east").connection_id != $room:find_passage(#90103, "stairs").connection_id};
{true, 4, true}
```

</details>

## Share building work

The owner grants specific operations with `@grant`. For example, these commands let Ada connect
rooms in your area and edit one room's exits. Replace the object numbers and player name:

```text example
@grant #100.area(add_room,create_passage) to Ada
@grant #101.room(dig_from,dig_into) to Ada
```

`add_room` permits new rooms in the area. `create_passage` permits storing connections there.
`dig_from` permits work on the source end. `dig_into` permits a connection to arrive at the room. A
bidirectional passage needs both endpoint permissions at both rooms. A one-way passage needs
`dig_from` at its source and `dig_into` at its destination.

The command reports missing grants when it cannot connect the rooms. A grant on an area does not
grant control of every room in it. Shared door changes also need authority at the other end.

## Connect another area

Use the destination room's object number when it is outside your current area:

```text example
@dig north|south to #123
```

The room owner at the far end must grant the same permissions as for a local connection. Both rooms
show the passage. Visitors can go north and return south. `walk`, `join`, and `home` can follow
ordinary passages across area boundaries.

The default courtyard and hillside overlook demonstrate this with different area owners. The blue
portal crosses the same boundary through custom code. [Custom exits](custom-exits.md) explains that
programmer interface.

## Move or remove a connection

`@undig east` removes that connection at both ends. It preserves a parallel passage such as the
stairs. Use a direction to select the connection when several lead to the same room.

To move a room into another area, use `@move #ROOM to #AREA`. You must own the room and have
`add_room` permission in the destination area. Its existing passages keep their identities and
remain visible from both ends. Moving a room does not grant control of a neighbour's side.

Inspect `@passage DIRECTION` before changing or removing a connection. Tell the other room's author
before you remove a shared route. Recycling a room also removes its connections.

## Names in room descriptions

Rooms and areas inherit complete-name grammar, so titles such as `The First Room` remain unchanged.
Objects choose their own listing phrases. Use `@grammar box listing definite` for “the box,” or
`@grammar boots number plural` for “some boots.” A collective actor also takes plural agreement. See
the [naming lesson](../docs/names-and-grammar.md) for examples.

Passage labels are travel choices, not object nouns. Default operation messages identify a door as
`the door "east"`. Custom passage messages can say “the brass door” when that fits the scene.
