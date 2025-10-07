# Object Definition Files and Object Import/Export

## The Challenge of "Living Database" Systems

MOO belongs to a family of programming systems that are fundamentally different from typical programming languages.
Unlike traditional programs where you write code in text files and then compile or run them, **everything in MOO lives
directly in the database**. Your objects, their properties, their code (verbs), and even the core system itself are all
stored as live, interactive data that can be modified while the system is running.

This "living database" approach draws inspiration from languages like Smalltalk (which calls this "the image") and Self,
and is incredibly powerful for building interactive worlds because:

- **Everything is persistent** - objects you create stick around forever until explicitly destroyed
- **Everything is modifiable** - you can change code, objects, and behaviors while people are using the system
- **Everything is interconnected** - objects can reference each other directly, creating complex webs of relationships

But this power comes with a significant challenge: **how do you move, share, version, or backup your work?**

## Traditional MOO Sharing: The @dump Approach

Historically, MOO developers shared code using the `@dump` verb (provided in LambdaCore type systems), which would
generate a series of authoring commands that could be pasted into another MOO to recreate objects. This approach worked
by essentially "puppeting" the receiving user through the same commands they would have typed to create the object
manually. However, this had significant limitations - it only worked if both MOOs had the same authoring commands
available,
and it wasn't well-suited to modern development workflows involving version control, collaboration, or large-scale
code management.

> **For Traditional MOO Users**: If you're familiar with the `@dump` command, think of object definition files as a
> modern, file-based evolution of that concept, designed for today's development workflows with version control, text
> editors, and collaboration tools.

## mooR's Solution: Object Definition Files

mooR introduces "object definition files" (objdef files) to solve these traditional challenges. This system brings MOO
development into the modern world of software development by providing:

- **Human-readable files** that can be opened in any text editor
- **Version control compatibility** with Git, allowing you to track changes over time
- **Easy sharing and collaboration** through file systems and repositories
- **Bulk operations** for entire libraries, worlds, or cores
- **Cross-MOO compatibility** for sharing code between different servers

This chapter covers how to work with object definition directories as an alternative to traditional textdump files, and
the `dump_object` and `load_object` functions that let you work with individual object definitions programmatically.

For complete technical details about the objdef file format syntax and grammar, see
the [Object Definition File Format Reference](objdef-file-format.md).

## Object Definition Files: A Modern Alternative to Textdumps

Traditionally, MOO databases have been stored and transferred using "textdump" files - large, monolithic text files
containing the entire database in a format that only MOO servers can easily read. While mooR can import and export
textdumps for compatibility, it also supports a more modern approach: **object definition directories**.

### What are Object Definition Files?

Object definition files (objdef files) are individual text files that describe MOO objects in a human-readable format.
Instead of one massive textdump file, an object definition directory contains:

- **Individual files** for each object (e.g., `123.moo`, `456.moo`)
- **Human-readable format** that can be opened in any text editor
- **Version control friendly** structure perfect for Git repositories
- **Database-independent** format that works across different MOO server versions
- **Easily comparable** files for tracking changes over time

### Advantages Over Textdumps

**Revision Control**: Each object is its own file, making Git diffs meaningful and allowing you to track changes to
individual objects over time.

**Collaboration**: Multiple developers can work on different objects simultaneously without merge conflicts.

**Readability**: Object definitions are formatted for human consumption, making it easy to understand what an object
does just by reading its file.

**Modularity**: You can easily extract, share, or backup individual objects or sets of objects.

**Cross-Platform**: Object definition files work identically across different MOO servers and versions.

### Uses for Object Definition Directories

