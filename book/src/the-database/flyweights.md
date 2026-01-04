# Flyweights

Flyweights are a special value type in mooR designed to represent "lightweight objects". They allow you to bundle data (
slots) and behaviour (verbs) together in a value that doesn't carry the weight of a full database object.

They are particularly useful for:

- **Events**: Passing complex event data (like "player clicked here with modifiers") to handlers.
- **Document Trees**: Representing nodes in XML, HTML, or UI trees where every tag needs to be an object but shouldn't
  clog up the database.
- **Lightweight Entities**: Transient things like "combat instances", "active spells", or "floating text" that behave
  like objects but only exist for a short time.

Unlike database objects, flyweights:

- Are **values**, not references. They are passed efficiently by the system, not by object number.
- Are **immutable**. You cannot change a flyweight in place; you create a new one with modified slots.
- Do not have their own unique object number in the database. Instead, they live inside properties, variables, or
  lists/maps, just like strings or numbers.
- Never have their own verbs; all verb calls are handled by the delegate object.
- Are automatically garbage collected when no longer used.

However, like objects:

- They can receive **verb calls**. Calls are delegated to a "parent" object (the **delegate**).
- They have **slots** (which work like object properties).
- They can contain other values (in a **contents** list).

## Terminology: Why "Slots" and "Delegate"?

Flyweights use special terminology to emphasize that they are **not** full database objects:

- **Slots** instead of "properties": While slots work like object properties (accessed with `.name`), calling them "
  slots" reminds you they are **immutable value storage**, not mutable object properties that can be directly assigned.

- **Delegate** instead of "parent": The delegate object provides verb implementations, but the flyweight itself is a
  separate value. **Flyweights never have verbs of their own**—when you call `flyweight:verb()`, the server finds that
  verb on the delegate object and runs it with `this` set to the **flyweight value** (not the delegate). Using "delegate"
  emphasizes that verbs are **implemented by** another object, not that the flyweight inherits from it in the object
  hierarchy.

