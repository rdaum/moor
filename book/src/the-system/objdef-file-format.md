# Object Definition File Format Reference

This chapter provides a complete technical reference for the object definition file format used by mooR. Object
definition files (objdef files) use a structured text format that describes MOO objects in a human-readable way.

For background information on what object definition files are, how to use them, and their role in MOO development
workflows, see [Object Packaging and Management](object-packaging.md).

## File Structure

An objdef file consists of:

1. **Optional constant declarations** - Define symbolic constants
2. **One or more object definitions** - Complete object specifications

```moo
// Optional constant declarations
define WIZARD = #3;
define ROOM = #456;

// Object definitions
object WIZARD
  name: "The Wizard"
  parent: PLAYER
  // ... more attributes
endobject

object ROOM
  name: "A Generic Room"
  parent: ROOT_ROOM
  // ... more attributes
endobject
```

## Constant Declarations

Constants provide symbolic names for objects and values that can be used throughout the file.

### Syntax

```moo
define CONSTANT_NAME = literal_value;
```

### Examples

```moo
define WIZARD = #3;
define MAX_ITEMS = 100;
define DEFAULT_DESC = "A nondescript object";
define DEBUG_MODE = true;
define ERROR_HANDLER = E_INVARG;
```

### Supported Literal Types

- **Objects**: `#123`, `#0`, `#-1`
- **Integers**: `42`, `-17`, `0`
- **Floats**: `3.14`, `-2.5e10`
- **Strings**: `"Hello, world!"`
- **Booleans**: `true`, `false`
- **Symbols**: `'symbol_name` (if symbols enabled)
- **Errors**: `E_INVARG`, `E_PERM`
- **Lists**: `{1, 2, 3}`, `{"a", "b", "c"}`
- **Maps**: `["key" -> "value", "count" -> 42]`

## Object Definitions

Each object definition describes a complete MOO object.

### Basic Structure

```moo
object OBJECT_IDENTIFIER
  // Required attributes
  name: "Object Name"
  parent: PARENT_OBJECT
  owner: OWNER_OBJECT

  // Optional attributes
  location: LOCATION_OBJECT
  wizard: BOOLEAN
  programmer: BOOLEAN
  player: BOOLEAN
  fertile: BOOLEAN
  readable: BOOLEAN
  writeable: BOOLEAN

  // Properties and verbs
  property prop_name (owner: OWNER, flags: "FLAGS") = VALUE;
  override prop_name = NEW_VALUE;

  verb "verb_name" (ARGSPEC) owner: OWNER flags: "FLAGS"
    // MOO code here
  endverb
endobject
```

## Object Identifiers

Object identifiers can be:

- **Numeric objects**: `#123`, `#0`, `#-1`
- **Constants**: `WIZARD`, `ROOM`, `THING`

## Required Attributes

Every object must specify these attributes:

### name

The object's display name.

```moo
name: "The Wizard's Staff"
name: STAFF_NAME  // Using a constant
```

### parent

The object's parent in the inheritance hierarchy.

```moo
parent: THING
parent: #456
parent: #-1      // No parent (root object)
```

### owner

The object that owns this object.

```moo
owner: WIZARD
owner: #3
```

## Optional Attributes

### location

Where the object is located.

```moo
location: WIZARD_OFFICE
location: #-1    // No location (not in any container)
```

### Boolean Flags

Control object permissions and behavior.

```moo
wizard: true      // Object has wizard privileges
programmer: false // Object cannot program
player: true      // Object is a player
fertile: true     // Object can have children
readable: true    // Object can be examined
writeable: false  // Object properties cannot be modified
```

## Property Definitions

Properties store data on objects.

### Property Definition Syntax

```moo
property PROPERTY_NAME (owner: OWNER, flags: "FLAGS") = INITIAL_VALUE;
```

### Property Override Syntax

```moo
override PROPERTY_NAME (owner: OWNER, flags: "FLAGS") = NEW_VALUE;
override PROPERTY_NAME = NEW_VALUE;  // Keep existing permissions
```

### Property Names

Property names can be:

- **Unquoted identifiers**: `description`, `my_property`
- **Quoted strings**: `"complex name"`, `"name with spaces"`

### Property Flags

Property flags control access permissions:

- **`r`** - Readable
- **`w`** - Writable
- **`c`** - Chown (can change ownership)

Examples:

