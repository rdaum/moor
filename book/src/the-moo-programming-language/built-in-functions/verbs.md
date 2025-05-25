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

- : The object with the verb to modify `object`
- : Either the verb name or a positive integer representing the verb's position (1-based) `verb-desc`
- : A list of strings, each representing a line of the verb's source code `code`

**Returns:** If successful, returns `none`. If compilation fails, returns a list of error messages.  
**Note:** Requires appropriate permissions to modify the verb and programmer bit.

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

## Verb Arguments Specification

The verb arguments specification consists of three components:

1. **Direct Object (dobj)** - Can be one of:
   - "this" - Object must match the verb's location
   - "none" - No object expected
   - "any" - Any object is acceptable

2. **Preposition (prep)** - Specifies the preposition, like "with", "at", "in", etc.
3. **Indirect Object (iobj)** - Same options as Direct Object

These specifications control how the parsing system matches player commands to verbs.
