# Calling Verbs

## What are verbs?

In MOO, a _verb_ is a piece of code that belongs to an object and can be called to perform actions or calculations. If you're familiar with other programming languages, verbs are similar to what other languages call "methods" - they're functions or behaviours that are attached to objects.

## The concept of message passing

When you call a verb on an object, you're essentially sending that object a message saying "please do this action" or "please tell me this information." The object then looks for a verb with that name and executes it. This is sometimes called "message passing" or "method dispatch" in programming terminology.

Think of it like talking to someone:
- You (the caller) say "Hey Bob, please `tell` me your `name`"
- Bob (the object) hears the message `tell` with argument `name`
- Bob responds by running his `tell` verb with that argument

## Basic verb call syntax

The basic syntax for calling a verb is:

```moo
object:verb_name(arguments...)
```

Here's how it breaks down:
- `object` - The object you want to send the message to (can be an object number like `#123`, a variable, or a system reference like `$player`)
- `:` - The colon tells MOO this is a verb call (not a built-in function)
- `verb_name` - The name of the verb you want to call
- `(arguments...)` - Any arguments you want to pass to the verb, separated by commas

### Simple examples:

```moo
// Tell a player something
player:tell("Hello, world!");

// Ask an object for its name
name = sword:name();

// Move an object to a new location
sword:move(player);

// Call a verb with multiple arguments
player:give_object(sword, 1);
```

## Verbs can have multiple names

One unique feature of MOO verbs is that they can have multiple names, and even use wildcards. This makes them very flexible and convenient to use.

### Multiple names:

A single verb might be defined with several names like `"get take grab"`. This means you can call it using any of those names:

```moo
// All of these call the same verb:
sword:get();
sword:take();
sword:grab();
```

### Wildcard names:

Verbs can also use wildcards (`*`) in their names. For example, a verb named `"*` might respond to any verb call:

```moo
// If an object has a verb named "*", these might all work:
thing:examine();
thing:poke();
thing:whatever();
```

Or a verb named `"get*"` might respond to:

```moo
thing:get();
thing:getall();
thing:getsilver();
```

This flexibility allows objects to respond intelligently to a wide variety of commands, making MOO feel more natural and conversational.

## Dynamic verb calls

Sometimes you don't know the verb name until your program is running. In these cases, you can use a string expression in parentheses:

```moo
verb_name = "tell";
player:(verb_name)("Hello!");

// Or directly with a string:
player:("tell")("Hello!");

// Useful for computed verb names:
action = "get";
target:(action + "_quietly")(player);  // Calls "get_quietly"
```

## Arguments and return values

Like functions in other languages, verbs can:
- Take arguments (input values)
- Return values (output results)
- Have side effects (change the state of objects or the world)

### Passing arguments:

```moo
// Verb with no arguments
time = clock:current_time();

// Verb with one argument
player:tell("You see a sword here.");

// Verb with multiple arguments
player:transfer_money(100, bank_account);
```

### Getting return values:

```moo
// Store the result of a verb call
player_name = player:name();
room_description = here:description();

// Use the result directly
if (sword:is_weapon())
    player:tell("That's a weapon!");
endif
```

### Verbs can fail:

If an object doesn't have the verb you're trying to call, MOO will raise an `E_VERBNF` (verb not found) error:

```moo
// This might raise E_VERBNF if the object doesn't have a "fly" verb
try
    player:fly();
except err (E_VERBNF)
    player:tell("You don't know how to fly!");
endtry
```

## Common patterns

### Chaining verb calls:

```moo
// Get an object from a container, then examine it
item = box:get_item("sword");
description = item:description();
player:tell(description);

// Or chain them together:
player:tell(box:get_item("sword"):description());
```

### Conditional verb calls:

```moo
// Only call a verb if the object has it
if (verb_info(player, "fly"))
    player:fly();
else
    player:tell("You cannot fly.");
endif
```

### Self-references:

Inside a verb, you can call other verbs on the same object using `this`:

```moo
// Inside a verb on the player object:
this:tell("You feel dizzy.");
current_location = this:location();
```

## Best practices

1. **Use descriptive verb names** that clearly indicate what the verb does
2. **Handle missing verbs gracefully** using try/except blocks when needed
3. **Use system references** like `$player` instead of hard-coded object numbers
4. **Consider using `this`** instead of the object's number when calling verbs on the same object
5. **Document your verbs** so other programmers know what arguments they expect

Verb calls are one of the most important concepts in MOO programming - they're how objects communicate with each other and how the virtual world comes alive through interaction!
