# MOO Value Types

There are only a few kinds of values that MOO programs can manipulate:

- integers (in a specific, large range)
- real numbers (represented with floating-point numbers)
- strings (of characters)
- object numbers (of the permanent objects in the database)
- object references (to the anonymous objects in the database)
- bools
- WAIFs
- errors (arising during program execution)
- lists (of all of the above, including lists)
- maps (of all of the above, including lists and maps)

## Integer Type

ToastStunt supports 64 bit integers, but it can also be configured to support 32 bit. In MOO programs, integers are written just as you see them here, an optional minus sign followed by a non-empty sequence of decimal digits. In particular, you may not put commas, periods, or spaces in the middle of large integers, as we sometimes do in English and other natural languages (e.g. 2,147,483,647).

> Note: The values $maxint and $minint define in the database the maximum integers supported. These are set automatically with ToastCore. If you are migrating from LambdaMOO it is still a good idea to check that these numbers are being set properly.

## Real Number Type

Real numbers in MOO are represented as they are in almost all other programming languages, using so-called _floating-point_ numbers. These have certain (large) limits on size and precision that make them useful for a wide range of applications. Floating-point numbers are written with an optional minus sign followed by a non-empty sequence of digits punctuated at some point with a decimal point '.' and/or followed by a scientific-notation marker (the letter 'E' or 'e' followed by an optional sign and one or more digits). Here are some examples of floating-point numbers:

```
325.0   325.  3.25e2   0.325E3   325.E1   .0325e+4   32500e-2
```

All of these examples mean the same number. The third of these, as an example of scientific notation, should be read "3.25 times 10 to the power of 2".

Fine point: The MOO represents floating-point numbers using the local meaning of the C-language `double` type, which is almost always equivalent to IEEE 754 double precision floating point. If so, then the smallest positive floating-point number is no larger than `2.2250738585072014e-308` and the largest floating-point number is `1.7976931348623157e+308`.

- IEEE infinities and NaN values are not allowed in MOO.
- The error `E_FLOAT` is raised whenever an infinity would otherwise be computed.
- The error `E_INVARG` is raised whenever a NaN would otherwise arise.
- The value `0.0` is always returned on underflow.

## String Type

Character _strings_ are arbitrarily-long sequences of normal, ASCII printing characters. When written as values in a program, strings are enclosed in double-quotes, like this:

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

MOO strings may not include special ASCII characters like carriage-return, line-feed, bell, etc. The only non-printing characters allowed are spaces and tabs.

Fine point: There is a special kind of string used for representing the arbitrary bytes used in general, binary input and output. In a _binary string_, any byte that isn't an ASCII printing character or the space character is represented as the three-character substring "\~XX", where XX is the hexadecimal representation of the byte; the input character '~' is represented by the three-character substring "~7E". This special representation is used by the functions `encode_binary()` and `decode_binary()` and by the functions `notify()` and `read()` with network connections that are in binary mode. See the descriptions of the `set_connection_option()`, `encode_binary()`, and `decode_binary()` functions for more details.

MOO strings can be 'indexed into' using square braces and an integer index (much the same way you can with lists):

```
"this is a string"[4] -> "s"
```

There is syntactic sugar that allows you to do:

```
"Sli" in "Slither"
```

as a shortcut for the index() built-in function.

## Object Type

_Objects_ are the backbone of the MOO database and, as such, deserve a great deal of discussion; the entire next section is devoted to them. For now, let it suffice to say that every object has a number, unique to that object.

In programs, we write a reference to a particular object by putting a hash mark (`#`) followed by the number, like this:

```
#495
```

> Note: Referencing object numbers in your code should be discouraged. An object only exists until it is recycled. It is technically possible for an object number to change under some circumstances. Thus, you should use a corified reference to an object ($my_special_object) instead. More on corified references later.

Object numbers are always integers.

There are three special object numbers used for a variety of purposes: `#-1`, `#-2`, and `#-3`, usually referred to in the ToastCore database as `$nothing`, `$ambiguous_match`, and `$failed_match`, respectively.

## Anonymous Object Type

Anonymous Objects are references and do not have an object number. They are created by passing the optional third argument to `create()`. Anonymous objects are automatically garbage collected when there is no longer any references to them (in your code or in properties).

