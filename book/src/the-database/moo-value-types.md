# MOO Value Types

There are only a few kinds of values that MOO programs can manipulate:

- Integers (in a specific, large range)
- Floats / Real numbers  (represented with floating-point numbers)
- Strings (of characters)
- Object numbers (references to the permanent objects in the database)
- "Flyweights" - anonymous lightweight object _values_
- Errors (arising during program execution)
- Lists (of all of the above, including lists)
- Maps (of all of the above, including lists and maps)

## Integer Type

mooR integers are signed, 64-bit integers.

In MOO programs, integers are written just as you see them here, an optional minus sign followed by a non-empty sequence
of decimal digits. In particular, you may not put commas, periods, or spaces in the middle of large integers, as we
sometimes do in English and other natural languages (e.g. 2,147,483,647).

> Note: Many databases define the values $maxint and $minint. Core databases built for LambdaMOO or ToastStunt may not
> have values set which correspond to the correct (64-bit signed integer) maximum / minimum values for `mooR`.
> In general, the integer range supported by mooR tends to be at least as large as the integer range supported by other
> servers (such as LambdaMOO and ToastStunt), so this shouldn't lead to any errors.

## Floats / Real Number Type

Real numbers in MOO are represented as they are in almost all other programming languages, using so-called
_floating-point_ numbers. These have certain (large) limits on size and precision that make them useful for a wide range
of applications. Floating-point numbers are written with an optional minus sign followed by a non-empty sequence of
digits punctuated at some point with a decimal point '.' and/or followed by a scientific-notation marker (the letter 'E'
or 'e' followed by an optional sign and one or more digits). Here are some examples of floating-point numbers:

```
325.0   325.  3.25e2   0.325E3   325.E1   .0325e+4   32500e-2
```

All of these examples mean the same number. The third of these, as an example of scientific notation, should be read "
3.25 times 10 to the power of 2".

Fine point: The MOO represents floating-point numbers using the local meaning of the Rust `f64` type. Maximum and
minimum values generally follow the constraints placed by the Rust compiler and library on those types.

To maintain backwards compatibility with LambdaMOO, in mooR:

- IEEE infinities and NaN values are not allowed in MOO.
- The error `E_FLOAT` is raised whenever an infinity would otherwise be computed.
- The error `E_INVARG` is raised whenever a NaN would otherwise arise.
- The value `0.0` is always returned on underflow.

## String Type

Character _strings_ are arbitrarily-long sequences of normal, UTF-8 characters.

When written as values in a program, strings are enclosed in double-quotes, like this:

```
"This is a character string."
```

