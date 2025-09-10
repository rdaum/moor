# The MOO Programming Language

MOO stands for "MUD, Object Oriented." MUD, in turn, has been said to stand for many different things, but I tend to
think of it as "Multi-User Dungeon" in the spirit of those ancient precursors to MUDs, Adventure and Zork.

MOO, the programming language, is a relatively small and simple object-oriented language designed to be easy to learn
for most non-programmers; most complex systems still require some significant programming ability to accomplish,
however.

For more experienced programmers, or people who've done some programming in other languages like Python or JavaScript,
MOO code will appear familiar -- and in some places a bit odd. In large part this is because MOO was developed before
those languages were popular or even invented. (See below for a brief discussion of what kinds of differences you
might expect to see.)

## Where Code Lives: Verbs

Before diving into the syntax of MOO code, it's important to understand where MOO code actually lives and how it's
organized.

Unlike most programming languages that have functions, procedures, or methods as their primary units of code
organization, MOO has **verbs**. All MOO code lives inside verbs, which are attached to objects in the database.

In traditional programming languages, you might write:

```python
def greet_player(name):
    print(f"Hello, {name}!")
```

In MOO, there are no standalone functions. Instead, you would create a verb on an object:

```moo
// This code lives in a verb, perhaps called "greet" on some object
player:tell("Hello, ", dobj.name, "!");
```

### Key Concepts About Verbs:

- **All code is in verbs**: There are no global functions, procedures, or scripts in MOO. Every piece of executable code
  must be a verb on some object.
- **Verbs belong to objects**: Each verb is owned by a specific object and can access that object's properties and other
  verbs.
- **Verbs are called, not functions**: When you want to execute code, you call a verb on an object, not a function.
  See [Calling Verbs](the-moo-programming-language/calling-verbs.md) for details.
- **Verbs contain statements**: Inside each verb, you
  write [statements](the-moo-programming-language/moo-language-statements.md) that describe the behavior you want.

This design reflects MOO's object-oriented nature—everything is an object, and all behavior is defined as verbs on those
objects. While this might seem limiting at first, it actually provides a very clear and consistent way to organize code
in a persistent, multi-user environment.

## What MOO programs are like

Having given you enough context to allow you to understand exactly what MOO code is doing, I now explain what MOO code
looks like and what it means. I begin with the syntax and semantics of expressions, those pieces of code that have
values. After that, I cover statements, the next level of structure up from expressions. Next, I discuss the concept of
a task, the kind of running process initiated by players entering commands, among other causes. Finally, I list all of
the built-in functions available to MOO code and describe what they do.

First, though, let me mention comments. You can include bits of text in your MOO program that are ignored by the server.
The idea is to allow you to put in notes to yourself and others about what the code is doing. To do this, begin the text
of the comment with the two characters `/*` and end it with the two characters `*/`; this is just like comments in the C
programming language. Note that the server will completely ignore that text; it will _not_ be saved in the database.
Thus, such comments are only useful in files of code that you maintain outside the database.

To include a more persistent comment in your code, try using a character string literal as a statement. For example, the
sentence about peanut butter in the following code is essentially ignored during execution but will be maintained in the
database:

```
for x in (players())
  "Grendel eats peanut butter!";
  player:tell(x.name, " (", x, ")");
endfor
```

> Note: In practice, the only style of comments you will use is quoted strings of text. Get used to it. Another thing of
> note is that these strings ARE evaluated. Nothing is done with the results of the evaluation, because the value is not
> stored anywhere-- however, it may be prudent to keep string comments out of nested loops to make your code a bit
> faster.

### Differences from Other Languages

MOO is a relatively simple language, but it does have some features that may be oddities to programmers used to other
dynamic scripting languages like Python or JavaScript. Here are some of the most notable differences:

* 1-indexed lists: MOO lists are 1-indexed, meaning that the first element of a list is at index 1, not 0 as in many
  other languages. This can be confusing at first, but it is consistent throughout the language. This is common in many
  earlier programming languages (like Pascal or BASIC) and is a design choice made by the original MOO language
  designers in the early 1990s. It also lends itself to a more natural way of thinking about lists in the context of new
  programmers.

* List syntax using `{}`: MOO uses curly braces `{}` to denote lists, which is different from many other languages that
  use square' brackets `[]`. This is a stylistic choice that has historical roots in the original MOO language design

* Map syntax using `[]`: MOO uses square brackets `[]` to denote maps (or dictionaries), which is opposite to languages
  like Python that use curly braces `{}` for dictionaries. This is primarily because the `{}` syntax was already taken
  for lists in MOO when Stunt/ToastSunt added maps to the language.

* No `null` or `None`: MOO does not have a `null` or `None` value like many other languages.

* Immutable strings, lists, maps, and sets: MOO's strings, lists, maps, and sets are immutable, meaning that once they
  are created, they cannot be changed. Instead, you create new versions of these data structures with the desired
  changes. Special syntax is provided for updating *variables* that contain these data structures, but in those cases
  the variable itself is being updated, not the value. In generaly *there are no references* in the MOO programming
  language, just values.

* Object-oriented programming in MOO is different from many other languages. MOO uses a prototype-based
  inheritance model, where objects can inherit properties and methods from other objects without the need for classes.
  This is different from languages like Java or C# that use class-based inheritance.

* Persistent objects: MOO objects are persistent, meaning that they exist in the database and can be accessed by
  multiple tasks. This is different from many other languages where objects are created and destroyed in memory during
  program execution. Traditional MOO has no concept of transient ephemeral objects, so all objects are persistent.
  However, `mooR` does support "anonymous" objects that are automatically garbage collected when no longer referenced,
  similar to objects in other programming languages. `mooR` also has a special object-like value called a "flyweight"
  that is used to represent small lightweight immutable values which have object-like properties, but these are not
  full objects, cannot be inherited from, and persist only inside properties, not as
  "rooted" objects in the database.
