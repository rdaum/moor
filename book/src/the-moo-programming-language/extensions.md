## Extensions

`mooR` adds the following notable extensions to the LambdaMOO 1.8.x language and runtime. Note that this list
isn't fully complete as some features are still in development, and some are not so easily categorized
as extensions and more "architectural differences" which will be described elsewhere.

### Lexical variable scoping

Adds block-level lexical scoping to the LambdaMOO language.

Enabled by default, can be disabled with command line option `--lexical-scopes=false`

In LambdaMOO all variables are global to the verb scope, and are bound at the first assignment.
`mooR` adds optional lexical scoping, where variables are bound to the scope in which they are declared,
and leave the scope when it is exited.
This is done by using the `let` keyword to declare variables that will exist only in the current scope.

```moo
while (y < 10)
  let x = 1;
  ...
  if (x == 1000)
    return x;
  endif
endwhile
```

The block scopes for lexical variables apply to `for`, `while`, `if`/`elseif`, `try`/`except`
... and a new ad-hoc block type declared with `begin` / `end`:

```moo
begin
  let x = 1;
  ...
end
```

Where a variable is declared in a block scope, it is not visible outside of that block.
Global variables are still available, and can be accessed from within a block scope.
In the future a `strict` mode may be added to require all variables to be declared lexically before use,
but this would break compatibility with existing code.

### Primitive type dispatching

Adds the ability to call verbs on primitive types, such as numbers and strings.

Enabled by default, can be disabled with command line option `--type-dispatch=false`

In LambdaMOO, verbs can only be called on objects. `mooR` adds the ability to indirectly dispatch verbs for primitive
types by interpreting the verb name as a verb on a special object that represents the primitive type and passing
the value itself as the first argument to the verb. Where other arguments are passed, they are appended to the argument
list.

```moo
"hello":length() => 5
```

is equivalent to

```moo
$string:length("hello") => 5
```

The names of the type objects to the system objects `$string`, `$float`, `$integer`, `$list`, `$map`, and `$error`

### Map type

`mooR` adds a dictionary or map type mostly equivalent to the one present in `stunt`/`toaststunt`.

This is a set of immutable sorted key-value pairs with convenient syntax for creating and accessing them and
can be used as an alternative to traditional MOO `alists` / "associative lists".

```moo
let my_map = [ "a" -> 1, "b" -> 2, "c" -> 3 ];
my_map["a"] => 1
```

Wherever possible, the Map type semantics are made to be compatible with those from `stunt`, so
most existing code from those servers should work with `mooR` with no changes.

Values are stored sorted, and lookup uses a binary search. The map is immutable, and modification is done through
copy and update like other MOO primitive types.

Performance wise, construction and lookup are O(log n) operations, and iteration is O(n).

### Custom errors and errors with attached messages

LambdaMOO had a set of hardcoded builtin-in errors with numeric values. It also uses the same errors for exceptions,
and allows optional messages and an optional value to be passed when raising them, but these are not part of the error
value itself.

`mooR` adds support for custom errors, where any value that comes after `E_` is treated as an error identifier, though
it does not have a numeric value (and will raise error if you try to call `tonum()` on it).

Additionally, `mooR` extends errors with an optional message component which can be used to provide more context:

```moo
E_PROPNF("$foo.bar does not exist")
```

This is useful for debugging and error handling, as it allows you to provide more information about the error.

Most of the builtin functions and builtin types have been updated to produce more descriptive error messages
when they fail. This can help greatly with debugging and understanding what went wrong in your code.

### Symbol type

`mooR` adds a symbol type which represents interned strings similar to those found in Lisp, Scheme and other languages.

Enabled by default, can be disabled with command line option `--symbol-type=false`

Symbols are immutable string values that are guaranteed to be unique and can be compared by reference rather than value.
This makes them useful for keys in maps and other data structures where fast comparison is important.

The syntax for creating a symbol is the same as for strings, but with a leading apostrophe:

`'hello` is a symbol, while `"hello"` is a string.

Symbols are useful for representing identifiers, keywords, and other values that are not meant to be modified, and
they can be used in place of strings as "keys" in maps and other data structures.

### For comprehensions

`mooR` adds a syntax similar to Python or Julia for list comprehensions, allowing you to create lists from existing
lists
or ranges in a more concise way:

```moo
{ x * 2 for x in ({1, 2, 3, 4}) };
=> {2, 4, 6, 8}
```

or

```moo
{ x * 2 for x in [1..10] };
=> {2, 4, 6, 8, 10, 12, 14, 16, 18, 20}
```

### Return as expression rather than a statement

`mooR` allows the `return` statement to be used as an expression, allowing for "short circuit" returns within chains
of expression, in a manner similar to Julia.

This can be useful for forcing immediate returns from a verb without having to use `if` statements.

```moo
this.colour != "red" && return true;
this.colour || return false;
```

### Flyweight objects

