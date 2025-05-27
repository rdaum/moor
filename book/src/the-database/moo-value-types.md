# Kinds of values

There are only a few kinds of values that MOO programs can manipulate, and that can be stored inside objects in the
mooR database.

- Integers (in a specific, large range)
- Floats / Real numbers  (represented with floating-point numbers)
- Strings (of characters)
- Symbols (special labels for naming things in code)
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

## Symbol Type

_Symbols_ are a special kind of text value that mooR adds to the original MOO language. Think of symbols as "smart
labels" that are perfect for naming things and organizing your code.

### What makes symbols different from strings?

While strings (like `"hello"`) are great for text that users will see, symbols are designed for text that your program
uses internally - like labels, categories, or identifiers.

To create a symbol, you put a single quote (apostrophe) before the text, like this:

```
'hello
'player_name
'room_description
```

### Key differences between symbols and strings:

**Symbols express intent as identifiers**

- Using `'name` clearly shows you mean it as an identifier or property name
- Using `"name"` suggests it's text content that might be displayed to users
- This makes your code's purpose clearer to other programmers

**Symbols have restricted characters**

- Symbols can only contain letters, numbers, and underscores
- No spaces, punctuation, or special characters (except `_`)
- Examples: `'player_name` ✓, `'hello world` ✗, `'item-count` ✗

**Symbols don't support string operations**

- You can't slice symbols like `'hello`[1..3]
- You can't index into them like `'test`[2]
- They're meant to be used whole, not manipulated like text

**Symbols with the same text are identical**

- Every time you write `'hello` in your code, it's the exact same symbol
- This makes comparing symbols very fast

### Simple examples:

```moo
// Symbols express clear intent:
player_stats = ['name -> "Alice", 'score -> 100, 'level -> 5];

// Symbols can't contain spaces or special characters:
'player_name    // ✓ Valid symbol
'hello_world    // ✓ Valid symbol  
'item2_count    // ✓ Valid symbol
'hello world    // ✗ Invalid - contains space
'item-count     // ✗ Invalid - contains hyphen

// Using symbols for states:
if (game_state == 'running)
    // Game is active
endif
```

### When should you use symbols?

**Good uses for symbols:**

- Property names: `'description`, `'location`, `'owner`
- Game states: `'running`, `'paused`, `'finished`
- Categories: `'weapon`, `'armor`, `'tool`
- Commands: `'look`, `'take`, `'drop`

**Better to use strings for:**

- Messages shown to players: `"Hello, welcome to the game!"`
- Descriptions: `"A rusty old sword"`
- User input that might change

### Converting between symbols and strings:

You can easily switch between symbols and strings:

```moo
// String to symbol:
my_symbol = tosym("hello");    // Creates 'hello

// Symbol to string:
my_string = tostr('world);     // Creates "world"
```

### Technical note:

The symbol feature is turned on by default in mooR. Server administrators can turn it off with the `--symbol-type=false`
option, but most servers keep it enabled because symbols make code faster and cleaner.

## Binary Type

_Binary_ values are sequences of bytes that can represent arbitrary binary data - like images, compressed files,
encrypted data, or any other non-text information.

### Writing binary literals

In MOO programs, binary values are written using a special prefix syntax with `b"` followed by a base64-encoded
string and ending with `"`, like this:

```moo
b"SGVsbG8gV29ybGQ="    // This represents the text "Hello World" as binary data
b""                   // An empty binary value
b"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="  // A 1x1 pixel PNG image
```

The content inside the quotes must be valid base64 encoding. If you provide invalid base64 data, you'll get a
parsing error.

### What can you do with binary values?

Binary values work like other sequence types in MOO - you can:

- **Index into them**: `my_binary[1]` returns the first byte as an integer (0-255)
- **Get their length**: `length(my_binary)` tells you how many bytes it contains
- **Search in them**: `65 in my_binary` checks if byte value 65 exists in the binary
- **Take slices**: `my_binary[1..10]` gets the first 10 bytes
- **Append to them**: `my_binary + other_binary` or `my_binary + 255` (to add a single byte)
- **Convert to/from strings**: Using built-in functions when the binary represents text data

### Working with binary data

mooR provides built-in functions for working with binary data:

- `decode_base64(string, [url-safe])` - Converts a base64 string to binary data
- `encode_base64(binary)` - Converts binary data to a base64 string

### When should you use binary values?

**Good uses for binary:**

- Storing image, audio, or video data
- Handling compressed or encrypted information
- Working with network protocols that use binary formats
- Interfacing with external systems that expect raw bytes
- Storing any non-text data efficiently

**Better to use strings for:**

- Regular text that users will read
- Configuration data and settings
- Most game content and descriptions

### Important notes:

- Binary values are immutable, just like strings and lists in MOO
- When you "modify" a binary value, you actually create a new one
- Binary data is stored efficiently and doesn't waste space on encoding overhead
- You can safely store any byte values (0-255) without worrying about text encoding issues

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
the LambdaCore database as `$nothing`, `$ambiguous_match`, and `$failed_match`, respectively.

## Flyweights - lightweight objects

_Flyweights_ are a special type that mooR adds to help you create lots of small, temporary objects without using too
much memory or slowing down your MUD. Think of them as "mini-objects" that can hold data and respond to verbs, but are
much lighter than real database objects.

### Why use flyweights instead of regular objects?

**Regular objects are "heavy":**

- Each object takes up database space permanently
- Creating many objects can slow down your MOO
- Objects need to be cleaned up manually or they stick around forever

**Flyweights are "light":**

- You can create thousands of them quickly without performance problems
- They automatically disappear when no longer needed
- They can't be changed once created (immutable)
- They exist only as _values_ in other things (properties, variables, arguments)

### What can flyweights do?

Flyweights combine the best parts of objects, lists, and maps:

- **Like objects**: They can have verbs called on them
- **Like maps**: They can store named properties ("slots")
- **Like lists**: They can contain other values

### Flyweight syntax:

The basic pattern is: `< delegate_object, [slots], {contents} >`

- **Delegate object** (required): The object that handles verb calls
- **Slots** (optional): Named properties, like a map
- **Contents** (optional): A list of other values

### Simple examples:

```moo
// Just a delegate - simplest flyweight:
< #123 >

// With some data slots:
< $generic_item, ['name -> "magic sword", 'power -> 15] >

// With contents (like inventory):
< $container, ['name -> "treasure chest"], {"gold coins", "ruby", "scroll"} >

// Complex example - a room in a maze:
< $maze_room, 
  ['description -> "A twisty passage", 'exits -> {"north", "south"}],
  {player1, player2} >
```

### When should you use flyweights?

**Great for flyweights:**

- Inventory items that aren't permanent
- Temporary game pieces (chess pieces, cards, etc.)
- Menu items and UI elements
- Parts of a large structure (maze rooms, building floors)
- Anything you need lots of that's similar but not identical

**Better to use regular objects for:**

- Players and important NPCs
- Rooms that should persist between server restarts
- Valuable items that players own long-term
- Anything that needs to be saved in the database

### How verb calls work:

When you call a verb on a flyweight, it looks for the verb on the delegate object:

```moo
// Create a flyweight sword:
sword = < $weapon, ['damage -> 10, 'name -> "iron sword"] >;

// Call a verb - this will look for "wield" on $weapon:
sword:wield(player);
```

### Accessing flyweight data:

You can read the slots (properties) of a flyweight:

```moo
sword = < $weapon, ['damage -> 10, 'name -> "iron sword"] >;
damage_value = sword.damage;    // Gets 10
weapon_name = sword.name;       // Gets "iron sword"
```

### Working with XML and web interfaces:

Flyweights are especially useful for building web pages because they can be easily converted to and from XML:

```moo
// A flyweight representing HTML structure:
div_element = < $html_div, 
               ['class -> "player-info"], 
               {"Player: Alice", "Score: 1500"} >;

// Convert to XML string:
html_string = to_xml(div_element);
```

### Important notes:

- Flyweights cannot be changed once created - they're immutable
- They only exist while your program is running
- They're perfect for temporary data structures
- The `player` variable can never be a flyweight (but `this` and `caller` can be)

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

An example of an error value with an attached message might look like this:

```moo
let my_error = E_TYPE("Expected a number, but got a string.");
error_message(my_error); // Returns "Expected a number, but got a string."
```

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