We will go into more detail on Anonymous Objects in the [Working with Anonymous Objects](#working-with-anonymous-objects) section.

## Bool Type

_bools_ are either true or false. Eg: `my_bool = true; my_second_bool = false;`. In MOO `true` evaluates to `1` and `false` evaluates to `0`. For example:

```
false == 0 evaluates to true
true  == 1 evaluates to true
false == 1 evaluates to false
true  == 0 evaluates to false
true  == 5 evaluates to false
false == -43 evaluates to false
```

## WAIF Type

_WAIFs_ are lightweight objects. A WAIF is a value which you can store in a property or a variable or inside a LIST or another WAIF. A WAIF is smaller in size (measured in bytes) than a regular object, and it is faster to create and destroy. It is also reference counted, which means it is destroyed automatically when it is no longer in use. An empty WAIF is 72 bytes, empty list is 64 bytes. A WAIF will always be 8 bytes larger than a LIST (on 64bit, 4 bytes on 32bit) with the same values in it.

> Note: WAIFs are not truly objects and don't really function like one. You can't manipulate a WAIF without basically recreating a normal object (and then what's the point?). It may be better to think of a WAIF as another data type. It's closer to being a list than it is to being an object. But that's semantics, really.

WAIFs are smaller than typical objects, and faster to create. A WAIF has two builtin OBJ properties, .class and .owner. A WAIF is only ever going to be 4 bytes larger than a LIST with the same values.

OBJs grow by value_bytes(value) - value_bytes(0) for every property you set (that is, every property which becomes non-clear and takes on its own value distinct from the parent). LISTs and WAIFs both grow by value_bytes(value) for each new list element (in a LIST) or each property you set (in a WAIF). So a WAIF is never more than 4 bytes larger than a LIST which holds the same values, except WAIFs give each value a name (property name) but LISTs only give them numbers.

Essentially you should consider a WAIF as something you can make thousands of in a verb without a second thought. You might make a mailing list with 1000 messages, each a WAIF (instead of a LIST) but you most likely wouldn't use 1000 objects.

You create and destroy OBJs explicitly with the builtins create() and recycle() (or allocate them from a pool using verbs in the core). They stay around no matter what you do until you destroy them.

All of the other types you use in MOO (that require allocated memory) are reference counted. However you create them, they stay around as long as you keep them in a property or a variable somewhere, and when they are no longer used, they silently disappear, and you can't get them back.

We will go into more detail on WAIFs in the [Working with WAIFs](#working-with-waifs) section.

## Error Type

_Errors_ are, by far, the least frequently used values in MOO. In the normal case, when a program attempts an operation that is erroneous for some reason (for example, trying to add a number to a character string), the server stops running the program and prints out an error message. However, it is possible for a program to stipulate that such errors should not stop execution; instead, the server should just let the value of the operation be an error value. The program can then test for such a result and take some appropriate kind of recovery action. In programs, error values are written as words beginning with `E_`. The complete list of error values, along with their associated messages, is as follows:

| Error     | Description                     |
| --------- | ------------------------------- |
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

## List Type

Another important value in MOO programs is _lists_. A list is a sequence of arbitrary MOO values, possibly including other lists. In programs, lists are written in mathematical set notation with each of the elements written out in order, separated by commas, the whole enclosed in curly braces (`{` and `}`). For example, a list of the names of the days of the week is written like this:

```
{"Sunday", "Monday", "Tuesday", "Wednesday",
 "Thursday", "Friday", "Saturday"}
```

> Note: It doesn't matter that we put a line-break in the middle of the list. This is true in general in MOO: anywhere that a space can go, a line-break can go, with the same meaning. The only exception is inside character strings, where line-breaks are not allowed.

## Map Type

The final type in MOO is a _map_. It is sometimes called a hashmap, associative array, or dictionary in other programming languages. A map is written as a set of key -> value pairs, for example: `["key" -> "value", 0 -> {}, #15840 -> []]`. Keys must be unique.

The key of a map can be:

- string
- integer
- object
- error
- float
- anonymous object (not recommended)
- waif
- bool

The value of a map can be any valid MOO type including another map.

> Note: Finding a value in a list is BigO(n) as a it uses a linear search. Maps are much more effective and are BigO(1) for retrieving a specific value by key.
