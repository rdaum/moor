# Emergency Medical Hologram Tool (moor-emh)

The `moor-emh` tool is an emergency database administration utility for mooR. It provides direct database access when normal logins are unavailable, making it essential for database recovery, emergency repairs, and system administration tasks.

## Overview

Think of `moor-emh` as the Emergency Medical Hologram for your MOO database - it activates when you need immediate access to repair critical issues. The tool provides a REPL (Read-Eval-Print Loop) interface with full wizard privileges, allowing you to:

- Inspect and modify object properties
- View and edit verb code
- Execute MOO code directly
- List objects, properties, and verbs
- Export and import objects in objdef format
- Reload objects from version-controlled source files
- Perform emergency database repairs

## When to Use moor-emh

Use `moor-emh` when:

- Normal telnet/web access is broken or unavailable
- You need to recover from a catastrophic database error
- The wizard character is locked out or corrupted
- You need to perform emergency maintenance without starting the full server
- You want to inspect or modify the database in a safe, read-only context
- You need to reload objects from version-controlled source files
- You want to export objects for backup or sharing with other MOOs

**Important:** `moor-emh` acquires an exclusive lock on the database, preventing other mooR processes from accessing it. Always shut down your mooR server before using this tool.

## Running moor-emh

### Basic Usage

```bash
moor-emh [OPTIONS] [DATA-DIR]
```

**Arguments:**
- `DATA-DIR` - Directory containing the database files (default: `./moor-data`)

**Options:**
- `--db <DB>` - Main database filename (default: `world.db`)
- `--wizard <WIZARD>` - Object ID to use as wizard (defaults to first valid wizard)
- `--debug` - Enable debug logging

### Examples

Start with default settings (looks for `./moor-data/world.db`):
```bash
moor-emh
```

Specify a custom data directory:
```bash
moor-emh /path/to/my/moo/data
```

Use a specific wizard object:
```bash
moor-emh --wizard 3
```

Use a different database file:
```bash
moor-emh --db backup.db /var/moor-data
```

## The REPL Interface

When you start moor-emh, you'll see the Emergency Medical Hologram welcome screen:

```
# Emergency Medical Hologram - Database Administration Subroutine

Please state the nature of the database emergency.

Running as wizard: #2

Type help for available commands or quit to deactivate.

(#2):
```

The prompt shows the wizard object you're operating as (e.g., `(#2):`).

### Object Reference Syntax

Throughout moor-emh, you can reference objects in two ways:

- **Direct reference:** `#123` - References object with ID 123
- **Property reference:** `$player` - Looks up the value of `#0.player` property

Property references (`$name`) are resolved by reading the property `name` from system object `#0`. This matches MOO syntax and allows you to use symbolic names instead of hardcoded object IDs. For example, if `#0.wiz` contains `#2`, then you can use `$wiz` anywhere you would use `#2`.

## Available Commands

### Basic Commands

| Command | Description |
|---------|-------------|
| `help`, `?` | Display help message with all available commands |
| `quit`, `exit` | Save changes and exit the tool |

### Evaluating MOO Code

Execute MOO expressions and code blocks directly:

| Command | Description |
|---------|-------------|
| `;EXPR` | Evaluate a MOO expression and print the result |
| `;;CODE` | Execute a multi-line MOO code block and print the result |

**Examples:**
```moo
(#2): ;2 + 2
=> 4

(#2): ;#1.name
=> "Root Object"

(#2): ;;x = 10; y = 20; return x + y;
=> 30
```

### Reading and Writing Properties

| Command | Description |
|---------|-------------|
| `get #OBJ.PROP` | Read a property value |
| `set #OBJ.PROP = VALUE` | Write a property value |
| `props #OBJ` | List all properties on an object |

**Examples:**
```moo
(#2): get #1.name
#1.name = "Root Object"

(#2): get $player.name
$player.name = "Wizard"

(#2): set #1.description = "The root of all objects"
✓ Property #1.description set successfully

(#2): props $player
# Properties on #2

|Property|
|--------|
|name    |
|description|
|...     |

37 properties
```

**Tab Completion:** When typing property names, you can press Tab to see available properties:
```moo
(#2): get #1.<TAB>
name  description  programmer  wizard  ...
```

### Working with Verbs

| Command | Description |
|---------|-------------|
| `verbs #OBJ` | List all verbs on an object |
| `list #OBJ:VERB` | Display the code of a verb |
| `prog #OBJ:VERB` | Program a verb (multi-line editor) |

**Tab Completion:** When typing verb names, you can press Tab to see available verbs:
```moo
(#2): list #1:<TAB>
initialize  recycle  set_name  title  ...

(#2): list #1:init<TAB>
(#2): list #1:initialize
```

