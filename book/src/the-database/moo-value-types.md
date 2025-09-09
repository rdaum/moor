# Kinds of values

There are only a few kinds of values that MOO programs can manipulate, and that can be stored inside objects in the mooR
database.

- Integers (in a specific, large range)
- Floats / Real numbers  (represented with floating-point numbers)
- Strings (of characters)
- Lists (ordered sequences of values)
- Maps (associative key-value collections)
- Errors (arising during program execution)
- Symbols (special labels for naming things in code)
- Binary values (arbitrary byte sequences)
- Object references (numbered, UUID, and anonymous objects in the database)
- "Flyweights" - anonymous lightweight object _values_
- Lambdas (anonymous functions with closures)

## Integer Type

Integers are numbers without decimal places.

Technically they are signed, 64-bit integers.

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

**Float errors to watch out for:**

Some mathematical operations on floats can cause errors instead of giving you a result:

- **Division by zero** gives you an `E_DIV` error: `5.0 / 0.0` → `E_DIV`
- **Operations that would be infinite** give you an `E_FLOAT` error: `1.0e308 * 1.0e308` → `E_FLOAT`
- **Invalid operations** give you an `E_INVARG` error: `sqrt(-1.0)` → `E_INVARG`

> **Technical Notes**
>
> The MOO represents floating-point numbers using the local meaning of the Rust `f64` type. Maximum and minimum values
> generally follow the constraints placed by the Rust compiler and library on those types.
>
> To maintain backwards compatibility with LambdaMOO, in mooR:
>
> - IEEE infinities and NaN values are not allowed in MOO.
> - The error `E_FLOAT` is raised whenever an infinity would otherwise be computed.
> - The error `E_INVARG` is raised whenever a NaN would otherwise arise.
> - The value `0.0` is always returned on underflow.

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

## List Type

Lists are one of the most important value types in MOO. A list is an ordered sequence of values, which can include any
kind of MOO value—even other lists! Lists are great for keeping track of collections of things, like player inventories,
search results, or a series of numbers.

In MOO, lists are written using curly braces `{}` with each element separated by a comma:

```
{"apple", "banana", "cherry"}
{1, 2, 3, 4, 5}
{"player1", #123, 42, {"nested", "list"}}
```

You can get the length of a list with `length(my_list)`, access elements by index (starting at 1), and use many built-in
functions to work with lists (like `setadd`, `setremove`, `index`, etc.).

```
fruits = {"apple", "banana", "cherry"};
first_fruit = fruits[1]; // "apple"
fruits = setadd(fruits, "date"); // {"apple", "banana", "cherry", "date"}
```

Lists are immutable: when you "change" a list, you actually create a new one. In the case of some functions for list
manipulation, this is implicit, but existing moo code often does this explicitly. For example, the recommended and
common style for appending to a list is to use the list expansion operator, the @ character, to expand the old list's
contents into a declaration of a new list:

```
newlist = {@oldlist, newelement};
```

The newly declared list can, of course, be assigned to the old list variable:

```
oldlist = {@oldlist, newelement};
```

### List Comprehensions

mooR also provides a concise syntax for creating new lists by transforming existing ones, called list comprehensions:

```
doubled = { x * 2 for x in {1, 2, 3, 4} };    // Creates {2, 4, 6, 8}
squares = { x * x for x in [1..5] };          // Creates {1, 4, 9, 16, 25}
```

This syntax lets you build complex lists in a single, readable expression. For detailed information about list
comprehensions, see the [List Comprehensions](../the-moo-programming-language/list-comprehensions.md) chapter.

(Note that @ is also a standard prefix character to denote certain kinds of user commands, but these two facts are not
connected.)

## Map Type

Maps let you associate keys with values, like a dictionary in other languages. They are perfect for storing things like
player stats, configuration options, or any data where you want to look up a value by a key.

Maps are written using square brackets `[]` with key -> value pairs, separated by commas:

```
["name" -> "Alice", "score" -> 100, 1 -> {"a", "b"}]
['level -> 5, #123 -> "object ref"]
```

You can use any MOO value as a key, but it's most common to use strings, symbols, or numbers. To get a value from a map,
use the key in square brackets:

```
player = ["name" -> "Alice", "score" -> 100];
score = player["score"]; // 100
```

Maps are also immutable—modifying them creates a new map.

