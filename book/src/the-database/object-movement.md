# Object Movement & Contents

## Understanding MOO's spatial world

MOO creates virtual worlds made up of places—rooms, containers, and spaces that players can navigate and explore. Just like in the real world, everything needs to be somewhere. Your character stands in a room, items sit on tables or in backpacks, and doors connect different locations.

This spatial organization is fundamental to how MOO works. When you type "look," you see what's in your current location. When you "drop" something, it appears in the same place you are. When another player enters the room, they can see you because you're both in the same location.

## How location works technically

Every object in a MOO database has a location, which is another object. The location of an object is typically a room,
but it can also be another object that is not a room (e.g. a player's inventory, a container, etc.). The location of an
object is represented by the `location` property of the object.

The builtin-property `contents` is the inverse relationship of `location`. It is a list of objects that are contained
in the object. For example, if an object is a room, then its `contents` property will contain all of the objects that
are in that room.

The `move()` function is used to move an object from one location to another. It takes two arguments: the object to be
moved and the destination object. The destination can be any object that is a valid location for the object being moved.

> **Important:** The `contents` and `location` properties are **system-managed** and automatically updated by the server whenever an object is moved using the `move()` function. You should never try to modify these properties directly—always use `move()` to change an object's location (it won't allow you). The server maintains the consistency between these two properties automatically.

## Smart movement: How objects respond to being moved

One of MOO's powerful features is that objects can react intelligently when something moves into or out of them. This happens through special verbs that get automatically called during movement:

**When something tries to enter a location:**
- The destination object's `:accept` verb runs first - this can reject the move if it doesn't make sense
- If accepted, the destination's `:enterfunc` verb runs - this handles what happens when something arrives

**When something leaves a location:**
- The source location's `:exitfunc` verb runs - this handles what happens when something departs

**Why this matters for builders:**

These verbs let you create smart, responsive environments. For example:
- A locked chest can reject items unless the player has the key (`accept` verb)
- A room can announce when players enter ("Alice walks in from the north") (`enterfunc` verb)
- A magic portal can transport the player somewhere else when they leave (`exitfunc` verb)
- A scale can update its weight display when items are added or removed

This system allows objects to have complex, realistic behaviors without requiring every command to know about every special case. The objects themselves handle their own logic for movement.

## Building with movement verbs: Practical examples

Here are some concrete examples of how builders use movement verbs to create interactive environments:

### Access control with `:accept`

... On a locked treasure chest

```moo
  if (this.locked && object.owner != player)
    player:tell("The chest is locked and won't accept your " + object.name + ".");
    return 0;  // Reject the move
  endif
  return 1;    // Allow the move
```

### Atmospheric messages with `enterfunc` and `exitfunc`

```moo
// On a room with a creaky door
  if (typeof(object) == OBJ && valid(object) && object.player)
    this:announce_all_but(object, object.name + " creaks through the ancient door.");
  endif
```

```moo
  if (typeof(object) == OBJ && valid(object) && object.player)
    this:announce_all_but(object, object.name + " slips quietly into the shadows.");
  endif
```

### Weight and capacity limits with `accept`

... On a backpack with weight limits

```moo
  current_weight = this:calculate_total_weight();
  if (current_weight + object.weight > this.max_capacity)
    player:tell("The " + this.name + " is too full to hold the " + object.name + ".");
    return 0;
  endif
  return 1;
```

### Special transportation with `exitfunc`

On a magical teleporter pad...

```moo
  if (typeof(object) == OBJ && valid(object) && object.player)
    random_destination = this.destinations[random(length(this.destinations))];
    object:tell("The world shimmers and you find yourself elsewhere!");
    move(object, random_destination);
  endif
```

These examples show how movement verbs let you create rich, interactive worlds where objects behave intelligently without requiring complex command parsing or special cases throughout your codebase.

For the detailed rules about when and how these verbs are called, see the documentation for the `move()` function.