This terminology helps distinguish flyweights from real objects. If you need something more object-like that is still
garbage collected but **mutable**, consider
using [anonymous objects](objects-in-the-moo-database.md#anonymous-objects) instead.

## Anatomy of a Flyweight

### The Delegate

Every flyweight has a **delegate** object. When you call a verb on a flyweight, the server looks up that verb on the
delegate object.

Inside the verb:

- `this` refers to the **flyweight value** itself, not the delegate object.
- `caller` is the object that called the verb.

This works exactly like regular object inheritance. Just as `this` refers to the child object even when running code
defined on a parent, here `this` refers to the flyweight even when running code defined on the delegate.

You can access the delegate of a flyweight using the `.delegate` slot:

```moo
let token = < $auth_token >;
player:tell(token.delegate); // prints #<object-number of $auth_token>
```

### Slots (Properties)

Flyweights store data in **slots**. These are accessed using standard dot notation, just like object properties.

```moo
let item = < $thing, .weight = 5 >;
player:tell(item.weight); // prints 5
```

If a slot does not exist on the flyweight, the server does **not** look for a property on the delegate object. It raises
`E_PROPNF` (Property Not Found).

> **Important**: Unlike database objects, you **cannot** assign to a slot using dot notation.
>
> ```moo
> let item = < $thing, .weight = 5 >;
> item.weight = 10; // ERROR! Flyweights are immutable.
> ```
>
> To "change" a slot, you must create a new flyweight using `flyslotset()` (see below).

### Contents

A flyweight can hold a list of values, referred to as its **contents**. This is useful for representing trees,
hierarchical structures (like XML/HTML), or simple containers.

```moo
let node = < $html_div, { "Hello", < $html_span, { "World" } > } >;
```

## Syntax

The literal syntax for a flyweight is enclosed in angle brackets `< ... >`:

```moo
< delegate_object, slot_assignments, contents_list >
```

- **Delegate** (Required): An object reference that handles verb calls.
- **Slots** (Optional): A comma-separated list of slots using `.name = value` syntax.
- **Contents** (Optional): A list of values enclosed in `{ ... }`.

### Examples

```moo
// An event flyweight
< $event, .source = player, .type = "click", .coords = {10, 20} >

// A UI element (document node)
< $ui_button, .label = "Cast Spell", .action = "cast", { "icon_fire.png" } >

// A transient effect
< $poison_effect, .duration = 5, .damage_per_tick = 10 >
```

## Built-in Functions

mooR provides a set of built-in functions to manipulate and inspect flyweights. Since flyweights are immutable, "
modification" functions return a *new* flyweight with the desired changes.

### Creation

#### `toflyweight(obj delegate [, map slots [, list contents]])`

Creates a new flyweight dynamically.

- `delegate`: The object reference to handle verb calls.
- `slots`: (Optional) A map of `symbol -> value` or `string -> value` pairs for slots.
- `contents`: (Optional) A list of values.

```moo
let f = toflyweight($thing, ["name" -> "Dynamic Item"], {"content1"});
```

### Introspection

#### `flyslots(flyweight f)`

Returns a map of all slots defined on the flyweight.

```moo
let f = < $thing, .a = 1, .b = 2 >;
flyslots(f); // Returns ["a" -> 1, "b" -> 2]
```

#### `flycontents(flyweight f)`

Returns the contents list of the flyweight.

```moo
let f = < $thing, { 1, 2, 3 } >;
flycontents(f); // Returns {1, 2, 3}
```

### Modification (Copy-on-Write)

#### `flyslotset(flyweight f, symbol key, any value)`

Returns a **new** flyweight with the specified slot set to the given value. If the slot already exists, it is
overwritten.

```moo
let f1 = < $thing, .a = 1 >;
let f2 = flyslotset(f1, 'a, 100);
let f3 = flyslotset(f1, 'b, 50);

// f1 is unchanged: < $thing, .a = 1 >
// f2 is: < $thing, .a = 100 >
// f3 is: < $thing, .a = 1, .b = 50 >
```

#### `flyslotremove(flyweight f, symbol key)`

Returns a **new** flyweight with the specified slot removed.

```moo
let f1 = < $thing, .a = 1, .b = 2 >;
let f2 = flyslotremove(f1, 'a);

// f2 is: < $thing, .b = 2 >
```

## Usage Patterns

### 1. Events and Messages

Flyweights are ideal for representing events in your system. Instead of passing a list of arguments or a map to an event
handler, you can pass a flyweight that encapsulates the event data and provides utility verbs.

```moo
// Creating an event
let evt = < $click_event, 
            .user = player, 
            .x = 100, 
            .y = 200, 
            .type = "right_click" >;

// The handler can call verbs on the event
// $click_event:describe() might return "Player clicked at 100, 200"
notify(player, evt:describe());
```

### 2. Structured Documents (XML/HTML)

mooR's XML parsing and generation tools often use flyweights to represent DOM nodes. The delegate represents the tag
type (e.g., `$html_div`, `$html_span`), slots represent attributes, and contents represent child nodes.

```moo
// <div class="container">Hello</div>
let dom = < $html_div, .class = "container", { "Hello" } >;
```

### 3. UI and Menu Systems

Transient UI elements that need to handle user interaction but don't need persistence are a perfect fit.

```moo
let button = < $ui_button, .label = "Submit", .action = "save", { "icon_save.png" } >;
```

### 4. Rich Data Transfer

When sending complex data structures between systems (or to a web client), flyweights provide a structured way to bundle
data with behavior, unlike plain Maps or Lists.

### 5. Fluent Interfaces

Because flyweights are immutable and cheap to copy, they lend themselves well to the "fluent" or "builder" pattern. You
can define verbs on the delegate that return a modified copy of the flyweight, allowing you to chain method calls.

On the delegate (e.g., `$event`):

```moo
verb with_timestamp(new_time)
    return flyslotset(this, 'timestamp, new_time);
endverb

verb with_source(new_source)
    return flyslotset(this, 'source, new_source);
endverb
```

Usage:

```moo
let base_event = < $event >;
let log_entry = base_event:with_timestamp(time()):with_source(player);
```

## Immutability & Performance

Because flyweights are immutable values, the server can handle them very efficiently. Copying a flyweight is cheap (it
just increments a reference count). Modifying a flyweight (creating a modified copy) is also optimized.

This immutability also makes them safe to pass around; you never have to worry about a called verb modifying your data
unexpectedly.
