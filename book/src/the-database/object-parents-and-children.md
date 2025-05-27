# Object Inheritance / Parents and Children

## Why inheritance matters

Inheritance is how builders can share their work in MOO. Imagine you've built a perfect "generic weapon" that knows how to be wielded, dropped, and examined. Instead of rebuilding all that functionality for every sword, axe, and bow, other objects can simply inherit from your weapon and automatically get all those abilities. This saves enormous amounts of time and keeps the MOO world consistent.

You may have used inheritance in another programming language, but MOO's approach is different and particularly well-suited for virtual worlds. Instead of abstract classes, MOO uses real, working objects as templates. This means you can actually interact with the "generic weapon" object itself—it's not just a blueprint, it's a functioning example.

## How inheritance works

Every object in the MOO database has a parent object, which is another object in the database. The parent object can be
thought of as a template for the child object, providing default values for properties and default implementations for
verbs. This hierarchy allows for inheritance, where a child object can override the properties and verbs of its parent  
object.

**What makes MOO inheritance special:**

Note that this style of inheritance is not the same as the "class"ical inheritance found in many object-oriented
programming languages. By this we mean that the parent object is not a "class" in the sense of defining a type, but
rather a "prototype" that provides default values and implementations for the child object. In fact this kind of
inheritance is often referred to as "prototype-based inheritance" or "delegation-based inheritance", and is a key
feature of MOO programming which works well with the multiuser, interactive nature of MOO.

**Working with parents:**

To access the parent object of an object, you can use the `parent` property. For example, if you have an object
with the number `#123`, you can access its parent object like this:

```
let parent = #123.parent;
```

The parent property itself is not writable, so you cannot change the parent of an object directly.

Instead, the `chparent` builtin function is used to change the parent of an object. This function takes two arguments:
the object to change
and the new parent object. For example, to change the parent of the object with number `#123` to the object with number
`#456`, you would do:

```
chparent(#123, #456);
```

Along with `chparent`, there is also the builtin function `children()` which returns a list of all the child objects of
a given parent object. For example, to get a list of all the children of the object with number `#456`, you would do:

```
let children = children(#456);
=> { #123, #789, ... }
```

This will return a list of all the objects that have `#456` as their parent. Note that this list is not necessarily
ordered in any particular way, so you may need to sort it if you want a specific order.

The special "nothing" object, which is designated as `#-1`, is the parent of all objects that do not have a parent, and
offers no properties or verbs. It is a placeholder for objects that do not have a parent, and is used to indicate that
an
object is at the root of an inheritance hierarchy.

## Rules for inheritance

MOO keeps inheritance simple by enforcing a few important rules:

**One parent only**

Every object can have only one parent—no multiple inheritance. Think of it like a family tree: you can't have two biological fathers. While some programming languages allow objects to inherit from multiple parents, MOO deliberately keeps it simple. This avoids confusing situations where two parents might define the same property or verb differently.

For example, you can't make a "flying sword" that inherits from both a "generic weapon" and a "flying object" at the same time. You'd need to pick one as the parent and add the flying abilities manually.

**No circular families**

Objects can't create inheritance loops. This means:
- An object can't be its own parent (obviously!)
- An object can't have a parent that is actually one of its children
- You can't create chains like: A inherits from B, B inherits from C, C inherits from A

This is just like real families—you can't be your own grandparent! These rules ensure the inheritance hierarchy forms a clean tree structure where relationships flow in one direction.

**Why these restrictions matter**

These simple rules prevent confusing situations and make it easy to understand where an object gets its properties and verbs from. When you look at any object, you can trace a clear path up through its ancestors without getting lost in complex webs of relationships.