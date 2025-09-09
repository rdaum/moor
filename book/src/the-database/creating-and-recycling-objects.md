# Creating and Recycling Objects

## Understanding object persistence

In MOO, objects are permanent and persistent—they stick around until someone explicitly destroys them. This is different
from most programming languages where objects automatically disappear when nothing references them anymore.

For a complete explanation of object persistence and how MOO objects work,
see [Objects in the MOO Database](objects-in-the-moo-database.md).

## Creating objects

When you're building your world, you'll spend a lot of time creating objects. Every room players can visit, every item
they can pick up, every character they can talk to—all of these start life as a call to the `create()` function.

The `create()` function brings a new object into existence. You give it a "parent" object (which determines what
properties and verbs your new object will inherit), and it gives you back a brand new object with its own unique
identifier. Think of this identifier as the object's permanent address — once created, that's how the system (and your
code)
will refer to that specific object forever.

For details on how inheritance works and how to choose good parent objects, see
the [Object Parents and Children](object-parents-and-children.md) chapter.

> **In Practice**: While `create()` is the fundamental built-in function that brings objects into existence, most
> builders will actually use higher-level commands provided by their MOO's core (like `@create` or `@dig`). These
> commands handle permissions, setup, and other details for you.
> We'll cover [how this works in practice](#creating-objects-in-practice) later in this chapter.

## Object identifier types

mooR supports two types of object identifiers: numbered IDs like `#123` and UUID identifiers like `#048D05-1234567890`.
Both work exactly the same way in your code, but they're designed for different purposes.

For details about these identifier types and when to use each one, see
the [Two Ways to Identify Objects](objects-in-the-moo-database.md#two-ways-to-identify-objects) section.

## Choosing which type to create

When you call `create(parent)`, the system creates whichever type your server is configured for. If your server has
`use_uuobjids` turned on, you will most likely get an object with a UUID.

The full signature for `create()` is: `create(parent [, owner] [, obj_type] [, init_args])`

So for example if you specifically need UUID objects, you can request them by specifying the object type:

- `create(parent, player, 2)` - creates a UUID object owned by player
- `create(parent, this, 2)` - creates a UUID object owned by this object
- `create($thing, #0, 2)` - creates a UUID object owned by the system object

The `obj_type` parameter controls which naming system to use:

- `0` (or `false`) = numbered objects like `#123`
- `1` (or `true`) = anonymous objects (requires `anonymous_objects` feature to be enabled)
- `2` = UUID objects like `#048D05-1234567890`

> **Anonymous Objects**: mooR supports "anonymous" objects (type `1` or `true`) that are automatically garbage
> collected when no longer referenced, similar to objects in other programming languages. This feature must be
> explicitly enabled by your server administrator via the `anonymous_objects` configuration flag, as it introduces
> garbage collection overhead. For details about anonymous objects, see the
> [Anonymous Objects section](objects-in-the-moo-database.md#anonymous-objects) in the objects chapter.

Your server administrator controls which type is created by default through configuration settings. For details on how
this works, see the [Server Configuration](../the-system/server-configuration.md) documentation.

## Creating Objects in Practice

While the `create()` and `recycle()` built-in functions are the underlying mechanics for object creation and
destruction, most MOO cores provide higher-level interfaces for builders and players.

**Administrative Commands**: Most cores provide commands like:

- `@create <parent>` - Creates a new object with the specified parent
- `@recycle <object>` - Destroys an object safely with permission checks
- `@dig <room name>` - Creates and sets up a new room

**Object Recyclers**: Many cores implement "recycler" systems—special objects that handle the lifecycle of created
objects. For example, when you use `@create`, it might:

1. Check your permissions and quotas
2. Call the appropriate `create()` function
3. Set up initial properties and ownership
4. Log the creation for administrative purposes
5. Handle any initialization or setup routines

**Building Interfaces**: Some cores provide menu-driven or web-based building interfaces that hide the complexity of
direct object creation, making it easier for non-programmers to build content.

The exact commands and interfaces available depend on which core your MOO is using. See
the [Understanding MOO Cores](../the-system/understanding-moo-cores.md) chapter for more information about how different
cores approach object management.

Whenever the `create()` function is used to create a new object, that object's `initialize` verb, if any, is called with
no arguments. The call is simply skipped if no such verb is defined on the object.

## Cleaning up and recycling

Since MOO objects don't automatically disappear like they do in other programming languages, you need to explicitly
clean them up when you no longer need them. This is where the `recycle()` function comes in.

The `recycle()` function destroys an object permanently. Just before it actually destroys the object, it calls the
object's `recycle` verb (if any) to give the object a chance to clean up after itself—perhaps removing itself from
lists, notifying other objects, or saving important data elsewhere.

> **Note**: `recycle()` cannot be used on anonymous objects, as they are automatically garbage collected when no longer
> referenced. Attempting to recycle an anonymous object will raise `E_INVARG`.

For more details about why you might need to be careful about cleaning up objects, including the "number slot problem"
with numbered objects,
see [Objects in the MOO Database](objects-in-the-moo-database.md#the-number-slot-problem-numbered-ids-only).

Permissions to create a child of an object, or to recycle an object, are controlled by the permissions and ownerships
constraints described in the [Objects in the MOO database](objects-in-the-moo-database.md) chapter. In particular,
the `create()` function will raise `E_PERM` if the caller does not have permission to create a child of the parent
object, and the `recycle()` function will raise `E_PERM` if the caller does not have permission to recycle the object
being destroyed. Documentation on `create()` and `recycle()` in
the [Manipulating Objects](../the-moo-programming-language/built-in-functions/objects.md) chapter describes
the exact permissions required for each function.
// TODO: Quota support as described below is not yet implemented in the mooR server, but may be in the future. Most
// modern cores instead implement this functionality in-core, however.

Both `create()` and `recycle()` check for the existence of an `ownership_quota` property on the owner of the
newly-created or -destroyed object. If such a property exists and its value is an integer, then it is treated as a
_quota_ on object ownership. Otherwise, the following two paragraphs do not apply.

The `create()` function checks whether or not the quota is positive; if so, it is reduced by one and stored back into
the `ownership_quota` property on the owner. If the quota is zero or negative, the quota is considered to be exhausted
and `create()` raises `E_QUOTA`.

The `recycle()` function increases the quota by one and stores it back into the `ownership_quota` property on the owner.

> **Lightweight alternatives: Flyweights**
>
> Because objects are "expensive" (they take up permanent number slots and require manual cleanup), mooR provides *
*flyweights** as a lightweight alternative for creating lots of small, temporary objects. Flyweights don't get object
> numbers, don't persist in the database, and automatically disappear when no longer needed—perfect for things like
> inventory items, temporary game pieces, or UI elements.
>
> For more details, see the [Flyweights section](moo-value-types.md#flyweights---lightweight-objects) in the value types
> documentation.