**Core Development**: The [cowbell core](https://github.com/rdaum/cowbell/) is built entirely from object definition
files, making it easy for contributors to add features and track changes.

**Database Backups**: Create readable, version-independent backups of your entire database that will remain usable even
as mooR evolves.

**Code Sharing**: Distribute libraries, utilities, or individual objects as readable files that others can examine,
modify, and integrate into their own databases.

**Development Workflow**: Build and test your MOO objects in a development environment, then deploy them to production
by loading the object definition files.

**Core Migration**: Convert existing [LambdaCore](understanding-moo-cores.md) or similar databases into object
definition format for easier maintenance and customization.

## Working with Object Definition Directories

### Command-Line Import and Export

mooR provides command-line tools for importing and exporting entire databases as object definition directories. This is
typically how you work with cores, perform database migrations, or create comprehensive backups.

#### Exporting to Object Definition Format

To export your entire database as an object definition directory:

```bash
# Export current database to objdef format
moor-daemon --export /path/to/export/directory --export-format objdef

# You can also export to traditional textdump format
moor-daemon --export /path/to/backup.db --export-format textdump
```

This creates a directory structure where each object becomes its own `.moo` file, numbered by object ID (e.g., `1.moo`,
`2.moo`, `123.moo`).

#### Importing from Object Definition Format

To import an object definition directory into a new database:

```bash
# Import from objdef directory
moor-daemon --import /path/to/objdef/directory --import-format objdef

# Import from traditional textdump
moor-daemon --import /path/to/backup.db --import-format textdump
```

#### Format Conversion

You can use mooR to convert between formats:

```bash
# Convert textdump to objdef
moor-daemon --import old_database.db --import-format textdump \
            --export new_objdef_dir --export-format objdef

# Convert objdef back to textdump
moor-daemon --import objdef_directory --import-format objdef \
            --export new_database.db --export-format textdump
```

#### Automatic Timestamped Exports

When you configure an export path, mooR automatically creates timestamped exports during database checkpoints. Each
export gets a unique filename based on Unix timestamp to prevent overwriting previous backups:

```bash
# Configure automatic exports with checkpoint interval
moor-daemon --export /path/to/backups --export-format objdef \
            --checkpoint-interval-seconds 3600  # Export every hour
```

This creates files like:

```
/path/to/backups/
├── textdump-1704067200.moo         # Exported at 2024-01-01 00:00:00
├── textdump-1704070800.moo         # Exported at 2024-01-01 01:00:00
├── textdump-1704074400.moo         # Exported at 2024-01-01 02:00:00
└── ...
```

The checkpoint interval controls how frequently these automatic exports occur. This provides:

- **Rolling backups** that don't overwrite each other
- **Point-in-time recovery** to any checkpoint moment
- **Automatic versioning** without manual intervention
- **Safe concurrent operation** using `.in-progress` temporary files

### Directory Structure

An example object definition directory contains:

```
objdef_directory/
├── constants.moo   # Special file with symbolic names for objects
├── sysobj.moo     # System object (#0)
├── root.moo       # Root class (#1)
├── wiz.moo        # Wizard object (#3)
├── thing.moo      # Generic thing prototype ($thing)
├── room.moo       # Room prototype ($room)
├── player.moo     # Player prototype ($player)
├── 123.moo        # Your custom object (#123)
├── 456.moo        # Another object (#456)
└── ...
```

#### The Special `constants.moo` File

The `constants.moo` file is like a set of preprocessor defines that give human-readable names to important objects.
Instead of remembering that the generic thing prototype is object #789, you can refer to it as `thing`. This file
contains mappings like:

```moo
// Example contents of constants.moo
define THING = #789;
define ROOM = #456;
define PLAYER = #123;
define WIZARD = #3;
define ROOT_ROOM = #2;
define SYSOBJ = #0;
```

When you import an objdef directory, these constants become available during compilation, so verb code can use readable
names instead of magic numbers.

#### System Object Reference Translation

mooR automatically handles the common MOO pattern of storing important object references as properties on the system
object (#0). During export:

1. **Detection**: The exporter examines every property on #0 that directly points to an object
2. **File Naming**: Objects referenced by #0 properties get exported with the property name as the filename
3. **Constants Generation**: These symbolic names are automatically added to `constants.moo`

For example, if #0 has these properties:

```moo
#0.thing = #789      // Generic thing prototype
#0.room = #456       // Room prototype
#0.player = #123     // Player prototype
#0.wizard = #3       // Wizard object
```

Then during export:

- Object #789 gets exported as `thing.moo` (instead of `789.moo`)
- Object #456 gets exported as `room.moo` (instead of `456.moo`)
- Object #123 gets exported as `player.moo` (instead of `123.moo`)
- Object #3 gets exported as `wizard.moo` (instead of `3.moo`)

And `constants.moo` automatically includes:

```moo
define THING = #789;
define ROOM = #456;
define PLAYER = #123;
define WIZARD = #3;
```

#### Benefits of This System

**Human Readability**: Files are named `player.moo` instead of `123.moo`, making the directory structure
self-documenting.

**Object Number Independence**: Code can refer to `PLAYER` instead of hardcoding #123, making it portable between
databases.

**Automatic Maintenance**: The export process handles all the bookkeeping - you don't need to manually maintain the
mappings.

**Standard MOO Patterns**: This works seamlessly with existing MOO conventions like `$thing`, `$room`, etc.

#### The Special `sysobj.moo` File

Object #0 (the system object) is always exported as `sysobj.moo`, never as `0.moo`. This file contains all the
properties that define the core object references for your MOO, like:

```moo
// Example property in sysobj.moo
property thing (owner: WIZARD, flags: "rc") = THING;
property room (owner: WIZARD, flags: "rc") = ROOM;
property player (owner: WIZARD, flags: "rc") = PLAYER;
```

#### Creating New Objects in Objdef Format

When you want to create a new object directly in objdef format (rather than using MOO's in-world authoring tools), you
need to follow a specific pattern to ensure the automatic naming system works correctly:

**Step 1: Choose an Object Number**
Pick an unused object ID that won't conflict with existing objects. Check your current database to see what numbers are
in use:

```bash
# Look at existing objdef directory to see what numbers are taken
ls objdef_directory/*.moo | grep -E '[0-9]+\.moo$'
```

**Step 2: Add the Constant Definition**
Add your new object to `constants.moo`:

```moo
// In constants.moo
define MY_NEW_OBJECT = #12345;
```

**Step 3: Add the System Object Property**
Add a property to `sysobj.moo` that points to your new object:

```moo
// In sysobj.moo
property my_new_object (owner: WIZARD, flags: "rc") = MY_NEW_OBJECT;
```

**Step 4: Create the Object File**
Create your object file with the correct name that matches the property name:

```bash
# Create file: my_new_object.moo
# NOT: 12345.moo or MY_NEW_OBJECT.moo
```

**Step 5: Maintain the Pattern**
If you name your file something that doesn't match this pattern during import, future exports will **not** maintain your
custom filename. The exporter only uses symbolic names for objects that follow the #0 property pattern.

#### Example: Adding a New Utility Object

Let's say you want to add a new string manipulation utility object:

1. **Choose ID**: Pick #98765 (assuming it's unused)

2. **Update `constants.moo`**:
   ```moo
   define STRING_FORMATTER = #98765;
   ```

3. **Update `sysobj.moo`**:
   ```moo
   property string_formatter (owner: WIZARD, flags: "rc") = STRING_FORMATTER;
   ```

4. **Create `string_formatter.moo`**:
   ```moo
   object STRING_FORMATTER
     name: "String Formatting Utilities"
     parent: THING
     owner: WIZARD
     flags: "upw"

     // ... properties and verbs would go here
   endobject
   ```

5. **Import and Export Test**: After importing this objdef directory and then exporting it again, the object will
   continue to be exported as `string_formatter.moo` because it follows the pattern.

#### Common Mistakes to Avoid

- **Wrong filename**: Creating `98765.moo` instead of `string_formatter.moo`
- **Missing sysobj property**: Adding to constants.moo but forgetting the #0 property
- **Case mismatch**: Using `STRING_FORMATTER.moo` instead of `string_formatter.moo`
- **Skipping constants**: Adding the sysobj property but not defining the constant

Each `.moo` file is human-readable and contains the complete definition of that object, including:

- Object metadata (parent, location, owner, flags)
- All properties with values and permissions
- All verbs with code and permissions
- Access to compilation constants from `constants.moo`

### Use Cases for Directory Operations

**Core Development**: Export a working core as objdef, modify objects in your text editor, and re-import to test
changes.

**Database Migration**: Move databases between different mooR versions or even different MOO server implementations by
exporting as objdef.

**Backup and Restore**: Create human-readable backups that remain valid even as the server software evolves.

**Collaboration**: Share entire databases or core systems through version control systems like Git.

## Working with Individual Objects

While command-line import/export handles entire databases, mooR also provides built-in functions for working with
individual objects from within the MOO itself. This enables more surgical operations like cherry-picking specific
objects, sharing individual utilities, or performing targeted updates.

Within object definition files and directories, each object is described as a structured text representation that
includes all its properties, verbs, and metadata. When you work with individual objects using `dump_object` and
`load_object`, you're working with pieces of this broader object definition format.

When you dump an object, you get a list of strings that completely describe that object in the same format used in
object definition files. When you load that definition back, mooR can recreate the object exactly as it was, or merge it
with existing objects according to your preferences.

## Basic Usage

### Dumping Objects

The `dump_object` function converts any object into its text representation:

```moo
// Dump a single object
definition = dump_object(#123);
// Returns a list of strings representing the object

// Save the definition for later use
player.my_object_backup = dump_object($my_widget);
```

### Loading Objects

The `load_object` function recreates objects from their text definitions:

```moo
// Load a simple object (creates a new object)
new_obj = load_object(definition);

// Load with options
new_obj = load_object(definition, [
    `target_object -> #456,     // Update existing object instead
    `constants -> [`MY_CONSTANT -> "value"]  // Set compilation constants
]);
```

### Batch Mutations

The `mutate_objects` function provides a powerful way to apply multiple atomic mutations across one or more objects in a
single transaction. Unlike `load_object` which works with complete object definitions, `mutate_objects` lets you apply
specific targeted changes (like defining properties, updating verbs, or changing object attributes) to multiple objects
efficiently.

**Function:** `mutate_objects(list changelist)`
**Returns:** Map containing per-object results with success/failure details
**Permissions:** Wizard only

#### Basic Usage

```moo
// Define a property on a single object
result = mutate_objects({
    {#123, {
        {'define_property, 'description, #2, "rwc", "A red ball"}
    }}
});

