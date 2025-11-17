## Verb Information Functions

### `verb_info`

**Description:** Retrieves basic information about a verb on an object.  
**Arguments:**

- : The object that has the verb `object`
- : Either the verb name or a positive integer representing the verb's position in the verb list (1-based) `verb-desc`

**Returns:** A list containing three elements:

1. The owner of the verb (object reference)
2. A string representing the permission flags: 'r' (read), 'w' (write), 'x' (execute), 'd' (debug)
3. A string containing the verb names (aliases) separated by spaces

**Note:** Requires read permission on the verb.

### `set_verb_info`

**Description:** Changes the permission information for a verb.  
**Arguments:**

- : The object with the verb to modify `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`
- : A list containing permission information: `[owner, permissions, names]`
    - : The new owner of the verb (object reference) `owner`
    - : A string containing the permission flags (combination of 'r', 'w', 'x', 'd') `permissions`
    - : A string containing space-separated verb names/aliases `names`

`info`

**Returns:** `none`  
**Note:** Requires appropriate permissions to modify the verb.

## Verb Arguments Functions

### `verb_args`

**Description:** Retrieves information about a verb's argument specification.  
**Arguments:**

- : The object that has the verb `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`

**Returns:** A list containing three elements:

1. The direct object specification (e.g., "this", "none", "any")
2. The preposition (e.g., "with", "at", "in front of")
3. The indirect object specification (e.g., "this", "none", "any")

**Note:** Requires read permission on the verb.

### `set_verb_args`

**Description:** Changes the argument specification for a verb.  
**Arguments:**

- : The object with the verb to modify `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`
- : A list containing argument specifications: `[dobj, prep, iobj]`
    - : String specifying direct object behavior `dobj`
    - : String specifying the preposition `prep`
    - : String specifying indirect object behavior `iobj`

`args`

**Returns:** `none`

**Note:** Requires appropriate permissions to modify the verb.

## Verb Code Functions

### `verb_code`

**Description:** Retrieves the source code of a verb.  
**Arguments:**

- : The object that has the verb `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`
- : Optional boolean indicating whether to fully parenthesize the code (default: false) `fully-paren`
- `indent`: Optional integer specifying indentation amount (default: 0)

**Returns:** A list of strings, each representing a line of the verb's source code  
**Note:** Requires read permission on the verb and programmer bit.

### `set_verb_code`

**Description:** Changes the source code of a verb.
**Arguments:**

- `object`: The object with the verb to modify
- `verb-desc`: Either the verb name or a positive integer representing the verb's position (1-based)
- `code`: A list of strings, each representing a line of the verb's source code
- `verbosity`: (Optional) Controls error output detail level (default: 2)
    - `0` - Summary: Brief error message only
    - `1` - Context: Message with error location (graphical display when output_mode > 0)
    - `2` - Detailed: Message, location, and diagnostic hints (default)
    - `3` - Structured: Returns error data as a map for programmatic handling
- `output_mode`: (Optional) Controls error formatting style (default: 0)
    - `0` - Plain text without special characters
    - `1` - Graphics characters for visual clarity
    - `2` - Graphics with ANSI color codes

**Returns:**

- On success: empty list `{}`
- On compilation failure with `verbosity` 0-2: list of formatted error strings
- On compilation failure with `verbosity` 3: map containing structured error data (use `format_compile_error()` to
  format)

**Note:** Requires appropriate permissions to modify the verb and programmer bit.

**Examples:**

```moo
// Basic usage with default detailed errors
set_verb_code(#123, "test", {"return 1 + ;"});
=> {"Parse error at line 1, column 12:", "  return 1 + ;", "             ⚠", ...}

// Get structured error data for custom handling
err = set_verb_code(#123, "test", {"return 1 + ;"}, 3);
=> [type -> "parse", message -> "unexpected ';'", line -> 1, column -> 12, ...]

// Format the structured error with custom verbosity
formatted = format_compile_error(err, 0);  // Summary only
=> {"Parse error at line 1, column 12: unexpected ';'"}
```

## Verb Management Functions

### `add_verb`

**Description:** Adds a new verb to an object.  
**Arguments:**