### Object Import/Export

| Command | Description |
|---------|-------------|
| `dump #OBJ [--file PATH]` | Dump object definition to file or console |
| `load [--file PATH] [options]` | Load object from objdef format |
| `reload [#OBJ] [--file PATH]` | Replace object contents completely |

#### Dumping Objects

The `dump` command exports an object's complete definition in objdef format:

**Examples:**
```moo
(#2): dump #1
# Object Definition: #1

object #1 "System Object"
  flags programmer wizard
  parent #-1
  property name "System Object" #1 r
  property description "" #1 rw
  ...
endobj

42 lines dumped

(#2): dump #1 --file system.moo
✓ Object #1 dumped to system.moo

42 lines written
```

#### Loading Objects

The `load` command imports objects from objdef format with flexible conflict handling:

**Basic Usage:**
```moo
(#2): load --file package.moo
✓ Object #123 loaded successfully

156 lines processed

(#2): load --file new-feature.moo --as new
✓ Object #201 loaded successfully

89 lines processed
```

**Load Options:**
- `--file PATH` - Load from file instead of stdin
- `--constants PATH` - MOO file with constant definitions for compilation
- `--dry-run` - Validate without making changes
- `--conflict-mode MODE` - How to handle conflicts: `clobber`, `skip`, or `detect`
- `--as SPEC` - Where to load: `new`, `anonymous` (or `anon`), `uuid`, or `#OBJ`
- `--return-conflicts` - Return detailed conflict information

**Advanced Examples:**
```moo
(#2): load --file obj.moo --dry-run
⚠ Load would have conflicts (dry-run or detect mode)

Would load: 1
Conflicts: 3

156 lines processed

(#2): load --file obj.moo --as #123
✓ Object #123 loaded successfully

156 lines processed

(#2): load --file obj.moo --as anonymous
✓ Object 01234567-89ab-cdef-0123-456789abcdef loaded successfully

156 lines processed

(#2): load --file package.moo --constants defs.moo --conflict-mode skip
✓ Object #150 loaded successfully

203 lines processed
```

**Interactive Loading (from stdin):**
```moo
(#2): load
**Loading object definition**

Paste object definition (type . on a line by itself to finish):
>> object #1 "System"
>>   property name "System" #0 r
>> endobject
>> .
✓ Object #1 loaded successfully

3 lines processed
```

#### Reloading Objects

The `reload` command completely replaces an object's contents, removing all properties and verbs not in the new definition:

**Examples:**
```moo
(#2): reload --file updated-core.moo
✓ Object #1 reloaded successfully

234 lines processed

(#2): reload #123 --file feature.moo
✓ Object #123 reloaded successfully

156 lines processed

(#2): reload --file package.moo --constants shared-defs.moo
✓ Object #45 reloaded successfully

189 lines processed
```

**When to Use Each Command:**

- **`dump`** - Export objects for version control, backup, or sharing
- **`load`** - Import new objects or merge updates into existing objects with conflict control
- **`reload`** - Completely replace an object's definition, removing obsolete properties/verbs

**Important Notes:**

- `reload` is destructive - it removes anything not in the new definition
- Use `load --dry-run` to preview changes before applying them
- The `--constants` flag allows you to share common definitions across multiple objects
- Object IDs can be inferred from the objdef file or explicitly specified with `--as #OBJ`

### Switching User Context

| Command | Description |
|---------|-------------|
| `su #OBJ` | Switch to a different player object |
| `su $property` | Switch to player referenced by #0.property |

The `su` command allows you to change the wizard/player object you're operating as. This is useful when you need to test permissions, debug player-specific issues, or perform operations as a different user.

**Requirements:**
- The target object must exist in the database
- The target object must have the User flag set (must be a player object)

**Object Reference Formats:**
- `#123` - Direct object ID reference
- `$player` - Property reference (looks up `#0.player`)

**Examples:**
```moo
(#2): su #3
✓ Switched to player #3

(#3): get #3.name
#3.name = "Programmer"

(#3): su $wiz
✓ Switched to player #2

(#2):
```

**Note:** The prompt updates to show the current wizard object you're operating as. Property references (`$name`) work by looking up the property on system object `#0`.

### Object ID Completion

When typing object IDs, press Tab to see available objects:
```moo
(#2): props #<TAB>
#0  #1  #2  #3  #4  #5  ...

(#2): props #1<TAB>
#1  #10  #11  #12  #13  ...
```

## Tab Completion Features

`moor-emh` provides comprehensive tab completion to make navigation easier:

