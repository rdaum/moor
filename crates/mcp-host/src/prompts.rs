// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! MCP prompts for MOO syntax and programming guidance

use crate::mcp_types::{Prompt, PromptMessage, PromptsListResult};

/// Get list of available prompts
pub fn get_prompts() -> Vec<Prompt> {
    vec![
        Prompt {
            name: "moo_language".to_string(),
            description: Some(
                "MOO programming language syntax: variables, control flow, data structures, operators, error handling".to_string(),
            ),
            arguments: vec![],
        },
        Prompt {
            name: "moo_object_model".to_string(),
            description: Some(
                "MOO object model: prototype OO, objects, verbs, properties, commands vs methods".to_string(),
            ),
            arguments: vec![],
        },
        Prompt {
            name: "moo_permissions".to_string(),
            description: Some(
                "MOO permissions model: task perms, set_task_perms, caller_perms, wizards, privilege requirements".to_string(),
            ),
            arguments: vec![],
        },
    ]
}

/// Get prompt content by name
pub fn get_prompt(name: &str) -> Option<PromptsListResult> {
    match name {
        "moo_language" => Some(PromptsListResult {
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: MOO_LANGUAGE_PROMPT.to_string(),
            }],
        }),
        "moo_object_model" => Some(PromptsListResult {
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: MOO_OBJECT_MODEL_PROMPT.to_string(),
            }],
        }),
        "moo_permissions" => Some(PromptsListResult {
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: MOO_PERMISSIONS_PROMPT.to_string(),
            }],
        }),
        _ => None,
    }
}

const MOO_LANGUAGE_PROMPT: &str = r#"# MOO Language Syntax Reference

MOO is a Wirth-style language with 1-based indexing (lists/strings start at 1, not 0).

**CRITICAL - Syntax backwards from Python:**
- MOO lists: `{ "a", "b", "c" }`       // curly braces!
- MOO maps:  `[ "key" -> "value" ]`   // square brackets with arrows!
This is OPPOSITE of Python where [] is lists and {} is dicts.

## Variables and Scoping

```moo
x = 5;                 // verb-global (classic MOO)
let y = 10;            // lexically scoped, mutable
const z = 15;          // lexically scoped, immutable
global w = 20;         // explicitly global scope

// Strings use double quotes
name = "Alice";
greeting = "Hello, " + name;
```

## Control Flow (Wirth-style with end keywords)

```moo
// If/elseif/else/endif
if (x > 0)
  return "positive";
elseif (x < 0)
  return "negative";
else
  return "zero";
endif

// While loops
while (count < 10)
  count = count + 1;
endwhile

// For-in loops (iterate over lists)
for item in ({"a", "b", "c"})
  notify(player, item);
endfor

// For-range loops (1-indexed!)
for i in [1..10]
  total = total + i;
endfor

// For-in with index
for i, item in ({"apple", "banana", "cherry"})
  notify(player, tostr(i) + ": " + item);
endfor
```

## Error Handling

```moo
// Try/except/endtry
try
  result = risky_operation();
except e (E_INVARG, E_RANGE)
  notify(player, "Operation failed: " + tostr(e));
  return E_NONE;
endtry

// Try/finally/endtry
try
  lock_resource();
  do_work();
finally
  unlock_resource();
endtry

// Inline try with backticks (very common pattern)
value = `risky_expr ! E_PROPNF => default_value';
valid_obj = `obj.parent ! ANY => #-1';
```

## Data Structures (1-indexed!)

```moo
// Lists use CURLY BRACES { }
items = {"first", "second", "third"};
first = items[1];           // "first" (not 0!)
last = items[$];            // "third" ($ = end)
slice = items[2..$];        // {"second", "third"}

// Maps use SQUARE BRACKETS [ ] with arrows ->
config = ["host" -> "example.com", "port" -> 8080];
host = config["host"];
config["timeout"] = 30;

// Flyweights (lightweight immutable mini-objects)
point = <$point, .x = 10, .y = 20>;      // $point is delegate, x/y are slots
password_obj = <$password, {"hash..."}>; // list content
```

## Functions and Lambdas

