## Type Information Functions

### `typeof`

Returns the type code of a value.
**Description:****Arguments:**

- `value`: The value to get the type of

**Returns:** An integer representing the type code of the value

### `length`

Returns the length of a sequence (string, list, map).
**Description:****Arguments:**

- `sequence`: The sequence to measure

**Returns:** An integer representing the length of the sequence
**Note:** Will raise an error if the value is not a sequence type.

## Type Conversion Functions

### `tostr`

Converts value(s) to a string representation.
**Description:****Arguments:**

- `value1, value2, ...`: One or more values to convert to string

**Returns:** A string representation of the concatenated values
**Note:** If multiple arguments are provided, they are concatenated together.

### `tosym`

Converts a scalar value to a symbol.
**Description:****Arguments:**

- `value`: The value to convert (must be a string, boolean, error, or symbol)

**Returns:** A symbol representing the value
**Note:** Will raise E_TYPE if the value cannot be converted to a symbol.

### `toliteral`

Converts a value to its literal string representation.
**Description:****Arguments:**

- `value`: The value to convert

**Returns:** A string containing the literal representation of the value
**Note:** This produces a string that could be evaluated to recreate the original value.

### `toint`

Converts a value to an integer.
**Description:****Arguments:**

- `value`: The value to convert (must be a number, object, string, or error)

**Returns:** The integer representation of the value
**Note:** String conversion parses the string as a number; invalid strings convert to 0.

### `tonum`

Alias for `toint`. **Description:**

### `toobj`

Converts a value to an object reference.
**Description:****Arguments:**

- `value`: The value to convert (must be a number, string, or object)

**Returns:** An object reference
**Note:** For strings, accepts formats like "123" or "#123". Invalid strings convert to object #0.

### `tofloat`

Converts a value to a floating-point number.
**Description:****Arguments:**

- `value`: The value to convert (must be a number, string, or error)

**Returns:** The floating-point representation of the value
**Note:** String conversion parses the string as a number; invalid strings convert to 0.0.

## Comparison Functions

### `equal`

Performs a case-sensitive equality comparison between two values.
**Description:****Arguments:**

- `value1`: First value to compare
- `value2`: Second value to compare

**Returns:** Boolean result of the comparison (true if equal, false otherwise)

## Memory and Hashing Functions

### `value_bytes`

Returns the size of a value in bytes.
**Description:****Arguments:**

- `value`: The value to measure

**Returns:** The size of the value in bytes

### `object_bytes`

Returns the size of an object in bytes.
**Description:****Arguments:**

- `object`: The object to measure

**Returns:** The size of the object in bytes
**Note:** This includes all properties, verbs, and other object data.

### `value_hash`

Computes an MD5 hash of a value's literal representation.
**Description:****Arguments:**

- `value`: The value to hash

**Returns:** An uppercase hexadecimal string representing the MD5 hash
Note: Most of these functions follow a consistent pattern of validating arguments and providing appropriate error
handling. Type conversion functions generally attempt to convert intelligently between types and provide sensible
defaults or errors when conversion isn't possible.

## Error Handling Functions

### `error_message`

Returns the error message associated with an error value.

**Description:****Arguments:**

- `error`: The error value to get the message from

**Returns:** The error message string

### `error_code`

Strips off the message from an error value and returns just the error without it.

**Description:****Arguments:**

- `error`: The error value to get the code from

**Returns:** The error code of the error value
