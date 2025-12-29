# mdmoot: Markdown-Based MOO Testing Framework

**Date:** 2025-12-29
**Status:** Design

## Overview

mdmoot is a redesign of the moot testing framework, treating test files as **living specification documents** that happen to be executable. It uses markdown as the primary format, supports multi-implementation comparison for establishing ground truth, and provides both CLI and web-based runners.

## Goals

1. **Specification-first** - Tests are readable documentation, not just executable code
2. **Multi-implementation comparison** - Compare moor against LambdaMOO, ToastStunt, etc. to validate behavior
3. **Variable persistence** - Named bindings that flow across test blocks (solving a key limitation of moot 1.0)
4. **Tabular testing** - Decision tables and script tables for data-driven tests
5. **Golden capture** - Discover behavior by running against implementations, then codify as specs

## File Format

### Extension

`.spec.md` - Emphasizes these are specifications that happen to be executable.

### Frontmatter

```yaml
---
tags: [strings, core, fast]
compare: [moor, lambdamoo]
---
```

- `tags` - For filtering test runs
- `compare` - Which implementations to run against (overrides config default)

## REPL Blocks

Interactive-style blocks for sequential MOO evaluation:

````markdown
```moot
@wizard
> x = create($nothing);
> return x.owner;
$wizard_player
```
````

### Syntax

| Element | Meaning |
|---------|---------|
| `@wizard`, `@programmer` | Switch player context |
| `> expr` | Evaluate expression |
| `value` | Expected result |
| `# comment` | Comment line |

### Divergence Annotations

When implementations differ, annotate inline:

````markdown
```moot
> return tofloat("inf");
inf  # !lambdamoo: E_INVARG  !toaststunt: E_INVARG
```
````

The primary expected value is listed first. `!impl: value` marks known divergences for specific implementations.

### Golden Annotations

Values captured via `mdmoot golden` are marked:

````markdown
```moot
> return server_version();
"moor 0.1"  <!-- golden:moor:2025-12-29 -->
```
````

## Named Bindings

Blocks can bind variables for use in later blocks:

````markdown
```moo @wizard bind: fixtures
$parent = create($nothing);
$child = create($parent);
```

Later blocks can reference `$parent` and `$child`:

```moot
> return parent($child);
$parent
```
````

### Scoping

Bindings follow **hierarchical markdown scoping** - they flow down the document structure:

````markdown
# String Functions                    ← top-level scope

## Setup                              ← bindings available to siblings & children
```moo @wizard bind: test_data
$str = "hello world";
```

## Basic Operations                   ← inherits $str

### Decision: length                  ← inherits $str
| _ | length(_)? |
|---|------------|
| `$str` | `11` |

## Edge Cases                         ← inherits from # but NOT from ## Basic Operations
````

### Isolation and Reset

- **Section isolation** - Add `(isolate)` to section heading:
  ```markdown
  ### Decision: error cases (isolate)
  ```

- **Mid-section reset** - Use `reset` keyword:
  ````markdown
  ```moo reset
  // fresh state from here
  ```
  ````

## Decision Tables

For independent, data-driven test cases. Each row executes in isolation.

````markdown
| _        | length(_)? | typeof(_)? |
|----------|------------|------------|
| `"foo"`  | `3`        | `STR`      |
| `""`     | `0`        | `STR`      |
| `{1,2}`  | `2`        | `LIST`     |
````

**Empty cells:** Re-execute the column expression (fresh value).

## Script Tables

For sequential test cases where state flows between rows.

````markdown
| Script: object lifecycle |                       |              |
| obj: create($nothing)    | action                | obj.owner?   |
|--------------------------|---------------------- |--------------|
|                          | `chown(obj, $player)` | `$player`    |
|                          | `chown(obj, $wizard)` | `$wizard`    |
````

**Empty cells:** Carry forward previous row's value.

## Column Conventions

| Convention | Meaning |
|------------|---------|
| `name?` | Output column - check result against expected |
| `#name` | Comment column - documentation only |
| `name:` or `name: expr` | Binding column - result bound to `name` |
| `_` | Placeholder for row's primary value |
| `_` in expression | Template - e.g., `length(_)?`, `_.owner?` |

### Code Generation

Columns generate MOO code dynamically:

- **Known builtins** (`parent`, `children`, `typeof`, `length`) → `builtin(obj)`
- **Property syntax** (`.owner`, `.name`) → `obj.prop`
- **Templates** (`_.foo?`, `move(_, $room)`) → literal expression with `_` substituted

### Multiple Bindings

For tables needing multiple objects, use named binding columns:

````markdown
| obj: create($nothing) | room: create($nothing) | move(obj, room)? | obj.location? |
|-----------------------|------------------------|------------------|---------------|
|                       |                        | `0`              | `room`        |
|                       | `$first_room`          | `0`              | `$first_room` |
````

## Configuration

### mdmoot.toml

```toml
[project]
root = "specs/"
default_impl = "moor"

[implementations.moor]
handler = "in-process"

[implementations.lambdamoo]
handler = "telnet"
host = "localhost"
port = 7777

[implementations.toaststunt]
handler = "telnet"
host = "localhost"
port = 7778

[server]
port = 8080
```

Located in project root or parent directories.

## CLI

