# The mooR Object Database ("WorldState")

## What makes MOO unique

MOO (MUD Object Oriented) is fundamentally different from most programming environments. Instead of writing programs
that manipulate external data, **MOO programs live inside a persistent object database that represents a virtual world**.
Every room, character, item, and even the programs themselves are objects stored in this database.

What makes this special:

- **Everything is persistent** - When you create a sword object or write a verb, it stays in the world permanently until
  explicitly removed
- **Everything is programmable** - Every object can have custom behaviors through verbs (programs attached to objects)
- **Everything is interconnected** - Objects can contain other objects, inherit from parents, and interact through a
  shared spatial environment
- **Everything is live** - Changes take effect immediately while players are connected and interacting

This creates a unique programming environment where you're not just writing code—you're building and inhabiting a
living, persistent virtual world.

## What you'll learn in this chapter

This chapter covers the four fundamental building blocks that let you create MOO worlds:

- **Objects** - The entities that make up your world (rooms, players, items, etc.)
- **Properties** - How objects store information and state
- **Verbs** - The programs that give objects their behaviors and responses
- **Values** - The different types of data MOO can work with

The database ("WorldState") is where all of this lives. Everything that players see and interact with
in a MOO is represented in the database, including characters, objects, rooms, and the MOO programs (verbs) that give
them their specific behaviours. If the server is restarted, it will start right back up with the same data that was
present
when it was last running, allowing players to continue their interactions seamlessly.

**The database *is* the MOO.**

## How the database is organized

The database is a collection of **objects**, each of which has:

- **Properties** - values that store information about the object (like a name, description, or hit points)
- **Verbs** - programs that define what the object can do and how it responds to commands

Objects are organized into a **hierarchy** with parent-child relationships. A parent object acts as a template for its
children, providing default properties and verbs that children can inherit and customize. This inheritance system allows
you to create families of similar objects efficiently—for example, all weapons might inherit from a basic "weapon"
parent, but each specific sword or axe can have its own unique properties and behaviors.

This chapter breaks down each of these building blocks in detail, showing you how to use objects, properties, verbs, and
values to create rich, interactive MOO worlds.

> **Note on "Core" vs "Database"**: The "db" is generally broken down, mentally, into the "core db" that a given MOO was
> started with, and then all the user-created content that came later. The core provides the fundamental systems and
> objects needed for a MOO to function, while user content builds upon that foundation.