`mooR` adds a new type of object called a "flyweight" which is a lightweight object that can be used to represent
data structures without the overhead of a full object. Flyweights are immutable and can be used to represent
complex data structures like trees or graphs without the overhead of creating a full object for each node.

Flyweights have three components only one of this is mandatory:

- A delegate (like a parent)
- A set of slots (like properties)
- Contents (a list)

Which is expressed in the following literal syntax:

```moo
< delegate, [ slot -> value, ... ], { contents } >
```

Examples:

```moo
< $key, [ password -> "secret" ], { 1, 2, 3 } >
< $exit, [ name -> "door", locked -> 1, description -> "..." ] >
< $password, [ crypted -> "fsadfdsa", salt -> "sdfasfd" ]>
```

When accessing a property (or slot) on a flyweight using property accessing syntax, the system will first check the
flyweight itself, and then check the delegate object. If the property is not found on either, it will return `E_PROPNF`:

```moo
let x = < $key, [ password -> "secret" ] >;
return x.password;

=> "secret"
```

Verbs cannot be defined on a flyweight, but calling a verb on one will attempt to call it on the delegate:

```moo
let x = < $key, [ password -> "secret" >;
x:unlock("magic_key");
```

Will call `$key:unlock` with `this` being the flyweight object, and the first argument being the string "magic_key".

### "Rich" output via `notify`

The `notify` builtin in MOO is used to output a line of text over the telnet connection to the player.

In `mooR`, the connection may not be `telnet`, and the output may be more than just text.

Enabled by default, can be disabled with command line option `--rich-notify=false`

mooR ships with a `web-host` that can serve HTTP and websocket connections, and the `notify` verb can be used to
output JSON values over the websocket connection.

When this feature is enabled, the second (value) argument to `notify` can be any MOO value, and it will be
serialized to JSON and sent to the client.

If a third argument is present, it is expected to be a "content-type" string, and will be places in the websocket
JSON message as such.

```moo
notify(player, [ "type" -> "message", "text" -> "Hello, world!" ], "application/json");
```

becomes, on the websocket:

```json
{
  "content-type": "application/json",
  "message": {
    "map_pairs": [
      [
        "type",
        "message"
      ],
      [
        "text",
        "Hello, world!"
      ]
    ]
  },
  "system-time": 1234567890
}
```

The `telnet` host will still output the text value of the second argument as before, and ignore anything which
is not a string.

### XML Document Processing

`mooR` can generate XML and HTML documents from MOO data structures, and parse XML or well-formed HTML document strings
into structured data.

The `xml_parse` builtin can produce XML data in three different formats:

**Flyweight format (original):** `xml_parse(xml_string, FLYWEIGHT, [tag_map])`

```moo
// Returns flyweight objects representing XML structure
let result = xml_parse("<div class='test'>Hello</div>", 15);
```

**List format:** `xml_parse(xml_string, 4)`

```moo
// Returns nested lists: {"tag", {"attr", "value"}, ...contents...}
let result = xml_parse("<div class='test'>Hello</div>", LIST);
// result = {{"div", {"class", "test"}, "Hello"}}
```

**Attributes-as-Map format:** `xml_parse(xml_string, 10)`

```moo
// Returns list of maps with structured data
let result = xml_parse("<div class='test'>Hello</div>", MAP);
// result = {["tag" -> "div", "attributes" -> ["class" -> "test"], "content" -> {"Hello"}]}
```

The `to_xml` builtin can generate XML from both flyweights and list formats:

```moo
// Generate XML from list format
let html_structure = {"div", {"class", "container"}, "Hello World"};
let xml_string = to_xml(html_structure);
// Returns: "<div class='container'>Hello World</div>"
```

This makes it easy to work with XML data in web applications and API integrations without requiring flyweight objects.

### Lambda functions

`mooR` adds support for creating small functions within your verbs, similar to functions in other programming languages.

Enabled by default, this feature lets you create both named functions (for organizing code) and anonymous functions (called "lambdas") that can remember variables from where they were created.

**Arrow syntax for simple expressions:**
```moo
let add = {x, y} => x + y;
let greet = {name} => "Hello, " + name;
```

**Function syntax for complex logic:**
```moo
let max = fn(x, y)
    if (x > y)
        return x;
    else
        return y;
    endif
endfn;
```

**Named recursive functions:**
```moo
fn factorial(n)
    if (n <= 1)
        return 1;
    else
        return n * factorial(n - 1);
    endif
endfn
```

**Closures with variable capture:**
```moo
let multiplier = 5;
let multiply_by_five = {x} => x * multiplier;  // Captures 'multiplier'
return multiply_by_five(10);  // Returns 50
```

Functions support all MOO parameter patterns including optional parameters (`?param`) and rest parameters (`@args`). They can be called like regular functions and are particularly useful for organizing code, event handling, and data processing.

> **Historical Note**: Despite its name suggesting otherwise, the original LambdaMOO never actually had lambda functions! mooR brings this useful programming tool to MOO as part of our mission of dragging the future into the past.