- **Commands:** Type the beginning of a command and press Tab
- **Object IDs:** Type `#` followed by Tab to see all objects (works with `props`, `verbs`, `list`, `prog`, `dump`, `reload`, and `su`)
- **Properties:** Type `get #OBJ.` or `set #OBJ.` and press Tab
- **Verbs:** Type `list #OBJ:` or `prog #OBJ:` and press Tab
- **File Paths:** Type `--file ` or `--constants ` and press Tab to complete file paths
- **Flags:** Type `--` and press Tab to see available flags for `dump`, `load`, and `reload` commands

The completion system queries the database in real-time, so you always see the current state of your MOO.

## Safety and Best Practices

### Data Safety

- **Exclusive Lock:** moor-emh locks the database to prevent corruption. Shut down your mooR server first.
- **Auto-save:** Changes are automatically saved to the database. There is no "undo" feature.
- **Backups:** Always make a backup before performing emergency repairs:
  ```bash
  cp -r moor-data moor-data.backup.$(date +%Y%m%d-%H%M%S)
  ```

### Common Tasks

**Reset a corrupted wizard password:**
```moo
(#2): set #2.password = ""
```

**Find all wizard objects:**
```moo
(#2): ;;objs = children(#0); results = {}; for o in (objs) if (o.wizard) results = {@results, o}; endif endfor return results;
=> {#2, #3}
```

**Check database integrity:**
```moo
(#2): ;length(children(#0))
=> 156

(#2): ;db_disk_size()
=> 2048576
```

**List all objects with a specific property:**
```moo
(#2): ;;objs = children(#0); results = {}; for o in (objs) if ("owner" in (properties(o))) results = {@results, o}; endif endfor return results;
=> {#1, #2, #5, #10}
```

**Export an object for version control:**
```moo
(#2): dump #1 --file system-object.moo
✓ Object #1 dumped to system-object.moo

234 lines written
```

**Update an object from a file:**
```moo
(#2): reload #1 --file system-object.moo
✓ Object #1 reloaded successfully

234 lines processed
```

**Import a package with conflict detection:**
```moo
(#2): load --file new-package.moo --dry-run --return-conflicts
⚠ Load would have conflicts (dry-run or detect mode)

Would load: 1
Conflicts: 2

156 lines processed
```

**Create a new object from a template:**
```moo
(#2): load --file feature-template.moo --as new
✓ Object #201 loaded successfully

89 lines processed
```

**Load with shared constants:**
```moo
(#2): reload #45 --file package.moo --constants shared-defs.moo
✓ Object #45 reloaded successfully

189 lines processed
```

## Terminal Features

The tool uses `termimad` for beautiful terminal output with:

- **Styled Markdown:** Headers, tables, and code blocks are nicely formatted
- **Color-coded Output:** Headers in yellow, bold text in cyan, italic in green
- **Tables:** Properties and verbs are displayed in clean, readable tables
- **Syntax Highlighting:** MOO code is displayed in formatted code blocks

## Troubleshooting

**"Failed to acquire lock on data directory"**
- Another mooR process is running. Shut down the server first.
- Or another moor-emh instance is already running.

**"No wizard objects found in database"**
- The database may be corrupted or empty.
- Use `--wizard` to specify a specific object ID to run as.

**"Database file not found"**
- Check that the path to your data directory is correct.
- Use `--db` to specify the correct database filename.

**Tab completion not working:**
- Ensure you have sufficient permissions to read the database.
- Check that the database is not corrupted.

## Technical Details

### Database Access

- Opens the database in read-write mode with full access
- Creates a fresh transaction for each tab completion request
- Runs all commands as the wizard character found at startup
- Maintains two database instances: one for the scheduler, one for completion

### Architecture

The tool consists of:
- **REPL Loop:** Built on `rustyline` with command history and editing
- **Scheduler:** Executes MOO code through the kernel's task scheduler
- **Tab Completion:** Real-time database queries for context-aware completion
- **Terminal Rendering:** Markdown formatting via `termimad`

### Session Management

- Creates `ConsoleSession` instances for command output
- Buffers narrative events and prints on commit
- No connection tracking or player management
- All output goes directly to stdout/stderr

## Related Tools

- **moor-daemon:** The main MOO server daemon (handles textdump import and objdef checkpoint export)
- **moorc:** Command-line compiler for importing textdumps or objdef directories and running tests

## See Also

- [Object Packaging (dump/load/reload)](./object-packaging.md) - Detailed documentation on the objdef format
- [Server Configuration](./server-configuration.md)
- [Server Assumptions About the Database](./server-assumptions-about-the-database.md)
- [Controlling the Execution of Tasks](./controlling-the-execution-of-tasks.md)