> **Syntax Note:**
>
> In MOO, lists use curly braces `{}` and maps use square brackets `[]`. This is the *opposite* of Python and
> JavaScript, where lists/arrays use `[]` and dictionaries/objects use `{}`. MOO's syntax came first, and this
> historical
> quirk can be confusing if you're used to other languages!

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
return my_error; // We can return errors from verbs to let callers know something went wrong.
```

And here is an example of a fully custom error:

```moo
return E_TOOFAST("The car is going way too fast");
```

## Object Type

_Object references_ are how you refer to the permanent objects stored in the MOO database. The value itself is not the
object—it's more like an address or pointer that tells MOO which object you're talking about.

Every object in the database has a unique identifier. When you store an object reference in a variable or property,
you're storing a reference that points to that specific object.

### Three Types of Object References

mooR supports three different kinds of object references:

**Traditional Object Numbers:**

```
#495
#123
#0
```

**UUID Object Names:**

```
#048D05-1234567890
#1A2B3C-4567891234
```

**Anonymous Object References:**
Anonymous objects cannot be typed directly—you can only get references to them from `create()` calls:

```
let anon_obj = create($thing, player, 1);  // Creates anonymous object
// anon_obj now contains an anonymous object reference
// No way to type this directly in code!
```

Numbered and UUID objects work exactly the same way in your code—you can call verbs on them, access properties, and use
them anywhere you'd use any object reference. Anonymous objects also work the same way, but they have no literal form
you can type directly.

### Important notes about object references:

- Traditional object numbers like `#495` refer to object 495 in the sequential numbering system
- UUID object names like `#048D05-1234567890` refer to objects in the UUID naming system
- Anonymous object references have no literal form—you get them from `create()` and they disappear when garbage
  collected
- If a numbered or UUID object gets recycled (deleted), any references to it become invalid
- Anonymous objects are automatically deleted when no references to them exist (garbage collection)
- You can pass object references around, store them in lists, use them as map keys, etc.
- When you call verbs or access properties, you use the object reference: `#495:tell("Hello!")`,
  `#048D05-1234567890:tell("Hello!")`, or `anon_obj:tell("Hello!")`
- Traditional object numbers can be negative for special purposes, but UUID and anonymous objects never are
- Use `is_anonymous(obj)` to detect if an object reference is anonymous (all types have `typeof(obj) == OBJ`)

### Special & negative traditional object numbers:

There are three special traditional object numbers used for specific purposes: `#-1`, `#-2`, and `#-3`, usually referred
to in the LambdaCore database as `$nothing`, `$ambiguous_match`, and `$failed_match`, respectively.