```moo
property description (owner: WIZARD, flags: "rc") = "A mysterious object";
property secret_data (owner: WIZARD, flags: "c") = {1, 2, 3};
property public_info (owner: WIZARD, flags: "rw") = "Anyone can read and write";
```

### Property Values

Property values can be any literal type:

```moo
property count (owner: WIZARD, flags: "rw") = 42;
property items (owner: WIZARD, flags: "rc") = {"sword", "shield", "potion"};
property config (owner: WIZARD, flags: "rc") = ["debug" -> true, "level" -> 5];
property parent_ref (owner: WIZARD, flags: "rc") = ROOM;
```

### Special Property: import_export_id

The `import_export_id` property is a special property that controls how objects are named during export:

```moo
property import_export_id (owner: WIZARD, flags: "rc") = "my_object_name";
```

**Purpose**: This property establishes stable identity for objects across import/export cycles.

**Behavior**:
- If present, the object exports as `<value>.moo` (e.g., `my_object_name.moo`)
- If absent, the object exports as `<object_number>.moo` (e.g., `123.moo`)
- The value appears in `constants.moo` as a symbolic constant

**Type**: Must be a String or Symbol

**Recommended Flags**: `"rc"` (readable, chown) - should be immutable after creation

**Example**:
```moo
object #789
  name: "Generic Thing"
  parent: ROOT
  owner: WIZARD

  // This makes it export as thing.moo
  property import_export_id (owner: WIZARD, flags: "rc") = "thing";
endobject
```

