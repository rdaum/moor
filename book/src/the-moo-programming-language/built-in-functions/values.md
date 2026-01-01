## Type Information Functions

### `typeof`

**Description**:   Returns the type code of a value.
**Arguments**:

- `value`: The value to get the type of

**Returns:** An integer representing the type code of the value

### `length`

**Description**:   Returns the length of a sequence (string, list, map).
**Arguments**:

- `sequence`: The sequence to measure

**Returns:** An integer representing the length of the sequence  
**Note:** Will raise an error if the value is not a sequence type.

## Type Conversion Functions

### `tostr`

**Description**: Converts value(s) to a string representation.  
**Arguments**:

- `value1, value2, ...`: One or more values to convert to string

**Returns:** A string representation of the concatenated values  
**Note:** If multiple arguments are provided, they are concatenated together.

### `tosym`

**Description**: Converts a scalar value to a symbol.  
**Arguments**:

- `value`: The value to convert (must be a string, boolean, error, or symbol)

**Returns:** A symbol representing the value  
**Note:** Will raise E_TYPE if the value cannot be converted to a symbol.

### `toliteral`

**Description**: Converts a value to its literal string representation.  
**Arguments**:

- `value`: The value to convert

**Returns:** A string containing the literal representation of the value  
**Note:** This produces a string that could be evaluated to recreate the original value.

### `toint`

**Description**: Converts a value to an integer.  
**Arguments**:

- `value`: The value to convert (must be a number, object, string, or error)

**Returns:** The integer representation of the value  
**Note:** String conversion parses the string as a number; invalid strings convert to 0. Boolean values convert to 1 for `true` and 0 for `false`.

### `tonum`

Alias for `toint`. **Description:**

### `toobj`

**Description**: Converts a value to an object reference.  
**Arguments**:

- `value`: The value to convert (must be a number, string, or object)

**Returns:** An object reference  
**Note:** For strings, accepts formats like "123" or "#123". Invalid strings convert to object #0.

### `tofloat`

**Description**: Converts a value to a floating-point number.  
**Arguments**:

- `value`: The value to convert (must be a number, string, or error)

**Returns:** The floating-point representation of the value  
**Note:** String conversion parses the string as a number; invalid strings convert to 0.0.

### `tobool`

**Description**: Converts a value to a boolean based on mooR's truthiness rules (empty strings/lists/maps/binaries and the number 0 evaluate to `false`, everything else to `true`).  
**Arguments**:

- `value`: The value to evaluate

**Returns:** `true` if the value is truthy, otherwise `false`  
**Note:** This matches the boolean tests used by `if`, `while`, and other control-flow constructs.

## Comparison Functions

### `equal`

**Description**: Performs a case-sensitive equality comparison between two values.  
**Arguments**:

- `value1`: First value to compare
- `value2`: Second value to compare

**Returns:** Boolean result of the comparison (true if equal, false otherwise)

## JSON Conversion Functions

### `generate_json`

```
str generate_json(value [, str mode])
```

Returns the JSON representation of the MOO value.

MOO supports a richer set of values than JSON allows. The optional mode specifies how this function handles the
conversion of MOO values into their JSON representation.

The common subset mode, specified by the literal mode string "common-subset", is the default conversion mode. In this
mode, only the common subset of types (strings and numbers) are translated with fidelity between MOO types and JSON
types. All other types are treated as alternative representations of the string type. This mode is useful for
integration with non-MOO applications.

The embedded types mode, specified by the literal mode string "embedded-types", adds type information. Specifically,
values other than strings and numbers, which carry implicit type information, are converted into strings with type
information appended. The converted string consists of the string representation of the value (as if tostr() were
applied) followed by the pipe (|) character and the type. This mode is useful for serializing/deserializing objects and
collections of MOO values.

```
generate_json([])                                           =>  "{}"
generate_json(["foo" -> "bar"])                             =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "common-subset")            =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "embedded-types")           =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> 1.1])                               =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "common-subset")              =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "embedded-types")             =>  "{\"foo\":1.1}"
generate_json(["foo" -> #1])                                =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "common-subset")               =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "embedded-types")              =>  "{\"foo\":\"#1|obj\"}"
generate_json(["foo" -> E_PERM])                            =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "common-subset")           =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "embedded-types")          =>  "{\"foo\":\"E_PERM|err\"}"
```

JSON keys must be strings, so regardless of the mode, the key will be converted to a string value.

```
generate_json([1 -> 2])                                     =>  "{\"1\":2}"
generate_json([1 -> 2], "common-subset")                    =>  "{\"1\":2}"
generate_json([1 -> 2], "embedded-types")                   =>  "{\"1|int\":2}"
generate_json([#1 -> 2], "embedded-types")                  =>  "{\"#1|obj\":2}"
```

> Warning: generate_json does not support WAIF or ANON types.

### `parse_json`

```
value parse_json(str json [, str mode])
```

Returns the MOO value representation of the JSON string.

If the specified string is not valid JSON, E_INVARG is raised.

The optional mode specifies how this function handles conversion of MOO values into their JSON representation. The
options are the same as for generate_json().

```
parse_json("{}")                                            =>  []
parse_json("{\"foo\":\"bar\"}")                             =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "common-subset")            =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "embedded-types")           =>  ["foo" -> "bar"]
parse_json("{\"foo\":1.1}")                                 =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "common-subset")                =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "embedded-types")               =>  ["foo" -> 1.1]
parse_json("{\"foo\":\"#1\"}")                              =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1\"}", "common-subset")             =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1|obj\"}", "embedded-types")        =>  ["foo" -> #1]
parse_json("{\"foo\":\"E_PERM\"}")                          =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM\"}", "common-subset")         =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM|err\"}", "embedded-types")    =>  ["foo" -> E_PERM]
```

In embedded types mode, key values can be converted to MOO types by appending type information. The full set of
supported types are obj, str, err, float and int.

```
parse_json("{\"1\":2}")                                     =>   ["1" -> 2]
parse_json("{\"1\":2}", "common-subset")                    =>   ["1" -> 2]
parse_json("{\"1|int\":2}", "embedded-types")               =>   [1 -> 2]
parse_json("{\"#1|obj\":2}", "embedded-types")              =>   [#1 -> 2]
```

> Note: JSON converts `null` to the string "null".

## Memory Functions

### `value_bytes`

**Description**: Returns the size of a value in bytes.  
**Arguments**:

- `value`: The value to measure

**Returns:** The size of the value in bytes

### `object_bytes`

**Description**: Returns the size of an object in bytes.  
**Arguments**:

- `object`: The object to measure

**Returns:** The size of the object in bytes  
**Note:** This includes all properties, verbs, and other object data.
Note: Most of these functions follow a consistent pattern of validating arguments and providing appropriate error
handling. Type conversion functions generally attempt to convert intelligently between types and provide sensible
defaults or errors when conversion isn't possible.

## Error Handling Functions

### `error_message`

**Description**: Returns the error message associated with an error value.  
**Arguments**:

- `error`: The error value to get the message from

**Returns:** The error message string

### `error_code`

**Description**: Strips off the message from an error value and returns just the error without it.  
**Arguments**:

- `error`: The error value to get the code from

**Returns:** The error code of the error value
