# Objects in the MOO Database

## What are objects?

Everything in a MOO world is made out of objects. Objects are the building blocks that create the virtual reality that
players experience. When you log into a MOO, you become a player object. The room you start in is a room object. The
items you can pick up, the doors you can open, the NPCs you can talk to—they're all objects.

Think of objects as smart containers that can:

- **Hold information** about what they are (through properties)
- **Do things** and respond to commands (through verbs)
- **Contain other objects** (like a backpack holding items)

**Properties** are how objects store information about themselves. A sword object might have properties for its name ("
rusty blade"), its damage (15), and its weight (3 pounds). A player object has properties for their name, score,
location, and inventory.

**Verbs** are what make objects interactive—they're the actions objects can perform or respond to. When you type "look
at sword," you're calling the "look" verb on the sword object. When you "take" something, you're calling the "take"
verb. Verbs are like mini-programs that make objects come alive.

**Object relationships** let objects be organized in hierarchies. A "generic weapon" object might be the parent of all
sword, axe, and bow objects, sharing common weapon behaviors while each type adds its own special features.

## Objects are permanent and persistent

One of the most important things to understand about MOO objects is that they're permanent and "persistent"—they stick
around until someone explicitly destroys them. Each object gets a unique identifier when it's created (like `#123` or
`#048D05-1234567890`), and this identifier becomes that object's permanent identity. Even if you log out, restart the
server, or come back months later, that object will still be there with the same properties and verbs.

This persistence of objects is one of the really unique and powerful aspects of a MOO. It's a world that builds up over
time, and creating objects (and then putting verbs and properties on them) is how builders and programmers help build
the world that every user of a MOO sees and experiences.

> **Different from other languages**: If you're coming from Python, JavaScript, or similar languages, this persistence
> model might feel unusual. In those languages, objects automatically disappear ("get garbage collected") when nothing
> references them anymore. MOO is different: objects stick around forever until you explicitly destroy them, even if no
> variables point to them. This means you need to be more careful about cleaning up objects you no longer need.

## How Objects Get Their IDs

In mooR, every object gets a unique identifier when it's created so it can be distinguished from every other object.
There are two different ID systems the server can use:

**Simple numbered IDs** like `#123` or `#456` work just like house numbers on a street. Each new object gets the next
number in sequence, so it's easy to remember and reference. These are perfect for the core parts of your world—rooms,
important NPCs, generic objects that other things inherit from, and anything you might want to reference directly in
your code.

**UUID identifiers** like `#048D05-1234567890` are a different kind of identifier, and each one is completely random and
unique and basically unguessable. These are designed for things that exist in your world but don't need to be directly
managed by builders—player inventory items, temporary objects created during gameplay, simulation content that comes and
goes.

The advantages of UUID identifiers are:

- You can create lots of them without consuming sequential number slots (avoiding the "used numbers" problem)
- They avoid conflicts between different versions of your world (dev/prod can coexist)
- They're much harder to guess than sequential numbers (providing security through obscurity)

The UUID format might look complicated, but you don't need to understand how it works—just know that
`#048D05-1234567890` is a valid object identifier just like `#123`.

> **Server Configuration**: UUID identifiers are only available if your server administrator has enabled the
`use_uuobjids` feature. All servers support numbered IDs, but UUID identifiers are an optional feature that must be
> explicitly turned on. See the [Server Configuration](../the-system/server-configuration.md) documentation for details
> on feature flags.

## Ways to Reference Objects

Once an object has an ID, there are several ways you can reference it in your code:

**Direct object IDs**: Use the object's actual identifier

```moo
#123:tell("Hello!")              // Numbered ID
#048D05-1234567890:tell("Hello!")  // UUID identifier
```

**System references**: Use readable names that point to important objects

```moo
$player:tell("Hello!")           // Refers to the generic player object
$room:look()                     // Refers to the generic room object
```

**Variables**: Store object references in variables for later use

```moo
let sword = #123;
sword:wield();                   // Same as #123:wield()
```

All of these work exactly the same way—they're just different ways of referring to the same objects.