Negative traditional object numbers never refer to an actual physical object in the world, but always to some concept (
e.g. #-1 for nothing) or something external (player connections are given special negative numbers).

Note that UUID object names and anonymous objects don't have negative equivalents—these special cases only apply to the
traditional numbering system.

### Best practices:

Instead of hard-coding object references like `#495` or `#048D05-1234567890` in your code, it's better to use system
references like `$my_special_object` (See below). This makes your code more readable and less fragile if object
identifiers change. Anonymous objects cannot be hard-coded since they have no literal form.

> **Note:** Hard-coding object references in your code should be discouraged. An object only exists until it is
> recycled (or garbage collected for anonymous objects), and it's technically possible for an object identifier to
> change
> under some circumstances. Thus, you should use a system reference to an object (`$my_special_object`) instead. More on
> system references later.
>
> **Object Types:** The choice between traditional numbers, UUID names, and anonymous objects depends on your server's
> configuration and the specific use case:
> - **Numbered objects**: World infrastructure, permanent fixtures
> - **UUID objects**: Dynamic content that needs unique but persistent IDs
> - **Anonymous objects**: Temporary objects that clean up automatically

## System References ($names)

In MOO, you'll often see identifiers that start with a dollar sign, like `$room`, `$thing`, or `$player`. These are
called _system references_ (sometimes called "corified" references), and they're a convenient way to refer to important
objects and values without having to remember their object numbers.

### How system references work:

A system reference like `$thing` is actually shorthand for `#0.thing` - it's a property stored on object `#0`, which is
called the "system object."

```moo
$room     // This is the same as #0.room
$player   // This is the same as #0.player  
$thing    // This is the same as #0.thing
```

### Why use system references?

**They make code readable and maintainable:**

- `$room` is much clearer than `#17` in your code
- If the room object gets a new number, you only need to update `#0.room`
- Other programmers can understand what `$player` means immediately

**They can store any value, not just object numbers:**

- `$maxint` might store the integer `9223372036854775807`
- `$default_timeout` might store `30` (seconds)
- `$server_name` might store the string `"My MOO Server"`

### The system object (#0):

Object `#0` is special in MOO - it's called the "system object" and serves as the central place to store important
system-wide values and references. Think of it as the "control panel" for your MOO:

- It holds properties that define important objects like `$room`, `$thing`, `$player`
- It stores system configuration values like `$maxint`, `$minint`
- It's where you put values that all verbs need to access
- It's always object number `0` and can't be recycled

### Common system references:

You'll encounter these frequently in MOO code:

- `$room` - The generic room object that other rooms inherit from
- `$thing` - The generic thing object for items and objects
- `$player` - The generic player object
- `$nothing` - Represents "no object" (usually `#-1`)
- `$ambiguous_match` - Used when parsing finds multiple matches (usually `#-2`)
- `$failed_match` - Used when parsing finds no matches (usually `#-3`)

### Creating your own system references:

You can create your own system references by adding properties to `#0`, but this requires wizard permissions and using
the proper commands:

```moo
// First, add the property (requires wizard permissions):
add_property(#0, "my_special_room", #1234, {player, "r"});

// Or using a core command like @property:
@property #0.my_special_room #1234

// Now you can use $my_special_room instead of #1234
```

> **Note:** Only wizards can add properties to the system object (`#0`). Most MOO cores provide utility commands like
`@property` to make this easier than using the `add_property()` builtin directly.

This is much better than hard-coding object numbers throughout your code!

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

- You can't slice symbols like `'hello[1..3]`
- You can't index into them like `'test[2]`
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
string (a way of writing binary data using letters and numbers) and ending with `"`, like this:

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

> **Technical Notes:**
>
> mooR uses URL-safe base64 encoding by default for binary literals. This means the encoding uses `-` and `_` instead of
`+` and `/`, making binary values safe to use in URLs and web applications.
>
> **LambdaMOO Compatibility:** LambdaMOO had its own custom way of encoding binary strings that mooR does not currently
> support. If you're migrating code from LambdaMOO that uses binary data, you may need to convert the encoding format.

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
< $generic_item, [name -> "magic sword", power -> 15] >

// With contents (like inventory):
< $container, [name -> "treasure chest"], {"gold coins", "ruby", "scroll"} >

// Complex example - a room in a maze:
< $maze_room, 
  [description -> "A twisty passage", exits -> {"north", "south"}],
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
sword = < $weapon, [damage -> 10, name -> "iron sword"] >;

// Call a verb - this will look for "wield" on $weapon:
sword:wield(player);
```

### Accessing flyweight data:

You can read the slots (properties) of a flyweight:

```moo
sword = < $weapon, [damage -> 10, name -> "iron sword"] >;
damage_value = sword.damage;    // Gets 10
weapon_name = sword.name;       // Gets "iron sword"
```

### Working with XML and web interfaces:

Flyweights are especially useful for building web pages because they can be easily converted to and from XML. However,
mooR also supports working with XML using regular lists and maps, which can be more convenient for simple cases:

```moo
// A flyweight representing HTML structure:
div_element = < $html_div, 
               [class -> "player-info"], 
               {"Player: Alice", "Score: 1500"} >;

// Convert to XML string:
html_string = to_xml(div_element);

// Alternative: Use list format (works without flyweights enabled)
list_element = {"div", {"class", "player-info"}, "Player: Alice", "Score: 1500"};
html_string = to_xml(list_element);

// Parse XML into different formats:
xml_as_flyweights = xml_parse(html_string, 15);  // Returns flyweights
xml_as_lists = xml_parse(html_string, 4);        // Returns nested lists
xml_as_maps = xml_parse(html_string, 10);        // Returns structured maps
```

**List format** uses the pattern: `{"tag_name", {"attr", "value"}, ...content...}`

**Map format** uses: `["tag" -> "tag_name", "attributes" -> ["attr" -> "value"], "content" -> {...}]`

Both list and map formats work regardless of whether flyweights are enabled, making them useful for systems that prefer
simpler data structures.

### Important notes:

- Flyweights cannot be changed once created - they're immutable
- They only exist while your program is running
- They're perfect for temporary data structures
- The `player` variable can never be a flyweight (but `this` and `caller` can be)

## Function Type

_Functions_ in mooR let you create small pieces of reusable code within your verbs. You can store them in variables,
pass them around like any other value, and they can even "remember" variables from where they were created.

> **Fun Fact**: Despite its name suggesting otherwise, the original LambdaMOO never actually had lambda functions! mooR
> brings this useful programming tool to MOO as part of our mission of dragging the future into the past.

### Function syntax:

Functions can be written in several forms:

**Arrow syntax** (for simple expressions):

```moo
{x, y} => x + y                    // Takes two parameters, returns their sum
{name} => "Hello, " + name         // Simple string formatting
{} => random(100)                  // No parameters, returns random number
```

**Function syntax** (for complex logic and code organization):

```moo
// Anonymous function
fn(x, y)
    if (x > y)
        return x;
    else
        return y;
    endif
endfn

// Named function (like def in Python)
fn calculate_distance(x1, y1, x2, y2)
    let dx = x2 - x1;
    let dy = y2 - y1;
    return sqrt(dx * dx + dy * dy);
endfn
```

**Named recursive functions**:

```moo
fn factorial(n)
    if (n <= 1)
        return 1;
    else
        return n * factorial(n - 1);
    endif
endfn
```

### Parameter types:

Lambdas support the same parameter patterns as regular MOO verbs:

```moo
{x, y} => x + y                    // Required parameters
{x, ?y} => x + (y || 0)           // Optional parameters (default to 0 if not provided)
{x, @rest} => x + length(rest)     // Rest parameters (collect extra args into a list)
{a, ?b, @c} => {a, b, c}          // Mix of all types
```

### Code organization with named functions:

Named functions help break down complex verb logic, much like functions in other programming languages:

```moo
// In a game combat verb:
fn calculate_hit_chance(attacker, defender)
    let base_chance = 0.5;
    let skill_bonus = attacker.skill / 100.0;
    return base_chance + skill_bonus;
endfn

fn apply_damage(target, damage)
    target.health = target.health - damage;
    notify(target, "You take " + damage + " damage!");
endfn

// Main logic becomes much cleaner:
if (random() < calculate_hit_chance(this, target))
    apply_damage(target, 10);
endif
```

### Closures - capturing variables:

Functions can "capture" variables from their surrounding environment:

```moo
let multiplier = 5;
let multiply_by_five = {x} => x * multiplier;  // Captures 'multiplier'
return multiply_by_five(10);  // Returns 50
```

Even when the original scope ends, the lambda remembers the captured values:

```moo
fn make_counter()
    let count = 0;
    return {} => count = count + 1;  // Captures 'count'
endfn

let counter = make_counter();
counter();  // Returns 1
counter();  // Returns 2
counter();  // Returns 3
```

### Calling functions:

Once you have a function, call it like any other function:

```moo
let add = {x, y} => x + y;
result = add(5, 3);  // Returns 8

// You can also call them immediately:
result = ({x} => x * 2)(5);  // Returns 10
```

### Higher-order functions:

Functions are perfect for creating functions that work with other functions:

```moo
fn map(func, list)
    let result = {};
    for item in (list)
        result = {@result, func(item)};
    endfor
    return result;
endfn

let numbers = {1, 2, 3, 4};
let doubled = map({x} => x * 2, numbers);  // {2, 4, 6, 8}
let squared = map({x} => x * x, numbers);  // {1, 4, 9, 16}
```

### When should you use functions?

**Great for named functions:**

- Breaking down complex verb logic into manageable pieces
- Code organization and avoiding duplication within verbs
- Mathematical calculations and data processing
- Any logic that benefits from a descriptive name

**Great for anonymous functions (lambdas):**

- Event handlers and callbacks
- Data transformation and filtering (map, filter, etc.)
- Creating functions that create other functions
- Short, one-off operations

**Better to use regular verbs for:**

- Functions called from multiple different verbs
- Complex business logic that forms your application's core
- Code that needs to be accessible via inheritance
- Functions that need persistent storage

### Technical notes:

- Functions are immutable values like strings and lists
- They capture variables by value, not by reference
- Named recursive functions can call themselves using their declared name
- Functions work with the scatter assignment syntax for flexible parameter handling
- Stack traces show function calls as `verb.<fn>` or `verb.function_name` for named functions

For comprehensive examples and advanced usage, see
the [Functions and Lambdas](../the-moo-programming-language/lambda-functions.md) chapter.

