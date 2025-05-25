# Object Movement & Contents

Every object in a MOO database has a location, which is another object. The location of an object is typically a room,
but it can also be another object that is not a room (e.g. a player's inventory, a container, etc.). The location of an
object is represented by the `location` property of the object.

The builtin-property `contents` is the inverse relationship of `location`. It is a list of objects that are contained
in the object. For example, if an object is a room, then its `contents` property will contain all of the objects that
are in that room. The `contents` property and the `location` property are automatically updated by the server whenever
an
object is moved, which is done by the `move()` function.

The `move()` function is used to move an object from one location to another. It takes two arguments: the object to be
moved and the destination object. The destination can be any object that is a valid location for the object being moved.

During evaluation of a call to the `move()` function, the server can make calls on the `accept` and `enterfunc` verbs
defined on the destination of the move and on the `exitfunc` verb defined on the source. The rules and circumstances are
somewhat complicated and are given in detail in the description of the `move()` function.
