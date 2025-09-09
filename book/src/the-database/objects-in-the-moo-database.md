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

## Anonymous Objects

In addition to numbered and UUID objects, mooR supports a third type: **anonymous objects**. These represent a
fundamentally different approach to object management that's closer to how objects work in other programming languages.

### Understanding the Three Types of Object References

To understand anonymous objects, it's important to first understand how they differ from the traditional object ID
systems:

**Numbered Objects (`#123`) and UUID Objects (`#048D05-1234567890`):**

- These are **object identifiers, not true references**
- Think of them as "names" or "addresses" for objects in the database
- **Pass by value** - when you write `#123`, you're writing a literal number that identifies an object
- **Persistent forever** - the object exists until someone explicitly calls `recycle()`
- **Can be typed directly** in your code - `#123` is something you can literally type

**Anonymous Objects:**

- These are **true object references or "handles"**
- Think of them as pointers that let you talk about an object, but have no "value" themselves
- **Pass by reference** - you can't type an anonymous object reference, only get one from `create()`
- **Temporary** - automatically disappear when nothing references them anymore
- **No literal form** - there's no way to type an anonymous object reference directly in code

### When Anonymous Objects Disappear

Anonymous objects are **garbage collected** using a mark & sweep algorithm that runs periodically in the background. An
anonymous object becomes unreachable and gets deleted when:

- No variables contain references to it
- No object properties contain references to it
- No running tasks have references to it
- No suspended tasks have references to it

This happens automatically - you don't need to call `recycle()` on anonymous objects (in fact, doing so will raise
`E_INVARG`).

### Creating Anonymous Objects

You create anonymous objects by passing `1` (or `true`) as the third argument to `create()`:

```moo
// Create an anonymous object
let temp_obj = create($thing, player, 1);
temp_obj.name = "temporary item";
temp_obj.description = "This won't be around for long";

// When temp_obj goes out of scope or gets reassigned,
// the anonymous object will eventually be garbage collected
temp_obj = 0;  // Now nothing references the anonymous object
```

### Detecting Anonymous Objects

Since anonymous objects have `typeof(obj) == OBJ` (same as regular objects), you need the `is_anonymous()` builtin to
detect them:

```moo
let regular_obj = create($thing);      // Regular numbered/UUID object  
let anon_obj = create($thing, player, 1); // Anonymous object

typeof(regular_obj);    // Returns OBJ
typeof(anon_obj);       // Also returns OBJ - same type!

is_anonymous(regular_obj);  // Returns 0 (false)
is_anonymous(anon_obj);     // Returns 1 (true)
```

> **Porting from ToastStunt**: This is an important difference from ToastStunt, where anonymous objects had
> `typeof(anon_obj) == ANON`. In mooR, you must use `is_anonymous()` to distinguish them.

### The Trade-offs

**Benefits of Anonymous Objects:**

- **Automatic cleanup** - no need to remember to call `recycle()`
- **No ID slot consumption** - doesn't use up numbered object slots
- **Convenient for temporary objects** - perfect for short-lived game objects

**Costs of Anonymous Objects:**

- **Garbage collection overhead** - background mark & sweep cycles consume CPU time
- **Potential pauses** - the sweep phase can briefly pause new task creation
- **Same memory cost** - anonymous objects use just as much memory/storage as regular objects
- **Must be enabled** - requires the `anonymous_objects` feature flag (disabled by default)

### When to Use Anonymous Objects

**Good use cases:**

- Temporary inventory items that come and go
- Short-lived UI elements or game pieces
- Objects created during calculations that don't need to persist
- Any object where manual cleanup is error-prone

**Avoid for:**

- Permanent world fixtures (rooms, important NPCs, etc.)
- Objects that need predictable, immediate cleanup
- Performance-critical code where GC pauses matter
- Systems where you need to reference objects by typed identifiers

### Performance Considerations

The garbage collection system introduces overhead that traditional MOO doesn't have:

- **Background marking** - periodically scans all references to find unreachable objects
- **Sweep pauses** - when collecting garbage, new tasks are briefly blocked
- **Memory overhead** - unreferenced objects stick around until the next GC cycle

For most MOO applications, this overhead is acceptable, but server administrators should be aware that enabling
anonymous objects will impact performance.

### Feature Requirements

Anonymous objects require your server administrator to enable the `anonymous_objects` configuration flag. This feature
is disabled by default due to the garbage collection overhead. If anonymous objects aren't enabled and you try to create
one, you'll get an `E_INVARG` error.

### Comparison with Flyweights

mooR also provides **flyweights**, which are truly lightweight object-like values. Here's how they compare:

| Feature            | Anonymous Objects           | Flyweights                         |
|--------------------|-----------------------------|------------------------------------|
| Storage cost       | Same as regular objects     | Very lightweight                   |
| Inheritance        | Full inheritance support    | No inheritance                     |
| Persistence        | Until garbage collected     | Only exist in variables/properties |
| Performance impact | Garbage collection overhead | Minimal overhead                   |
| Use case           | Temporary full objects      | Small immutable data structures    |

For more on flyweights, see the [Flyweights section](moo-value-types.md#flyweights---lightweight-objects) in the value
types documentation.

## Ways to Reference Objects

Once an object exists, there are several ways you can reference it in your code. The methods available depend on whether
it's a numbered/UUID object or an anonymous object:

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

All of these work exactly the same way for numbered and UUID objects—they're just different ways of referring to the
same objects.

**Anonymous objects work differently**: Since anonymous objects have no literal form, you can only reference them
through variables. You get anonymous object references from `create()` calls and can store them in variables or
properties, but you cannot type them directly like `#123`.

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

> **Converting UUID Objects to Numbered Objects**: If you have UUID objects that you want to convert to numbered objects
> (perhaps for easier reference or integration with existing code), you can use the `renumber()` function. For example,
> `renumber(#048D05-1234567890)` will convert the UUID object to an available numbered object like `#241`. The system
> will automatically find the best available numbered slot, or you can specify an exact target with
> `renumber(#048D05-1234567890, #500)`. This is useful when promoting temporary objects to permanent world fixtures.

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

> **Lightweight alternatives: Flyweights**
>
> Because objects are "permanent residents" of your world (they take up database space and require manual cleanup), mooR
> provides **flyweights** as a lightweight alternative for creating lots of small, temporary objects. Flyweights don't
> get object identifiers, don't persist in the database, and automatically disappear when no longer needed—perfect for
> things like inventory items, temporary game pieces, or UI elements.
>
> For more details, see the [Flyweights section](moo-value-types.md#flyweights---lightweight-objects) in the value types
> documentation.
