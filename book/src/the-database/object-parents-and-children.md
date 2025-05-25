# Object Inheritance / Parents and Children

Every object in the MOO database has a parent object, which is another object in the database. The parent object can be
thought of as a template for the child object, providing default values for properties and default implementations for
verbs. This hierarchy allows for inheritance, where a child object can override the properties and verbs of its parent  
object.

Note that this style of inheritance is not the same as the "class"ical inheritance found in many object-oriented
programming languages. By this we mean that the parent object is not a "class" in the sense of defining a type, but
rather a "prototype" that provides default values and implementations for the child object. In fact this kind of
inheritance is often referred to as "prototype-based inheritance" or "delegation-based inheritance", and is a key
feature of MOO programming which works well with the multiuser, interactive nature of MOO.

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

## Restrictions on inheritance hierarchy

Objects may not have more than one parent. mooR does not support multiple inheritance, so an object can only have one
parent object. This means that the inheritance hierarchy is a tree structure, where each object has at most one parent,
which -- while less flexible than multiple inheritance -- simplifies the inheritance model and avoids difficulties
associated with multiple inheritance, such as the "diamond problem" where a child object inherits from two parents that
define the same property or verb. In MOO, the parent-child relationship is strictly hierarchical, meaning that an object
can only have one parent, and that parent must be an object in the database.

Objects may also not have a parent that is itself a child of the object, nor can objects "inherit" from themselves.
This means that the inheritance hierarchy cannot form cycles, and that an object cannot be its own parent, just like
in the real world where a person cannot be their own parent. This restriction ensures that the inheritance hierarchy is
a
tree structure, where each object has a unique parent and no cycles are allowed.