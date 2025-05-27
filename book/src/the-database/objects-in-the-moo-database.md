# Objects in the MOO Database

## What are objects?

Everything in a MOO world is made out of objects. Objects are the building blocks that create the virtual reality that players experience. When you log into a MOO, you become a player object. The room you start in is a room object. The items you can pick up, the doors you can open, the NPCs you can talk to—they're all objects.

Think of objects as smart containers that can:
- **Hold information** about what they are (through properties)
- **Do things** and respond to commands (through verbs)
- **Contain other objects** (like a backpack holding items)

**Properties** are how objects store information about themselves. A sword object might have properties for its name ("rusty blade"), its damage (15), and its weight (3 pounds). A player object has properties for their name, score, location, and inventory.

**Verbs** are what make objects interactive—they're the actions objects can perform or respond to. When you type "look at sword," you're calling the "look" verb on the sword object. When you "take" something, you're calling the "take" verb. Verbs are like mini-programs that make objects come alive.

**Object relationships** let objects be organized in hierarchies. A "generic weapon" object might be the parent of all sword, axe, and bow objects, sharing common weapon behaviors while each type adds its own special features.

## Technical details

Objects encapsulate state and behavior – as they do in other object-oriented programming languages. Objects
are also used to represent objects in the virtual reality, like people, rooms, exits, and other concrete things. Because
of this, MOO makes a bigger deal out of creating objects than it does for other kinds of values, like integers.

Numbers always exist, in a sense; you have only to write them down in order to operate on them. With permanent objects,
it is different. The permanent object with number `#958` does not exist just because you write down its number. An
explicit operation, the `create()` function described later, is required to bring a permanent object into existence.
Once created, permanent objects continue to exist until they are explicitly destroyed by the `recycle()` function (also
described later).

The identifying number associated with a permanent object is unique to that object. It was assigned when the object was
created and will never be reused unless `recreate()` or `reset_max_object()` are called. Thus, if we create an object
and it is assigned the number `#1076`, the next object to be created using `create()` will be assigned `#1077`, even if
`#1076` is destroyed in the meantime.

Objects are made up of three kinds of pieces that together define its behavior: _attributes_,
_properties_, and _verbs_.

## Fundamental Object Attributes

There are three fundamental _attributes_ to every object:

1. A flag representing the built-in properties allotted to the object.
2. A list of objects that are its parents
3. A list of the objects that are its _children_; that is, those objects for which this object is their parent.

The act of creating a character sets the player attribute of an object and only a wizard (using the function
`set_player_flag()`) can change that setting. Only characters have the player bit set to 1. Only permanent objects can
be players.

The parent/child hierarchy is used for classifying objects into general classes and then sharing behavior among all
members of that class. For example, the LambdaCore database contains an object representing a sort of "generic" room.
All
other rooms are _descendants_ (i.e., children or children's children, or ...) of that one. The generic room defines
those pieces of behavior that are common to all rooms; other rooms specialize that behavior for their own purposes. The
notion of classes and specialization is the very essence of what is meant by _object-oriented_ programming.

Only the functions `create()`, `recycle()`, `chparent()`, `chparents()`, `renumber()` and `recreate()` can change the
parent and children attributes.

## Objects have properties and verbs

Objects are made up of two main kinds of content that define their behavior:

**Properties** store information about the object. Think of properties as variables that belong to the object—they hold data like names, descriptions, scores, damage values, or any other characteristics you want to track. You can read and modify properties using dot notation like `object.property_name`.

**Verbs** are programs that define what the object can do and how it responds to commands. When a player types "look at sword" or "take apple," they're calling verbs on those objects. Verbs are like mini-programs that bring objects to life with interactive behaviors.

Both properties and verbs can be inherited from parent objects and customized by child objects, making it easy to create families of related objects that share common characteristics while having their own unique features.

For detailed information about how properties and verbs work, see:
- [Object Properties](./object-properties.md) - How objects store and manage data
- [Object Verbs](./object-verbs.md) - How objects implement behaviors and commands