```moo
// Lambda expressions
double = {x} => x * 2;
result = double(5);  // 10

// Named functions (local to verb)
fn calculate_area(width, height)
  return width * height;
endfn

// List comprehensions
squares = {x * x for x in [1..10]};
evens = {x for x in [1..20] if x % 2 == 0};
```

## Scatter Assignment (destructuring)

```moo
{a, b, c} = {1, 2, 3};
{first, @rest} = {1, 2, 3, 4};  // first=1, rest={2,3,4}
{x, ?y = 10} = {5};             // x=5, y=10 (default)
```

## Operators

- Arithmetic: `+ - * / % ^` (power)
- Comparison: `== != < > <= >=`
- Logical: `&& || !`
- Bitwise: `&. |. ^. ~ << >> >>>`
- Ternary: `condition ? true_val | false_val`
- Range test: `x in [1..10]`

**IMPORTANT:** `in` operator returns POSITION (1-indexed), not boolean!
```moo
pos = "y" in "xyz";        // Returns 2 (position)
pos = 3 in {1, 2, 3};      // Returns 3 (position)
pos = "z" in "abc";        // Returns 0 (not found)
```

## Special Values

```moo
// Objects
#0          // System object
#123        // Object by number
$login      // System reference ($name = #0.name)

// Error types
E_NONE E_TYPE E_DIV E_PERM E_PROPNF E_VERBNF E_VARNF E_INVIND
E_RECMOVE E_MAXREC E_RANGE E_ARGS E_NACC E_INVARG E_QUOTA E_FLOAT

// Custom errors with messages
raise(E_INVARG("Expected positive number"));

// Booleans
true false

// Symbols (Lisp-style keywords)
'success 'failure 'pending
```

## Type Constants

`TYPE_INT TYPE_NUM TYPE_FLOAT TYPE_STR TYPE_OBJ TYPE_LIST TYPE_MAP TYPE_ERR TYPE_BOOL TYPE_FLYWEIGHT TYPE_BINARY TYPE_LAMBDA TYPE_SYM`

**CRITICAL:** Type constants CANNOT be used as variable names!
```moo
// BAD - compile error:
TYPE_INT = 42;
for TYPE_OBJ in (list)

// GOOD:
int_value = 42;
for obj in (list)
```

## Editing Verbs and Objdefs

- For large verbs, prefer `moo_apply_patch_verb` with a unified diff to avoid resending full source.
- For object definitions, use `moo_apply_patch_objdef` to patch in-memory; this avoids filesystem access (handy in containers).
- Use `moo_get_verb` or `moo_dump_object` first if you need the current source to build the patch.

## Getting Help

Use the `moo_function_help` tool to get documentation for any builtin function.

## Reference

- Full grammar: https://raw.githubusercontent.com/rdaum/moor/refs/heads/main/crates/compiler/src/moo.pest
- Documentation: https://timbran.codeberg.page/moor-book-html/
"#;

const MOO_OBJECT_MODEL_PROMPT: &str = r#"# MOO Object Model Reference

MOO uses prototype-based object orientation. There are no classes - objects inherit directly from other objects.

## Objects

Every object has a number (`#0`, `#123`) or UUID (`#0000-0000-0000`).

**System references:** `$name` translates to `#0.name` at runtime. So `$room` means "the object stored in #0.room".

**Special objects:**
- `#0` - System object (lobby for utilities and common names)
- `#1` - Root object (prototype for all objects, parent is `#-1`)
- `#-1` aka `$nothing` - Null/nothing

## Builtin Object Properties

All objects have these server-provided properties:
- `.name` (string) - object name
- `.owner` (object) - who controls access
- `.location` (object) - where it physically is (read-only, use `move()`)
- `.contents` (list) - objects inside (read-only, modified by `move()`)
- `is_player(obj)` (function) - checks if object is a player
- `.programmer` (bool) - has programmer rights
- `.wizard` (bool) - has superuser rights
- `.r` (bool) - publicly readable
- `.w` (bool) - publicly writable
- `.f` (bool) - fertile (can be used as parent)

## Inheritance vs Containment (Critical!)

**Inheritance (prototype chain):**
- `parent(obj)` - builtin, returns prototype parent
- `children(obj)` - builtin, returns objects inheriting from this
- `ancestors(obj)` / `descendants(obj)` - full chains