To include a double-quote in the string, precede it with a backslash (`\`), like this:

```
"His name was \"Leroy\", but nobody ever called him that."
```

Finally, to include a backslash in a string, double it:

```
"Some people use backslash ('\\') to mean set difference."
```

mooR strings can be 'indexed into' using square braces and an integer index (much the same way you can with lists):

```
"this is a string"[4] -> "s"
```

There is syntactic sugar that allows you to do:

```
"Sli" in "Slither"
```

as a shortcut for the index() built-in function.

## Object Type

_Objects_ are the backbone of the MOO database and, as such, deserve a great deal of discussion; the entire next section
is devoted to them. For now, let it suffice to say that every object has a number, unique to that object.

In programs, we write a reference to a particular object by putting a hash mark (`#`) followed by the number, like this:

```
#495
```

> Note: Referencing object numbers in your code should be discouraged. An object only exists until it is recycled. It is
> technically possible for an object number to change under some circumstances. Thus, you should use a corified
> reference
> to an object ($my_special_object) instead. More on corified references later.

Object numbers are always integers.

There are three special object numbers used for a variety of purposes: `#-1`, `#-2`, and `#-3`, usually referred to in
the ToastCore database as `$nothing`, `$ambiguous_match`, and `$failed_match`, respectively.

## Flyweights - lightweight objects

mooR adds a new type called flyweights which are lightweight, garbage collected bundles of a delegate object,
attributes ("slots") and arbitrary contents, and so combine features of objects, lists, and maps. Verb calls against
flyweights dispatch against the delegate, and property lookups work gainst both the delegate and the flyweight's own set
of slots.

Their syntax looks like:

```
< <delegate>, [ slot -> value, ... ], { contents, ... } >
```

But the only mandatory element is the delegate.

Examples:

```
< $div_tag, [ class -> "background_div" ], { child_node_a, child_node_b } >
< #54 >
< $maze_node, [ description -> "You are in a maze of twisty passages, all alike", exits = { "north", "up" } >
```

During verb execution, `this`, and `caller` can be flyweights. `player` cannot.

Flyweights are useful for representing large quantities of small, lightweight things (like a maze or a large quantity of
small items).

The syntax for flyweights is specifically designed to easily express tree-structured data (like an HTML or XML
document).

There are builtins (`to_xml` and `xml_parse`) for converting properly structured flyweights to/from XML documents, which
are in place to aid in the construction of web user interfaces, or integrations with this party services.

We will go into more detail on Anonymous Objects in the [Working with Flyweights](#working-with-flyweights) section.

## Error Type

_Errors_ represent failures or error conditions while running verbs or builtins.

In the normal case, when a program attempts an operation that is erroneous for some reason (for example, trying to add
a number to a character string), the server stops running
the program and prints out an error message. However, it is possible for a program to stipulate that such errors should
not stop execution; instead, the server should just let the value of the operation be an error value. The program can
then test for such a result and take some appropriate kind of recovery action.

In programs, error values are written as words beginning with `E_`. mooR has a series of built-in error values that
represent common error conditions that can arise during program execution. In addition, it is possible to define
your own error values, which can be used to represent application-specific error conditions, which is done by prefixing
any identifier with `E_` (e.g. `E_MY_ERROR`).

The complete list of error values, along with their associated messages, is as follows:

| Error     | Description                     |
|-----------|---------------------------------|
| E_NONE    | No error                        |
| E_TYPE    | Type mismatch                   |
| E_DIV     | Division by zero                |
| E_PERM    | Permission denied               |
| E_PROPNF  | Property not found              |
| E_VERBNF  | Verb not found                  |
| E_VARNF   | Variable not found              |
| E_INVIND  | Invalid indirection             |
| E_RECMOVE | Recursive move                  |
| E_MAXREC  | Too many verb calls             |
| E_RANGE   | Range error                     |
| E_ARGS    | Incorrect number of arguments   |
| E_NACC    | Move refused by destination     |
| E_INVARG  | Invalid argument                |
| E_QUOTA   | Resource limit exceeded         |
| E_FLOAT   | Floating-point arithmetic error |
| E_FILE    | File system error               |
| E_EXEC    | Exec error                      |
| E_INTRPT  | Interrupted                     |

Error values can also have an optional message associated with them, which can be used to provide additional context
about the error. This message can be set when the error is raised, and it can be retrieved later using the
`error_message` builtin function.

## List Type

Another important value in MOO programs is _lists_. A list is a sequence of arbitrary MOO values, possibly including
other lists. In programs, lists are written in mathematical set notation with each of the elements written out in order,
separated by commas, the whole enclosed in curly braces (`{` and `}`). For example, a list of the names of the days of
the week is written like this:

```
{"Sunday", "Monday", "Tuesday", "Wednesday",
 "Thursday", "Friday", "Saturday"}
```

> Note: It doesn't matter that we put a line-break in the middle of the list. This is true in general in MOO: anywhere
> that a space can go, a line-break can go, with the same meaning. The only exception is inside character strings, where
> line-breaks are not allowed.

## Map Type

mooR adds a Map type to the pantheon of types that LambdaMOO originally offered. In general mooR's Map attempts to copy
the syntax and semantics of the same in ToastStunt.

A map is an associative structure, mapping keys to values. It is often called a dictionary in other languages.

It is written as a set of key -> value pairs, for example: `["key" -> "value", 0 -> {}, #15840 -> []]`. Keys must be
unique.

The key of a map can be of any valid mooR value type. However it is not generally recommended to use maps, lists, or
flyweights as keys as the cost to compare and index them starts to become prohibitive and the map itself difficult to
understand.

> Note: mooR maps are built internally on a sorted vector, and searching within them is a BigO(Log(N)) operation. As
> they are immutable, and a copy must be made for modification, insertions and deletions are an BigO(n) operation. 