```bash
# Run tests
mdmoot test                              # run all specs
mdmoot test --tags "strings,core"        # filter by tags
mdmoot test --tags "!slow"               # exclude tags
mdmoot test strings.spec.md              # specific spec
mdmoot test --compare moor,lambdamoo     # compare implementations

# Output formats
mdmoot test --format summary             # pass/fail counts
mdmoot test --format detailed            # full diff output
mdmoot test --format json                # machine readable
mdmoot test --format html -o report.html

# Capture golden outputs
mdmoot golden                            # all specs
mdmoot golden strings.spec.md            # specific spec
mdmoot golden --impl lambdamoo           # capture from specific impl

# Web runner
mdmoot serve                             # start web UI

# Interactive REPL
mdmoot repl                              # against default impl
mdmoot repl --compare moor,lambdamoo     # side-by-side comparison

# Validate syntax
mdmoot check                             # validate all specs
mdmoot check specs/                      # specific directory

# Migration from moot 1.0
mdmoot migrate                           # convert .moot → .spec.md
mdmoot migrate crates/kernel/testsuite/  # specific directory
mdmoot migrate --dry-run                 # preview only
```

## Web Runner

A wiki-style web interface with:

1. **View + Run** - Browse specs, execute tests, see results
2. **Edit** - Modify specs directly in browser, save to filesystem
3. **Interactive REPL** - Scratch area to try MOO expressions against running implementations
4. **File watching** - Reload when specs change on disk (external editor support)
5. **Conflict detection** - Warn if file changed since loaded

## Migration from moot 1.0

The `mdmoot migrate` command converts existing `.moot` files:

**Conversions:**
- `; expr` → `> expr` in moot blocks
- `@wizard`, `@programmer` preserved as player switches
- Assertions converted to expected values
- Related tests grouped under markdown headings
- Frontmatter with tags inferred from directory structure

**Tabular Pattern Detection:**

The migrator detects repetitive patterns that should become tables:

1. **Type checking patterns** - `typeof(x) == TYPE` becomes:
   | _ | typeof(_)? |
   |---|------------|
   | `3` | `INT` |
   | `"abc"` | `STR` |

2. **Conversion function patterns** - `tostr(x)`, `toint(x)`, etc. become:
   | _ | tostr(_)? | toliteral(_)? |
   |---|-----------|---------------|
   | `17` | `"17"` | `"17"` |
   | `{1,2}` | `"{list}"` | `"{1, 2}"` |

3. **Comparison patterns** - `equal(a, b)`, `==`, `<`, `>`, etc. become:
   | a | b | equal(a, b)? | a < b? | a > b? |
   |---|---|--------------|--------|--------|

4. **Arithmetic patterns** - binary operations with consistent structure:
   | a | b | a + b? | a - b? | a * b? | a / b? | a % b? |
   |---|---|--------|--------|--------|--------|--------|

5. **Bitwise operations** - `&.`, `|.`, `^.`, `<<`, `>>`:
   | a | b | a &. b? | a \|. b? | a ^. b? |
   |---|---|---------|----------|---------|

6. **String function patterns** - `index`, `rindex`, `strsub`, `strcmp`:
   | haystack | needle | index(haystack, needle)? |
   |----------|--------|--------------------------|
   | `"foobar"` | `"bar"` | `4` |

7. **List operation patterns** - `listappend`, `listdelete`, `setadd`:
   | list | listappend(list, 3)? | length(list)? |
   |------|----------------------|---------------|

8. **Object property patterns** - consecutive property/builtin access on same object type:
   | obj | obj.owner? | parent(obj)? | children(obj)? |
   |-----|------------|--------------|----------------|

9. **Error case patterns** - repeated calls expecting errors:
   | expr | expected? |
   |------|-----------|
   | `random(0)` | `E_INVARG` |
   | `random(-1)` | `E_INVARG` |

10. **Boolean result patterns** - Repeated `; return expr; 1` or `; return expr; 0` suggests a decision table

The migrator groups consecutive similar patterns into tables automatically.

**Behavior:**
- Creates new `.spec.md` files alongside `.moot` files
- Never deletes original files (user decides via git)
- Best-effort conversion - manual cleanup expected

## Example Spec

````markdown
---
tags: [objects, core]
compare: [moor, lambdamoo]
---

# Object Creation and Hierarchy

This spec validates object creation, parent/child relationships, and ownership.

## Setup

```moo @wizard bind: fixtures
$parent = create($nothing);
add_property($parent, "inherited_prop", "default", {player, "rwc"});
```

## Creating Objects

```moot
@wizard
> $child = create($parent);
> return parent($child);
$parent
> return children($parent);
{$child}
```

## Property Inheritance

### Decision: inherited property access

| obj: create($parent) | obj.inherited_prop? |
|----------------------|---------------------|
|                      | `"default"`         |

### Script: property override

| Script: override flow |                              |                     |
| obj: create($parent)  | action                       | obj.inherited_prop? |
|-----------------------|------------------------------|---------------------|
|                       |                              | `"default"`         |
|                       | `obj.inherited_prop = "new"` | `"new"`             |

## Edge Cases (isolate)

Error handling for invalid operations:

```moot
@programmer
> create($nothing, $wizard);
E_PERM  # !lambdamoo: E_INVARG
```
````

## Implementation Notes

### Handlers

Each implementation needs a handler that can:
- Connect/initialize
- Execute MOO code
- Return results
- Clean up

Handler types:
- `in-process` - Direct Rust API (moor)
- `telnet` - Network connection (LambdaMOO, ToastStunt)
- Custom handlers via plugin interface (future)

### Parser

The spec parser needs to:
1. Parse markdown structure (headings, code blocks, tables)
2. Extract frontmatter
3. Identify block types (moot, moo, decision, script)
4. Parse table structure and column conventions
5. Track scoping for bindings

### Executor

The test executor:
1. Loads config and spec files
2. Initializes requested implementations
3. Executes blocks/tables in document order
4. Tracks bindings per scope
5. Compares results, handles divergence annotations
6. Generates output in requested format
````
