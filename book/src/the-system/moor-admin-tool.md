# Emergency Admin Tool (moor-admin)

The `moor-admin` tool is an emergency database administration utility for mooR. It provides direct database access when normal logins are unavailable, making it essential for database recovery, emergency repairs, and system administration tasks.

## Overview

Think of `moor-admin` as the Emergency Medical Hologram for your MOO database - it activates when you need immediate access to repair critical issues. The tool provides a REPL (Read-Eval-Print Loop) interface with full wizard privileges, allowing you to:

- Inspect and modify object properties
- View and edit verb code
- Execute MOO code directly
- List objects, properties, and verbs
- Perform emergency database repairs

## When to Use moor-admin

Use `moor-admin` when:

- Normal telnet/web access is broken or unavailable
- You need to recover from a catastrophic database error
- The wizard character is locked out or corrupted
- You need to perform emergency maintenance without starting the full server
- You want to inspect or modify the database in a safe, read-only context

**Important:** `moor-admin` acquires an exclusive lock on the database, preventing other mooR processes from accessing it. Always shut down your mooR server before using this tool.

## Running moor-admin

### Basic Usage

```bash
moor-admin [OPTIONS] [DATA-DIR]
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
moor-admin
```

Specify a custom data directory:
```bash
moor-admin /path/to/my/moo/data
```

Use a specific wizard object:
```bash
moor-admin --wizard 3
```

Use a different database file:
```bash
moor-admin --db backup.db /var/moor-data
```

## The REPL Interface

When you start moor-admin, you'll see the Emergency Medical Hologram welcome screen:

```
# Emergency Medical Hologram - Database Administration Subroutine

Please state the nature of the database emergency.

Running as wizard: #2

Type help for available commands or quit to deactivate.

(#2):
```

The prompt shows the wizard object you're operating as (e.g., `(#2):`).

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

(#2): set #1.description = "The root of all objects"
✓ Property #1.description set successfully

(#2): props #1
# Properties on #1

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

### Switching User Context

| Command | Description |
|---------|-------------|
| `su #OBJ` | Switch to a different player object |

The `su` command allows you to change the wizard/player object you're operating as. This is useful when you need to test permissions, debug player-specific issues, or perform operations as a different user.

**Requirements:**
- The target object must exist in the database
- The target object must have the User flag set (must be a player object)

**Example:**
```moo
(#2): su #3
✓ Switched to player #3

(#3): get #3.name
#3.name = "Programmer"

(#3): su #2
✓ Switched to player #2

(#2):
```

**Note:** The prompt updates to show the current wizard object you're operating as.

**Examples:**
```moo
(#2): verbs #1
# Verbs on #1

|Verb|
|----|
|initialize|
|recycle|
|set_name|
|...  |

37 verbs

(#2): list #1:initialize
# Verb: #1:initialize

```moo
if (this.location != #-1)
  this.location:announce_all_but(this.name + " has arrived.", this);
endif
```

7 lines
```

**Tab Completion:** When typing verb names, you can press Tab to see available verbs:
```moo
(#2): list #1:<TAB>
initialize  recycle  set_name  title  ...

(#2): list #1:init<TAB>
(#2): list #1:initialize
```

### Object ID Completion

When typing object IDs, press Tab to see available objects:
```moo
(#2): props #<TAB>
#0  #1  #2  #3  #4  #5  ...

(#2): props #1<TAB>
#1  #10  #11  #12  #13  ...
```

## Tab Completion Features

`moor-admin` provides comprehensive tab completion to make navigation easier:

- **Commands:** Type the beginning of a command and press Tab
- **Object IDs:** Type `#` followed by Tab to see all objects (works with `props`, `verbs`, `list`, `prog`, and `su`)
- **Properties:** Type `get #OBJ.` or `set #OBJ.` and press Tab
- **Verbs:** Type `list #OBJ:` or `prog #OBJ:` and press Tab

The completion system queries the database in real-time, so you always see the current state of your MOO.

## Safety and Best Practices

### Data Safety

- **Exclusive Lock:** moor-admin locks the database to prevent corruption. Shut down your mooR server first.
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

## Terminal Features

The tool uses `termimad` for beautiful terminal output with:

- **Styled Markdown:** Headers, tables, and code blocks are nicely formatted
- **Color-coded Output:** Headers in yellow, bold text in cyan, italic in green
- **Tables:** Properties and verbs are displayed in clean, readable tables
- **Syntax Highlighting:** MOO code is displayed in formatted code blocks

## Troubleshooting

**"Failed to acquire lock on data directory"**
- Another mooR process is running. Shut down the server first.
- Or another moor-admin instance is already running.

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

- **moor-daemon:** The main MOO server daemon
- **moor-db-load:** Load textdump files into the database
- **moor-db-dump:** Export database to textdump format

## See Also

- [Server Configuration](./server-configuration.md)
- [Server Assumptions About the Database](./server-assumptions-about-the-database.md)
- [Controlling the Execution of Tasks](./controlling-the-execution-of-tasks.md)