> **Best Practice - Avoid Hard-Coded Object IDs**: While you can use direct object IDs like `#123` anywhere in your
> code, it's generally discouraged in verbs that other people will use or in code that's meant for general use. If
> object
`#123` gets recycled or renumbered, your code will break. Instead, use system references like `$player` or store object
> references in properties with meaningful names. For more about system references, see
> the [System References section](moo-value-types.md#system-references-names) in the value types documentation.

### The Number Slot Problem (Numbered IDs Only)

When you're working with numbered objects, there's an important consideration: each numbered object "takes up" a number
slot permanently. When you create a new numbered object, the server assigns it the next available number (`#123`, then
`#124`, then `#125`, etc.). Even if you later destroy the object, that number slot remains "taken" and won't be reused
under normal circumstances.

This is intentional—it prevents confusion where old references to a recycled object might accidentally point to a
completely different new object. But it does mean you can eventually run out of numbers if you create and destroy
millions of objects over time.

UUID objects avoid this problem entirely. Each UUID is unique by design, so there's no sequential numbering to exhaust
or manage.

> **Lightweight alternatives: Flyweights**
>
> Because objects are "permanent residents" of your world (they take up database space and require manual cleanup), mooR
> provides **flyweights** as a lightweight alternative for creating lots of small, temporary objects. Flyweights don't
> get object identifiers, don't persist in the database, and automatically disappear when no longer needed—perfect for
> things like inventory items, temporary game pieces, or UI elements.
>
> For more details, see the [Flyweights section](moo-value-types.md#flyweights---lightweight-objects) in the value types
> documentation.

## How Objects Actually Work

Unlike numbers or strings, objects don't just exist by writing them down. If you type `#958` in your code, that doesn't
automatically create object `#958`—it just references it, and it might not even exist yet! The object has to be
explicitly created first using the `create()` function, and once created, it exists until someone explicitly destroys it
with `recycle()`.

This is different from most programming languages where you can create objects on the fly. In MOO, object creation is a
bigger deal because each object is a permanent part of the world that other players will interact with.

Objects are made up of several components that work together:

## What Every Object Has

Every object has some built-in characteristics that make it work:

**Player status**: Some objects represent players (actual people logged into the MOO), while others represent things in
the world. Only wizards can change whether an object is a player object or not.

**Parent-child relationships**: Most objects have one parent object that they inherit behavior from. Objects can also
have multiple children that inherit from them. This creates family trees of objects where common behaviors are shared.

In practice, most objects in a MOO world trace their ancestry back to a "root" object (usually `$root` or `#1`) that
contains many of the fundamental verbs and properties that make objects behave nicely in the MOO world—things like basic
movement, communication, and interaction behaviors.

For example, there might be a "generic room" object that defines basic room behavior like `look` and `go`. All other
rooms in the MOO would be children of this generic room, inheriting those basic behaviors while adding their own unique
features. And the generic room itself would typically inherit from the root object.

Objects without parents are possible but won't behave as nicely in the world since they miss out on this foundational
behavior—they're typically used for system or utility objects that players don't interact with directly.

For complete details about how inheritance works, how to change parent relationships, and the rules that govern them,
see the [Object Parents and Children](object-parents-and-children.md) chapter.

**Built-in properties**: The system automatically gives objects certain standard properties they need to function, like
their `name` and `location`. Parent relationships are accessed using the `parent()` and `children()` built-in functions
rather than as properties.

## Objects have properties and verbs

Objects are made up of two main kinds of content that define their behavior:

**Properties** store information about the object. Think of properties as variables that belong to the object—they hold
data like names, descriptions, scores, damage values, or any other characteristics you want to track. You can read and
modify properties using dot notation like `object.property_name`.

**Verbs** are programs that define what the object can do and how it responds to commands. When a player types "look at
sword" or "take apple," they're calling verbs on those objects. Verbs are like mini-programs that bring objects to life
with interactive behaviors.

Both properties and verbs can be inherited from parent objects and customized by child objects, making it easy to create
families of related objects that share common characteristics while having their own unique features.

For detailed information about how properties and verbs work, see:

- [Object Properties](./object-properties.md) - How objects store and manage data
- [Object Verbs](./object-verbs.md) - How objects implement behaviors and commands