**Spatial/Containment (physical location):**
- `obj.location` - PROPERTY, where object physically is
- `obj.contents` - PROPERTY, list of objects inside
- Managed by `move(obj, destination)` builtin

**DO NOT confuse these!** `parent()` ≠ `.location`, `children()` ≠ `.contents`

## Properties

User-defined data on objects. Accessed with dot notation: `obj.prop`

**Property flags:** `r` (read), `w` (write), `c` (chown)

Properties inherit down the prototype chain. Child objects can override inherited values.

```moo
// Define property on object
add_property(obj, "health", 100, {owner, "rc"});

// Access
current = player.health;
player.health = 50;
```

## Verbs

Code attached to objects. Called with colon notation: `obj:verb(args)`

**Verb declaration:**
```
verb <names> (<dobj> <prep> <iobj>) owner: <owner> flags: "<flags>"
```

**Argument specifiers:**
- `this` - must be the verb's container object
- `none` - must be absent ($nothing)
- `any` - any object or $nothing

**Verb flags:**
- `r` = readable (code visible) - USE ON EVERYTHING
- `d` = debug (errors propagate as exceptions) - USE ON EVERYTHING
- `w` = writable (others can modify) - RARE
- `x` = executable via `obj:verb()` syntax

## Methods vs Commands

**Methods** - called programmatically as `obj:method(args)`:
- Argspec: `(this none this)`
- Flags: `"rxd"`
- Example: `verb calculate (this none this) owner: #2 flags: "rxd"`

**Commands** - matched from user input like "look at box":
- Argspec: varies, e.g. `(any at any)`, `(this none none)`
- Flags: `"rd"` (NO x flag!)
- Example: `verb "look l*" (any none none) owner: #2 flags: "rd"`

The parser finds verbs by matching verb names and argspecs against user input.

## Code Style

**Prefer early returns - avoid deep nesting:**
```moo
// Good
!valid(obj) && raise(E_INVARG);
caller.wizard || raise(E_PERM);
typeof(arg) != LIST && raise(E_TYPE);
// Main logic here, unindented

// Bad - deeply nested
if (valid(obj))
  if (caller.wizard)
    if (typeof(arg) == LIST)
      // Main logic buried
    endif
  endif
endif
```

**Use short-circuit expressions:**
```moo
caller == this || raise(E_PERM);
valid(target) || return E_INVARG;
length(args) > 0 && process(args);
```

## Common Builtins

```moo
// Object operations
create(parent)              // Create child object
recycle(obj)                // Destroy object
move(obj, destination)      // Move object
valid(obj)                  // Check if object exists
parent(obj) / children(obj) // Inheritance
ancestors(obj) / descendants(obj)

// Verb/property introspection
verbs(obj)                  // List verb names
verb_code(obj, verb)        // Get verb source
properties(obj)             // List property names

// Output
notify(player, message)     // Send text to player
```

## Getting Help

Use the `moo_function_help` tool to get documentation for any builtin function.
"#;

const MOO_PERMISSIONS_PROMPT: &str = r#"# MOO Permissions Model

MOO has a capability-based security model. Objects, properties, and verbs each have owners (which need not be the same), plus permission flags that control access.

## Ownership

**Objects** have an owner (the `.owner` property). Only the owner or a wizard can modify the object's flags or add/remove properties and verbs.