// Apply multiple mutations to one object
result = mutate_objects({
    {$my_object, {
        {'define_property, 'version, #2, "rc", "1.0.0"},
        {'set_property_value, 'name, "Updated Widget"},
        {'set_object_flags, "upw"}
    }}
});

// Mutate multiple objects in one transaction
result = mutate_objects({
    {#100, {{'define_property, 'locked, #2, "rc", 0}}},
    {#101, {{'define_property, 'locked, #2, "rc", 0}}},
    {#102, {{'define_property, 'locked, #2, "rc", 0}}}
});
```

#### Changelist Format

The changelist is a list of `{target_object, mutations}` pairs:

```moo
{
    {object1, {mutation1, mutation2, ...}},
    {object2, {mutation1, mutation2, ...}},
    ...
}
```

Each mutation is a list starting with a mutation action symbol, followed by action-specific arguments.

#### Available Mutation Actions

**Property Operations:**

- `{'define_property, name, owner, flags, value}` - Create a new property with initial value
- `{'delete_property, name}` - Remove a property definition
- `{'set_property_value, name, value}` - Change property value
- `{'set_property_flags, name, owner, flags}` - Update property permissions
- `{'clear_property, name}` - Reset property to inherited value

**Verb Operations:**

- `{'define_verb, {names}, owner, flags, argspec, program}` - Create a new verb
    - `names`: List of symbols like `{'look, 'l, 'examine}`
    - `argspec`: List like `{"this", "none", "this"}`
    - `program`: String containing verb code
- `{'delete_verb, {names}}` - Remove a verb
- `{'update_verb_code, {names}, program}` - Change verb code only
- `{'update_verb_metadata, {names}, new_names, owner, flags, argspec}` - Update verb properties
    - Any of `new_names`, `owner`, `flags`, or `argspec` can be `0` to leave unchanged

**Object Lifecycle Operations:**

- `{'create_object, objid_or_0, parent, location, owner, flags}` - Create a new object
  - `objid_or_0`: If `0`, auto-assign next available object ID; otherwise use specified object ID (will error if exists)
  - `flags`: String like "upw" for user/programmer/wizard
  - **Note**: Will raise E_INVARG if an object with the specified ID already exists
- `{'recycle_object}` - Delete/recycle the target object

**Object Attribute Operations:**

- `{'set_object_flags, flags}` - Change object permission flags (e.g., "upw" for user/programmer/wizard)
- `{'set_parent, parent_obj}` - Change object's parent
- `{'set_location, location_obj}` - Move object to new location

#### Result Format

The function returns a map with detailed per-object results:

```moo
{
    ['results -> {
        ['index -> 1, 'success -> true],
        ['index -> 2, 'success -> false, 'error -> E_INVARG]
    },
    'target -> #123,
    'success -> false  // Overall success (false if any mutation failed)
}
```

Each result includes:

- `target` - The object that was mutated
- `success` - Boolean indicating if all mutations succeeded
- `results` - List of per-mutation results with:
    - `index` - 1-based index of the mutation in the list
    - `success` - Boolean indicating if this specific mutation succeeded
    - `error` - MOO error code if mutation failed (E_INVARG, E_PROPNF, etc.)

#### Error Handling

Common errors returned in results:

- `E_INVARG` - Invalid argument (e.g., duplicate property definition, invalid flags)
- `E_PROPNF` - Property not found (when updating non-existent property)
- `E_VERBNF` - Verb not found (when updating non-existent verb)
- `E_TYPE` - Type mismatch in arguments
- `E_PERM` - Permission denied (shouldn't occur for wizard)

#### Example: Checking Results

```moo
result = mutate_objects(changelist);

if (result['success])
    player:tell("All mutations succeeded!");
else
    player:tell("Some mutations failed:");
    for mutation_result in (result['results])
        if (!mutation_result['success])
            player:tell("  Mutation ", mutation_result['index],
                       " failed with ", mutation_result['error]);
        endif
    endfor
endif
```

#### Use Cases

**Bulk Property Initialization:**

```moo
// Add the same property to many objects at once
objects = children($thing);
changelist = {};
for obj in (objects)
    changelist = {@changelist, {obj, {
        {'define_property, 'initialized, #2, "rc", time()}
    }}};
endfor
mutate_objects(changelist);
```

**Atomic Multi-Object Updates:**

```moo
// Update multiple related objects atomically
mutate_objects({
    {$config, {{'set_property_value, 'version, "2.0"}}},
    {$system, {{'update_verb_code, {'init}, "/* new code */"}}},
    {$registry, {{'clear_property, 'cache}}}
});
```

**Safe Verb Deployment:**

```moo
// Update verbs across multiple objects, check for failures
result = mutate_objects({
    {$handler1, {{'update_verb_code, {'process}, new_code}}},
    {$handler2, {{'update_verb_code, {'process}, new_code}}},
    {$handler3, {{'update_verb_code, {'process}, new_code}}}
});

if (!result['success])
    // Rollback is automatic - transaction was not committed
    player:tell("Deployment failed, all changes rolled back");
endif
```

#### Important Notes

- **Atomic Transactions**: All mutations across all objects happen in a single transaction. If any mutation fails, the
  entire operation is automatically rolled back and no changes are committed to the database.
- **All-or-Nothing**: The function only commits if every single mutation succeeds. Even one failure causes a complete
  rollback.
- **Transactional Isolation**: On success, `mutate_objects` commits its transaction before returning, ensuring changes
  are immediately visible to subsequent operations.
- **Wizard Only**: This function requires wizard permissions as it bypasses normal permission checks.
- **Performance**: More efficient than individual mutation operations for bulk updates since it batches all changes into
  a single transaction.

## Advanced Loading Options

The `load_object` function accepts an optional second argument - a map of options that control how the loading process
works. This map can contain any combination of the following options:

## Complete Options Reference

> **Note about Examples**: The examples in this documentation
> use [symbols](../the-moo-programming-language/extensions.md#symbol-type) (like `'dry_run`), boolean values (`true`/
> `false`), and [maps](../the-moo-programming-language/extensions.md#map-type) (like `['key -> "value"]`) which are mooR
> extensions. If your mooR instance is not configured with these extension features enabled, you can use strings (
`"dry_run"`),
> integers (`1`/`0`), and alists (`{{"key", "value"}, ...}`) instead throughout - they work identically.

The following table provides a complete reference for all `load_object` options:

| Option             | Type    | Default    | Description                                             |
|--------------------|---------|------------|---------------------------------------------------------|
| `target_object`    | Object  | None       | Existing object to update instead of creating new       |
| `constants`        | Map     | `[]`       | Compilation constants available during verb compilation |
| `conflict_mode`    | Symbol  | `'clobber` | How to handle conflicts: `'clobber`, `'skip`, `'detect` |
| `dry_run`          | Boolean | `false`    | Test mode - don't make actual changes                   |
| `return_conflicts` | Boolean | `false`    | Return detailed conflict information                    |
| `overrides`        | List    | `{}`       | Force specific entities to use `clobber` mode           |
| `removals`         | List    | `{}`       | Remove specified entities not in definition             |

### Option Details

### Target Object

**Option:** `target_object`
**Type:** Object
**Default:** None (create new object)

Instead of creating a new object, load the definition into an existing object:

```moo
// Update an existing object with new definition
load_object(definition, [`target_object -> #123]);

// Useful for updating objects in place
load_object(new_widget_def, [`target_object -> $my_widget]);
```

### Compilation Constants

**Option:** `constants`
**Type:** Map
**Default:** Empty map

Provide constants that will be available when resolving object references in property values:

```moo
load_object(definition, [
    `constants -> [
        `THING -> #789,
        `ROOM -> #456,
        `PLAYER -> #123,
        `WIZARD -> #3,
    ]
]);
```

These constants are used to resolve symbolic object references in property values, similar to the `constants.moo` file
in object definition directories. They allow object definitions to use readable names instead of hardcoded object
numbers.

### Conflict Handling

**What is a Conflict?**

A conflict occurs when you try to load an object definition that contains data that differs from what already exists in
the database. For example:

- **Property conflicts**: The object definition sets `description = "A red ball"` but the existing object has
  `description = "A blue sphere"`
- **Verb conflicts**: The definition includes a `look` verb with different code than the existing `look` verb
- **Flag conflicts**: The definition specifies different object flags (like wizard/programmer status) than currently set
- **Ownership conflicts**: The definition assigns different owners to properties or verbs

**Why Conflicts Matter**

Conflicts are important because they represent potential data loss or unintended changes:

- **User customizations**: Players may have customized descriptions or properties that you don't want to overwrite
- **Site-specific modifications**: Your MOO may have local changes to core objects that should be preserved
- **Version differences**: Loading an older object definition might downgrade newer functionality
- **Security implications**: Changing ownership or permissions could create security vulnerabilities

**Option:** `conflict_mode`
**Type:** Symbol
**Default:** `clobber`
**Values:** `clobber`, `skip`, `detect`

Controls what happens when the definition conflicts with existing object data:

```moo
// Overwrite everything (default) - DESTROYS existing conflicting data
load_object(definition, [`conflict_mode -> `clobber]);

// Skip conflicting parts, only add new properties/verbs - PRESERVES existing data
load_object(definition, [`conflict_mode -> `skip]);

// Don't make changes, just report what conflicts exist - SAFE inspection
load_object(definition, [`conflict_mode -> `detect]);
```

**When to Use Each Mode:**

- **`clobber`**: When you want to completely replace objects with canonical versions (fresh installs, reverting changes)
- **`skip`**: When adding new functionality while preserving existing customizations (package updates, safe installs)
- **`detect`**: When you need to understand what would change before deciding how to proceed (conflict analysis, impact
  assessment)

### Dry Run Mode

**Option:** `dry_run`
**Type:** Boolean
**Default:** `false`

Test what would happen without actually making changes:

```moo
// See what conflicts would occur
result = load_object(definition, [
    `dry_run -> true,
    `return_conflicts -> true
]);
// Examine result[2] for conflict details
```

### Selective Overrides

**Option:** `overrides`
**Type:** List of `{object, entity}` pairs
**Default:** Empty list

Force specific parts to be overwritten even in `skip` mode:

```moo
load_object(definition, [
    `conflict_mode -> `skip,
    `overrides -> [
        {#123, {'property_value, 'description}},
        {#123, {'verb_program, {'look, 'l}}},
        {#456, 'object_flags}
    ]
]);
```

Available entity types (see Entity Reference below for complete details):

- `object_flags` - Object permission flags
- `builtin_props` - Built-in properties like name, description
- `parentage` - Parent/child relationships
- `{'property_def, name}` - Property definition
- `{'property_value, name}` - Property value
- `{'property_flag, name}` - Property permissions
- `{'verb_def, {names}}` - Verb definition (names is list like {'look, 'l})
- `{'verb_program, {names}}` - Verb code

### Selective Removals

**Option:** `removals`
**Type:** List of `{object, entity}` pairs
**Default:** Empty list

Remove specific properties or verbs that aren't in the definition:

```moo
load_object(definition, [
    `removals -> [
        {#123, {'property_def, 'old_property}},
        {#123, {'verb_def, {'obsolete_verb}}}
    ]
]);
```

### Detailed Results

**Option:** `return_conflicts`
**Type:** Boolean
**Default:** `false`

Get detailed information about the loading process:

```moo
result = load_object(definition, [`return_conflicts -> true]);
// result[1]: success (boolean)
// result[2]: conflicts (list of conflict details)
// result[3]: removals (list of what was removed)
// result[4]: loaded objects (list of object numbers)
```

## Entity Reference

When working with `overrides` and `removals` options, you specify entities using symbol-based identifiers. Each entity
type targets a specific part of an object's data:

### Object-Level Entities

**Object Flags** - `'object_flags`

- **Description**: Object permission flags (user, programmer, wizard, fertile, readable, writeable)
- **Example**: `{#123, 'object_flags}`
- **Use case**: Changing an object's basic permissions

**Built-in Properties** - `'builtin_props`

- **Description**: Built-in object properties like name, location, owner, parent
- **Example**: `{#123, 'builtin_props}`
- **Use case**: Updating core object metadata

**Parentage** - `'parentage`

- **Description**: Parent-child inheritance relationships
- **Example**: `{#123, 'parentage}`
- **Use case**: Changing which object this inherits from

### Property-Level Entities

**Property Definition** - `'property_def`

- **Description**: Complete property definition (creates new property with permissions and initial value)
- **Format**: `{'property_def, property_name}` (list with type symbol and property name)
- **Example**: `{#123, {'property_def, 'description}}`
- **Use case**: Adding or completely replacing a property definition

**Property Value** - `'property_value`

- **Description**: Just the value of a property (preserves existing permissions)
- **Format**: `{'property_value, property_name}` (list with type symbol and property name)
- **Example**: `{#123, {'property_value, 'description}}`
- **Use case**: Updating content while keeping permissions

**Property Flags** - `'property_flag`

- **Description**: Just the permissions flags of a property (preserves existing value)
- **Format**: `{'property_flag, property_name}` (list with type symbol and property name)
- **Example**: `{#123, {'property_flag, 'description}}`
- **Use case**: Changing who can read/write a property

### Verb-Level Entities

**Verb Definition** - `'verb_def`

- **Description**: Complete verb definition (names, permissions, argument spec)
- **Format**: `{'verb_def, {verb_names}}` (list with type symbol and list of verb names)
- **Example**: `{#123, {'verb_def, {'look, 'l, 'examine}}}`
- **Use case**: Adding new verb or changing verb metadata
- **⚠️ Important**: Verb identity is determined by the **complete set of names**. If you add or remove aliases, it
  becomes a different verb.

**Verb Program** - `'verb_program`

- **Description**: Just the code/program of a verb (preserves existing definition)
- **Format**: `{'verb_program, {verb_names}}` (list with type symbol and list of verb names)
- **Example**: `{#123, {'verb_program, {'look, 'l}}}`
- **Use case**: Updating verb code while keeping permissions
- **⚠️ Important**: Must specify the **exact same names** as the existing verb to update it.

### Entity Usage Examples

```moo
// Force update just the description property value, skip everything else
load_object(definition, [
    `conflict_mode -> `skip,
    `overrides -> [
        {$my_object, {'property_value, 'description}}
    ]
]);

// Update verb code but preserve existing permissions
load_object(definition, [
    `conflict_mode -> `skip,
    `overrides -> [
        {$my_object, {'verb_program, {'main_function, 'main}}}
    ]
]);

// Remove obsolete properties and verbs not in the new definition
load_object(definition, [
    `removals -> [
        {$my_object, {'property_def, 'old_config}},
        {$my_object, {'verb_def, {'deprecated_verb}}}
    ]
]);

// Complex selective update preserving user customizations
load_object(package_update, [
    `target_object -> $package_obj,
    `conflict_mode -> `skip,           // Preserve existing data by default
    `overrides -> [
        // Force update these core components
        {$package_obj, {'verb_program, {'init, 'initialize}}},
        {$package_obj, {'property_value, 'version}},
        {$package_obj, 'object_flags}   // Simple entities are just symbols
    ],
    `removals -> [
        // Clean up deprecated features
        {$package_obj, {'property_def, 'legacy_setting}},
        {$package_obj, {'verb_def, {'old_interface}}}
    ]
]);
```

### Entity Selection Tips

**Property vs Value vs Flag**:

- Use `{'property_def, name}` when adding completely new properties
- Use `{'property_value, name}` when updating content but preserving permissions
- Use `{'property_flag, name}` when changing access control but preserving content

**Verb Def vs Program**:

- Use `{'verb_def, {names}}` when changing verb names, permissions, or argument specifications
- Use `{'verb_program, {names}}` when just updating the code

**Removal Safety**:

- Only specify removals for entities you're certain should be deleted
- Test with `dry_run` first to see what would be removed

## Important: Verb Name Behavior

**Verb Identity is Based on Complete Name Sets**

When working with verbs, it's crucial to understand that **verb identity is determined by the complete set of names**,
not individual name matches. This has significant implications:

### Scenario: Adding Aliases

```moo
// Existing verb in database: {"look", "l"}

// Objdef file contains:
verb "look l examine" (this none none) owner: WIZARD flags: "rxd"
  player:tell("You look around.");
endverb

// Result: Creates a NEW verb with all three names
// The old {"look", "l"} verb remains unchanged
// You now have TWO verbs that respond to "look" and "l"!
```

### Scenario: Removing Aliases

```moo
// Existing verb in database: {"look", "l", "examine"}

// Objdef file contains:
verb "look l" (this none none) owner: WIZARD flags: "rxd"
  player:tell("You look around.");
endverb

// Result: Creates a NEW verb with just two names
// The old {"look", "l", "examine"} verb remains unchanged
// You now have TWO verbs with overlapping names!
```

### Why This Behavior Exists

The loader cannot determine **intent** when verb names change:

- Did you want to add an alias to the existing verb?
- Did you want to create a new verb that happens to share some names?
- Did you want to rename the verb entirely?

Since the intent is ambiguous, the loader treats different name sets as different verbs.

### Best Practices for Verb Management

**Option 1: Manual Verb Updates**

```moo
// To add an alias to an existing verb:
// 1. Use your MOO's verb management commands to modify the existing verb in-world
// 2. Then export to capture the change in objdef format
```

**Option 2: Explicit Removal + Recreation**

```moo
// To replace a verb with a new alias set:
load_object(new_definition, [
    `removals -> [
        {$my_object, {'verb_def, {'old_names}}}  // Remove old version
    ]
    // The new definition will create the verb with new names
]);
```

**Option 3: Target Object Strategy**

```moo
// Load into a temporary object first, then manually copy verbs
temp_obj = load_object(definition);
// Manually copy/update verbs as needed
// Then recycle temp_obj
```

### Conflict Detection for Verbs

The conflict detection system will warn you about name overlaps:

```moo
result = load_object(definition, [`conflict_mode -> `detect, `return_conflicts -> true]);
// Check result[2] for verb conflicts before proceeding
```

This behavior ensures **data safety** at the cost of requiring more explicit management of verb aliases.

## Practical Scenarios

### Package Installation

Installing a new package that might conflict with existing objects:

```moo
// First, check for conflicts
result = load_object(package_def, [
    `dry_run -> true,
    `return_conflicts -> true
]);

if (result[1])
    // No conflicts, safe to install
    load_object(package_def);
else
    // Handle conflicts - perhaps ask user what to do
    player:tell("Package conflicts with: ", result[2]);
endif
```

### Safe Updates

Updating an object while preserving user customizations:

```moo
// Update only the core functionality, skip user properties
load_object(new_version, [
    `target_object -> $my_object,
    `conflict_mode -> `skip,
    `overrides -> [
        {$my_object, {'verb_program, {'main_function}}},
        {$my_object, {'property_value, 'version}}
    ]
]);
```

### Database Migration

Moving objects between servers with different configurations:

```moo
// Export from source server
definitions = [];
for obj in (objects_to_migrate)
    definitions[obj] = dump_object(obj);
endfor

// Import on target server with appropriate constants
for obj in (keys(definitions))
    load_object(definitions[obj], [
        `constants -> [`THING -> #789, `ROOM -> #456, `PLAYER -> #123],
        `conflict_mode -> `skip  // Don't overwrite existing customizations
    ]);
endfor
```

### Conflict Resolution

Handling conflicts intelligently:

```moo
// Check what conflicts exist
result = load_object(new_package, [
    `conflict_mode -> `detect,
    `return_conflicts -> true
]);

// Process each conflict
for conflict in (result[2])
    obj = conflict[1];
    conflict_type = conflict[2];

    if (conflict_type == {'property_value, 'description})
        // Ask user whether to keep old or use new description
        // Note: :choose is a hypothetical verb to prompt the user
        choice = player:choose("Keep existing description?");
        // Handle based on choice...
    endif
endfor
```

## Flag String Formats

When working with object and property flags in conflict reports or entity specifications, mooR uses readable string
formats:

### Object Flags

- `u` - User flag
- `p` - Programmer flag
- `w` - Wizard flag
- `r` - Read flag
- `W` - Write flag (capital W)
- `f` - Fertile flag

Example: `"upw"` means user, programmer, and wizard flags are set.

### Property Flags

- `r` - Read permission
- `w` - Write permission
- `c` - Chown permission

Example: `"rw"` means read and write permissions.

### Verb Flags

- `r` - Read permission
- `w` - Write permission
- `x` - Execute permission
- `d` - Debug permission

Example: `"rwx"` means read, write, and execute permissions.

## Best Practices

### Version Control

Always include version information in your object definitions:

```moo
// Set version property before dumping
obj.version = "2.1.0";
obj.last_updated = time();
definition = dump_object(obj);
```

### Backup Before Loading

Create backups before making significant changes:

```moo
backup = dump_object($important_object);
// Store backup somewhere safe
result = load_object(new_definition, [`target_object -> $important_object]);
if (!result[1])
    // Restore from backup if needed
    load_object(backup, [`target_object -> $important_object]);
endif
```

### Test in Development First

Always test packages in a development environment:

```moo
// Load into test objects first
test_result = load_object(package, [
    `target_object -> $test_object,
    `return_conflicts -> true
]);

// Only proceed to production if tests pass
if (test_passes(test_result))
    load_object(package, [`target_object -> $production_object]);
endif
```

## Error Handling

The `load_object` function can return various errors. Always check the result:

```moo
try
    result = load_object(definition, options);
    if (typeof(result) == LIST && !result[1])
        // Loading failed, check conflicts
        player:tell("Load failed due to conflicts: ", result[2]);
    else
        // Success
        player:tell("Object loaded successfully: #", result);
    endif
except error (E_INVARG)
    player:tell("Invalid object definition format");
except error (E_PERM)
    player:tell("Permission denied - wizard access required");
except error (ANY)
    player:tell("Unexpected error: ", error[2]);
endtry
```