See [Object Identity and Export Names](object-packaging.md#object-identity-and-export-names) for more details on how this property is used.

## Verb Definitions

Verbs define executable code on objects.

### Verb Syntax

```moo
verb "VERB_NAMES" (ARGSPEC) owner: OWNER flags: "FLAGS"
  // MOO code statements
endverb
```

### Verb Names

Verb names specify how the verb can be called:

```moo
verb "look" (this none none) owner: WIZARD flags: "rxd"
  // Single name
endverb

verb "get take grab" (any from this) owner: WIZARD flags: "rxd"
  // Multiple names (space-separated)
endverb

verb "l*ook examine" (this none none) owner: WIZARD flags: "rxd"
  // Names with wildcards
endverb
```

### Argument Specifications

The argument specification defines what arguments the verb accepts:

```moo
(DIRECT_OBJ PREPOSITION INDIRECT_OBJ)
```

**Direct and Indirect Object Types:**

- **`this`** - Must be this object
- **`any`** - Any object or string
- **`none`** - No object allowed

**Preposition Types:**

- **`none`** - No preposition
- **Specific prepositions**: `with`, `from`, `to`, etc.

**Examples:**

```moo
verb "look" (this none none)         // look
verb "get" (any none none)           // get <object>
verb "put" (any in any)              // put <obj> in <container>
verb "give" (any to any)             // give <obj> to <player>
verb "tell" (any any any)            // tell <player> <message>
```

### Verb Flags

Verb flags control permissions and behavior:

- **`r`** - Readable (can view verb code)
- **`w`** - Writable (can modify verb code)
- **`x`** - Executable (can call the verb)
- **`d`** - Debug (can debug the verb)

Examples:

```moo
verb "look" (this none none) owner: WIZARD flags: "rxd"
verb "admin_cmd" (any any any) owner: WIZARD flags: "rwd"
verb "public_util" (this none none) owner: WIZARD flags: "rx"
```

### Verb Code

The verb body contains standard MOO code:

```moo
verb "look_self" (this none none) owner: WIZARD flags: "rxd"
  if (this.dark && !player.wizard)
    player:tell("It's too dark to see.");
    return;
  endif

  player:tell(this.name);
  if (this.description)
    player:tell(this.description);
  endif

  // Show contents
  contents = this:contents();
  if (contents)
    player:tell("Contents: ", $string_utils:english_list(contents));
  endif
endverb
```

## Data Types

### Primitive Types

**Integers:**

```moo
42
-17
0
```

**Floats:**

```moo
3.14
-2.5e10
1.23e-4
```

**Strings:**

```moo
"Hello, world!"
"String with \"escaped\" quotes"
"Multi\nline\nstring"
```

**Objects:**

```moo
#123        // Object number 123
#0          // System object
#-1         // Invalid/nothing object
```

**Booleans (mooR extension):**

```moo
true
false
```

**Symbols (mooR extension):**

```moo
'symbol_name
'property
'verb_name
```

**Errors:**

```moo
E_INVARG
E_PERM
E_PROPNF
E_CUSTOM("Custom error message")
```

### Collection Types

**Lists:**

```moo
{}                          // Empty list
{1, 2, 3}                   // Integer list
{"a", "b", "c"}             // String list
{#1, #2, #3}                // Object list
{1, "mixed", #3, true}      // Mixed type list
```

**Maps (mooR extension):**

```moo
[]                                    // Empty map
["key" -> "value"]                    // Single entry
["name" -> "Bob", "age" -> 25]        // Multiple entries
['symbol -> "value", "key2" -> 42]    // Mixed key types
```

### Flyweights (mooR extension)

Flyweights are lightweight object-like structures:

```moo
< PARENT, [SLOT -> VALUE, ...], {CONTENTS} >
< THING, ["color" -> "red"], {} >
< ROOM, [], {"table", "chair"} >
< #123, ["hp" -> 100, "mp" -> 50], {#456} >
```

## Comments

### File-Level Comments

Object definition files support both C-style and C++-style comments for documenting the file structure:

```moo
// Single line comment

/*
 * Multi-line comment
 * Can span multiple lines
 */

object WIZARD  // Comment at end of line
  name: "The Wizard"  /* inline comment */
  // More attributes...
endobject
```

**Important**: File-level comments are supported during import but are **lost during export**. They are not stored in
the MOO database and will not appear when you export objects back to objdef format.

### Comments Within Verb Code

For comments that should be preserved, use MOO-style comments within verb code using string literals:

```moo
verb "complex_calculation" (this none none) owner: WIZARD flags: "rxd"
  "This verb performs a complex mathematical calculation";
  "It takes no arguments and returns the result as an integer";

  x = 42;
  "Start with the magic number";

  y = x * 2;
  "Double it for good measure";

  return x + y;
  "Return the sum";
endverb
```

These string literal comments are preserved because they are part of the compiled MOO code stored in the database.

## File Naming Conventions

- **Individual objects**: `123.moo`, `456.moo`
- **Named objects**: `wizard.moo`, `room.moo`, `thing.moo`
- **System object**: `sysobj.moo` (never `0.moo`)
- **Constants**: `constants.moo`

## Complete Example

```moo
// Constants for this file
define WIZARD = #3;
define THING = #789;
define ROOM = #456;

// A utility object
object #12345
  name: "String Utilities"
  parent: THING
  owner: WIZARD
  location: #-1
  wizard: false
  programmer: false
  player: false
  fertile: true
  readable: true
  writeable: false

  property version (owner: WIZARD, flags: "rc") = "2.1.0";
  property debug_mode (owner: WIZARD, flags: "rw") = false;
  property cache (owner: WIZARD, flags: "rc") = [];

  override name = "Enhanced String Utilities";

  verb "capitalize" (this none none) owner: WIZARD flags: "rxd"
    if (!args || length(args) != 1)
      return E_INVARG;
    endif

    str = args[1];
    if (typeof(str) != TYPE_STR)
      return E_TYPE;
    endif

    if (str == "")
      return "";
    endif

    return tostr(str[1] == tostr(str[1])):uppercase(), str[2..$]);
  endverb

  verb "split" (this none none) owner: WIZARD flags: "rxd"
    {str, ?delimiter = " "} = args;

    if (typeof(str) != TYPE_STR || typeof(delimiter) != TYPE_STR)
      return E_TYPE;
    endif

    result = {};
    current = "";

    for i in [1..length(str)]
      char = str[i];
      if (char == delimiter)
        if (current != "")
          result = {@result, current};
          current = "";
        endif
      else
        current = current + char;
      endif
    endfor

    if (current != "")
      result = {@result, current};
    endif

    return result;
  endverb
endobject
```

## Grammar Notes

The complete formal grammar is defined in `/crates/compiler/src/moo.pest`. Key points:

- **Case sensitivity**: Keywords like `object`, `verb`, `property` are case-insensitive
- **Whitespace**: Whitespace and comments are ignored between tokens
- **Identifiers**: Must start with letter or underscore, can contain letters, digits, underscores
- **String escaping**: Standard escape sequences: `\n`, `\t`, `\"`, `\\`, etc.
- **Number formats**: Support for scientific notation, underscores in large numbers
- **Keyword conflicts**: Identifiers can start with keywords (e.g., `objects` is valid)

## Anonymous Objects

If your mooR server has anonymous objects enabled, they receive special treatment in the objdef format since they cannot
be referenced by typed identifiers and don't get symbolic constants.

### Anonymous Object Identifiers

Anonymous objects use a special identifier format in objdef files:

```moo
object #anon_048D05-1234567890
  name: "Temporary Item"
  parent: THING
  owner: WIZARD
  // ... rest of object definition
endobject
```

The format `#anon_XXXXXX-YYYYYYYYYY` represents the internal anonymous object ID, where:

- `XXXXXX` is a 6-digit hex value combining autoincrement and random components
- `YYYYYYYYYY` is a 10-digit hex timestamp

### Anonymous Object File Organization

Unlike regular objects that get individual files, **all anonymous objects are exported to a single file** called
`_anonymous_objects.moo`:

```
objdef_directory/
├── constants.moo              # Regular object constants only
├── sysobj.moo                # System object
├── thing.moo                 # Regular objects get individual files
├── room.moo
├── player.moo
└── _anonymous_objects.moo    # ALL anonymous objects in one file
```

### Constants and References

Anonymous objects are **excluded from constants generation**:

- No entries are created in `constants.moo` for anonymous objects
- Anonymous objects must be referenced by their full `#anon_` identifier
- Other objects can reference anonymous objects using the full identifier

```moo
// In regular object files - direct reference required
property temp_item = #anon_048D05-1234567890;
property item_list = {#anon_048D05-1234567890, #anon_123ABC-9876543210};
```

### When Anonymous Objects Are Exported

Anonymous objects are only exported if they are **reachable** from regular objects:

- Referenced in properties of regular objects
- Referenced in property values (lists, maps, etc.)
- Have properties or verbs defined on them

Anonymous objects that are not reachable will be garbage collected and won't appear in the export.

### Loading Anonymous Objects

When loading objdef files containing anonymous objects:

1. **New anonymous object IDs are generated** - the original IDs from export cannot be preserved
2. **References are automatically updated** - all references to anonymous objects are rewritten to use the new IDs
3. **Relationships are maintained** - parent/child, property references, etc. are preserved

### Anonymous Object Limitations

- **No symbolic constants** - must use full `#anon_` identifiers
- **Cannot be typed directly** in normal MOO code - only appear in objdef format
- **Grouped file export** - all anonymous objects share one file rather than individual files
- **ID regeneration** - original IDs are not preserved across import/export cycles

### Example Anonymous Objects File

```moo
object #anon_048D05-1234567890
  name: "Temporary Sword"
  parent: WEAPON
  owner: WIZARD

  property damage = 15;
  property durability = 100;
endobject

object #anon_123ABC-9876543210
  name: "Quest Item"
  parent: THING
  owner: WIZARD

  property quest_id = "dragon_slayer";
  property is_unique = true;
endobject
```

For more information about anonymous objects and how they work, see
the [Anonymous Objects section](../the-database/objects-in-the-moo-database.md#anonymous-objects) in the objects
documentation.

## Validation Rules

When loading objdef files, mooR validates:

1. **Syntax compliance** with the grammar
2. **Object references** exist or can be resolved
3. **Property and verb names** are valid identifiers
4. **Flag strings** contain only valid flag characters
5. **Argument specifications** use valid argument types
6. **MOO code** in verbs compiles successfully
7. **Constant references** can be resolved
8. **Anonymous object format** (if anonymous objects are enabled)

## Error Handling

Common parsing errors include:

- **Invalid syntax**: Mismatched braces, missing semicolons
- **Undefined constants**: References to undefined symbolic names
- **Invalid object references**: References to non-existent objects
- **Bad flag strings**: Invalid characters in property/verb flags
- **Type mismatches**: Wrong data types for attributes
- **Compilation errors**: Invalid MOO code in verb bodies
- **Invalid anonymous object IDs**: Malformed `#anon_` identifiers

Error messages include file names and line numbers to help locate issues.

## Migrating Legacy Code

If you have objdef files that use legacy LambdaMOO/ToastStunt type constants (`INT`, `OBJ`, `STR`, etc. instead of `TYPE_INT`, `TYPE_OBJ`, `TYPE_STR`), you can migrate them using the `make migrate` target:

```bash
# In your core directory
make migrate
```

Or invoke `moorc` directly:

```bash
moorc --legacy-type-constants true --src-objdef-dir src --out-objdef-dir gen.objdir
cp gen.objdir/*.moo src/
```

This parses the files with legacy type constant support and outputs them in the new `TYPE_*` format. After migration, use `make rebuild` for normal development.

See [Type constant literals](../the-moo-programming-language/extensions.md#type-constant-literals) for more details on this change.