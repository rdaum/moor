## Extensions

`moor` adds the following notable extensions to the LambdaMOO 1.8.x language and runtime. Note that this list
isn't fully complete as some features are still in development, and some are not so easily categorized
as extensions and more "architectural differences" which will be described elsewhere.

### Lexical variable scoping

Adds block-level lexical scoping to the LambdaMOO language.

Enabled by default, can be disabled with command line option `--lexical-scopes=false`

In LambdaMOO all variables are global to the verb scope, and are bound at the first assignment.
`moor` adds optional lexical scoping, where variables are bound to the scope in which they are declared,
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

In LambdaMOO, verbs can only be called on objects. `moor` adds the ability to indirectly dispatch verbs for primitive
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

`moor` adds a dictionary or map type mostly equivalent to the one present in `stunt`/`toaststunt`.

Enabled by default, can be disabled with command line option `--map-type=false`

This is a set of immutable sorted key-value pairs with convenient syntax for creating and accessing them and
can be used as an alternative to traditional MOO `alists` / "associative lists".

```moo
let my_map = [ "a" => 1, "b" => 2, "c" => 3 ];
my_map["a"] => 1
```

Wherever possible, the Map type semantics are made to be compatible with those from `stunt`, so
most existing code from those servers should work with `moor` with no changes.

Values are stored sorted, and lookup uses a binary search. The map is immutable, and modification is done through
copy and update like other MOO primitive types.

Performance wise, construction and lookup are O(log n) operations, and iteration is O(n).

### "Rich" output via `notify`

The `notify` builtin in MOO is used to output a line of text over the telnet connection to the player.

In `moor`, the connection may not be `telnet`, and the output may be more than just text.

Enabled by default, can be disabled with command line option `--rich-notify=false`

Moor ships with a `web-host` that can serve HTTP and websocket connections, and the `notify` verb can be used to
output JSON values over the websocket connection.

When this feature is enabled, the second (value) argument to `notify` can be any MOO value, and it will be
serialized to JSON and sent to the client.

If a third argument is present, it is expected to be a "content-type" string, and will be places in the websocket
JSON message as such.

```moo
notify(player, [ "type" => "message", "text" => "Hello, world!" ], "application/json");
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