**Properties** have an owner. The initial owner is whoever added the property (usually, but not always, the object's owner). Only a wizard can change a property's owner.

**Verbs** have an owner. The verb owner can change its code, flags, and argument specifiers. Only a wizard can change a verb's owner.

**Important:** The owner of an object may not own every property or verb on that object!

## Task Permissions

When a verb runs, it executes with "task permissions" - a player object that determines what operations are allowed. The task perms are set to the **owner of the verb**.

```moo
// This verb runs with #2's permissions (a wizard)
verb do_something (this none this) owner: #2 flags: "rxd"
  // Can do anything #2 can do
```

**Warning:** Wizard-owned verbs must be written carefully - they can do almost anything, bypassing most permission checks.

## set_task_perms(who)

```
none set_task_perms(obj who)
```

Changes the permissions for the currently-executing verb to those of `who`. Raises `E_PERM` if the programmer is neither `who` nor a wizard.

**Key points:**
- Only **downgrades** permissions (wizard → player), never upgrades
- Only affects the **current verb frame**, not verbs it calls
- Does NOT change the verb's owner, just this invocation's permissions

```moo
verb safe_action (this none this) owner: #2 flags: "rxd"
  // Running as wizard (#2)
  set_task_perms(player);  // Now running as player
  // Called verbs still get THEIR owner's perms
```

## caller_perms()

```
obj caller_perms()
```

Returns the permissions in use by the verb that called the current verb. Returns `#-1` if this is the first verb in a command/task (no caller).

```moo
verb check_access (this none this) owner: #2 flags: "rxd"
  caller_perms().wizard || raise(E_PERM);
```

## Object Builtin Properties (Flags)

Objects have builtin properties that control permissions and capabilities. These are accessed
as regular properties (e.g., `obj.programmer`) or via functions (e.g., `is_player(obj)`):

| Property | Type | Meaning |
|----------|------|---------|
| `is_player(obj)` | function | Object is a player/user (can login) |
| `.programmer` | bool | Can write and execute code |
| `.wizard` | bool | Superuser - bypasses most permission checks |
| `.r` | bool | Publicly readable (anyone can see property values) |
| `.w` | bool | Publicly writable (anyone can modify properties) |
| `.f` | bool | Fertile (can be used as parent for new objects) |

```moo
// Example: check and set flags
obj.programmer = 1;
if (obj.wizard)
  notify(player, "This is a wizard!");
endif
```

## Property Flags

| Flag | Name | Meaning |
|------|------|---------|
| `r` | Read | Non-owners can get the value |
| `w` | Write | Non-owners can set the value |
| `c` | Chown | Descendants inherit with child's owner (see below) |

Example: `"rw"` = readable and writable by anyone.

### The `c` (chown) Flag

When an object inherits a property from its parent:
- If `c` is set (default): the child's owner becomes the property owner
- If `c` is NOT set: the property keeps its parent's owner

**Example - Password property:** To prevent anyone but wizards from reading/writing passwords, a wizard owns the `password` property on the player prototype with flags `""` (no r, w, or c). Because `c` is not set, the wizard remains owner on all player descendants.

**Example - Radio channel:** A verb to change a radio's channel runs as its author (Ford). If the `channel` property has `c` set, the radio's owner (yduJ) owns that property - Ford's verb can't change it! Fix: set property flags to just `"r"` (no `c`), so Ford remains owner on descendants.

## Verb Flags

| Flag | Name | Meaning |
|------|------|---------|
| `r` | Read | Non-owners can see the code |
| `w` | Write | Non-owners can modify the code |
| `x` | Execute | Can be called from within another verb |
| `d` | Debug | Errors raise exceptions instead of returning values |

Example: `"rxd"` = readable, executable, debug enabled.

### The `x` (execute) Flag

Without `x`, a verb can only be invoked from the command line, not from code. Methods (`this none this`) need `x` to be called as `obj:method()`.

### The `d` (debug) Flag

**All new verbs should have `d` set.**

- With `d`: errors raise exceptions (catchable with try/except)
- Without `d`: errors return error values instead of raising

The `d` flag exists for historical reasons. Old code without `d` should be updated to use exception handling.

## Wizards

Wizards (`.wizard = true`) are superusers:
- Bypass most permission checks
- Can read/write any property
- Can modify any object, verb, or property
- Can change ownership
- Only wizards can grant wizard/programmer status

**Best practice:** Create a non-wizard programmer character for most coding. Use wizard only when truly needed.

## Common Patterns

```moo
// Check caller has permission
caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);

// Downgrade to player perms for safety
set_task_perms(player);

// Safe property access for non-readable props
value = `obj.secret ! E_PERM => "access denied"';
```

## Frame-Local Perms

`set_task_perms()` only affects the current frame:

```moo
verb outer (this none this) owner: #2 flags: "rxd"
  // perms = #2 (wizard)
  set_task_perms(player);
  // perms = player
  this:inner();  // inner() runs with ITS owner's perms
  // still perms = player here

verb inner (this none this) owner: #100 flags: "rxd"
  // perms = #100, NOT the player from outer()
```
"#;