- : The object to add the verb to `object`
- : A list containing permission information (same format as in ) `info`set_verb_info``
- : A list containing argument specifications (same format as in ) `args`set_verb_args``

**Returns:** `none`  
**Note:** Requires appropriate permissions to add verbs to the object and programmer bit.

### `delete_verb`

**Description:** Removes a verb from an object.  
**Arguments:**

- : The object to remove the verb from `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`

**Returns:** `none`  
**Note:** Requires ownership of the verb or the object and programmer bit.

## Error Formatting Functions

### `format_compile_error`

**Description:** Formats a structured compilation error map into human-readable text.
**Arguments:**

- `error`: Map containing compilation error data (from `set_verb_code()` or `eval()` with `verbosity` 3)
- `verbosity`: (Optional) Controls output detail level (default: 2)
    - `0` - Summary: Brief error message only
    - `1` - Context: Message with error location (graphical display when output_mode > 0)
    - `2` - Detailed: Message, location, and diagnostic hints
- `output_mode`: (Optional) Controls formatting style (default: 0)
    - `0` - Plain text without special characters
    - `1` - Graphics characters for visual clarity
    - `2` - Graphics with ANSI color codes

**Returns:** List of formatted error strings

**Note:** Use this function to format structured error maps returned by `set_verb_code()` or `eval()` when called with
`verbosity` 3.

**Example:**

```moo
// Get structured error data
err = set_verb_code(#123, "test", {"return 1 + ;"}, 3);

// Format with different verbosity levels
summary = format_compile_error(err, 0);
=> {"Parse error at line 1, column 12: unexpected ';'"}

detailed = format_compile_error(err, 2);
=> {"Parse error at line 1, column 12:", "  return 1 + ;", "             ⚠", ...}

// Format with color for terminal display
colored = format_compile_error(err, 2, 2);
```

## Advanced Verb Functions

### `disassemble`

**Description:** Provides a detailed breakdown of the compiled bytecode for a verb.  
**Arguments:**

- : The object that has the verb `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`

**Returns:** A list of strings showing the internal compiled representation of the verb  
**Note:** Output format is not standardized and may change between versions.

### `respond_to`

**Description:** Checks if an object has a verb with a specific name.  
**Arguments:**

- : The object to check `object`
- : The name of the verb to check for `verb-name`

**Returns:**

- If the object doesn't have a verb with that name: (false) `0`
- If the caller controls the object or the object is readable and the verb exists: a list containing the location of the
  verb and its names
- If the caller doesn't control the object but the verb exists: (true) `1`

## Verb Permissions Explained

Verbs in this system use a permission model based on the following flags:

- (read): Controls who can read the verb's code **r**
- **w** (write): Controls who can modify the verb's code
- (execute): Controls who can execute the verb **x**
- (debug): Controls whether the verb runs in debug mode **d**

These permissions are represented as a string (e.g., "rwxd" for all permissions, "rx" for read and execute only).

### `prepositions`

**Description:** Returns a list of all valid prepositions that can be used in verb argument specifications.

**Arguments:** None

**Returns:** A list of preposition entries. Each entry is a list containing three elements:

1. The preposition ID (1-indexed, 1-16)
2. The canonical/short form (single preposition string)
3. A list of all valid alternate forms for that preposition

**Example:**

```moo
prepositions()
=> {
  {1, "with", {"with", "using"}},
  {2, "at", {"at", "to"}},
  {3, "in-front-of", {"in front of", "in-front-of"}},
  {4, "in", {"in", "inside", "into"}},
  ...
  {16, "named", {"named", "called", "known as"}}
}
```

**Note:** This is useful for introspection and for code that needs to validate or work with prepositions
programmatically.

## Verb Arguments Specification

The verb arguments specification consists of three components:

1. **Direct Object (dobj)** - Can be one of:
    - "this" - Object must match the verb's location
    - "none" - No object expected
    - "any" - Any object is acceptable

2. **Preposition (prep)** - Specifies the preposition, like "with", "at", "in", etc. Use `prepositions()` to get a list
   of valid values.
3. **Indirect Object (iobj)** - Same options as Direct Object

These specifications control how the parsing system matches player commands to verbs.
