# The Built-in Command Parser

MOO users usually nteract with the MOO server by typing commands at a prompt. The built-in command parser interprets these commands and determines which verb to execute, based on the command structure and the objects involved.

## What Are MOO Commands?

In a MOO environment, commands are how players interact with the virtual world. They range from simple social actions to complex object manipulation and system administration. The command parser's job is to break these natural-language-like commands into structured calls to MOO programs (verbs).

### Types of Commands

**Social and Communication Commands:**
```
say Hello, everyone!
"Hello, everyone!          (shorthand for 'say')
emote waves cheerfully.
:waves cheerfully.         (shorthand for 'emote')
```

**Object Interaction Commands:**
```
look                       (examine surroundings)
look at lamp               (examine specific object)
get lamp                   (pick up object - dobj only)
put lamp on table          (place object - dobj + prep + iobj)
give coin to merchant      (transfer object - dobj + prep + iobj)
unlock door with key       (use tool - dobj + prep + iobj)
```

**MOO System Commands:**
```
@create $thing named "lamp"     (create new object)
@describe me as "A helpful player"
@quit                           (disconnect)
@who                           (list connected users)
@eval 2 + 2                    (evaluate MOO expression)
;2 + 2                         (shorthand for '@eval')
```

> **Note on the `@` Symbol**: The `@` prefix for administrative and utility commands is a long-standing MUD convention that dates back to TinyMUD in the 1980s. It helps distinguish system commands from in-character actions. However, mooR itself treats `@` as just another character—the special meaning is purely a convention established by the programmers who write the database core and its verbs. You could just as easily have system commands named `admin_quit` or `sys_create`.

Each of these commands gets parsed into components:
- **Verb**: The action word (`say`, `get`, `put`, `@create`)
- **Direct Object**: The primary target (`lamp`, `coin`, `me`)
- **Preposition**: Relationship word (`on`, `to`, `with`, `at`)
- **Indirect Object**: Secondary target (`table`, `merchant`, `key`)

The parser then looks for a verb program that can handle this specific combination of verb name and argument pattern, turning your natural command into a structured program call.

---

## Overview: How Commands Are Parsed

When a player types a command, the server receives it as a line of text. The command can be as simple as a single word or more complex, involving objects and prepositions. The parser's job is to break down the command and match it to a verb that can be executed.

### 1. Special Cases (Handled Before Parsing)
- **Out-of-band commands**: Lines starting with a special prefix (e.g., `#$#`) are routed to `$do_out_of_band_command` instead of normal parsing.
- **.program and input holding**: If a `.program` command is in progress or input is being held for a read(), the line is handled accordingly.
- **Flush command**: If the line matches the connection's flush command (e.g., `.flush`), all pending input is cleared. No further processing occurs.

### 2. Initial Punctuation Aliases
If the first non-blank character is one of these:
- `"` → replaced with `say `
- `:` → replaced with `emote `
- `;` → replaced with `eval `

For example, `"Hello!` is treated as `say Hello!`.

### 3. Breaking Apart Words
The command is split into words:
- Words are separated by spaces.
- Double quotes can be used to include spaces in a word: `foo "bar baz"` → `foo`, `bar baz`
- Backslashes escape quotes or spaces within words.

### 4. Built-in Commands
If the first word is a built-in command (e.g., `.program`, `PREFIX`, `SUFFIX`, or the flush command), it is handled specially. Otherwise, normal command parsing continues.

### 5. Database Override: $do_command
Before the built-in parser runs, the server checks for a `$do_command` verb. If it exists, it is called with the command's words and the raw input. If `$do_command` returns a true value, no further parsing occurs. Otherwise, the built-in parser proceeds.

---

## The Command Parsing Steps

1. **Identify the verb**: The first word is the verb.
2. **Preposition matching**: The parser looks for a preposition (e.g., `in`, `on`, `to`) at the earliest possible place in the command. If found, words before it are the direct object, and words after are the indirect object. If not, all words after the verb are the direct object.
3. **Direct and indirect object matching**:
   - If the object string is empty, it is `$nothing` (`#-1`).
   - If it is an object number (e.g., `#123`), that object is used.
   - If it is `me` or `here`, the player or their location is used.
   - Otherwise, the parser tries to match the string to objects in the player's inventory and location.
   - **Aliases**: Each object may have an `aliases` property (a list of alternative names). The parser matches the object string against all aliases and the object's `name`. Exact matches are preferred over prefix matches. If multiple objects match, `$ambiguous_match` (`#-2`) is used. If none match, `$failed_match` (`#-3`) is used.

---

## How Verbs Are Matched

The parser now has:
- A verb string
- Direct and indirect object strings and their resolved objects
- A preposition string (if any)

It checks, in order, the verbs on:
1. The player
2. The room
3. The direct object (if any)
4. The indirect object (if any)

For each verb, it checks:
- **Verb name**: Does the command's verb match any of the verb's names? (Names can use `*` as a wildcard.)
- **Argument specifiers**:
  - `none`: The object must be `$nothing`.
  - `any`: Any object is allowed.
  - `this`: The object must be the object the verb is on.
- **Preposition specifier**:
  - `none`: Only matches if no preposition was found.
  - `any`: Matches any preposition.
  - Specific: Only matches if the found preposition is in the allowed set.

The first verb that matches all criteria is executed. If none match, the server tries to run a `huh` verb on the room. If that fails, it prints an error message.

---

## Variables Available to the Verb

When a verb is executed, these variables are set:

| Variable | Value |
|----------|----------------------------------------------------------|
| player   | the player who typed the command |
| this     | the object on which this verb was found |
| caller   | same as `player` |
| verb     | the first word of the command |
| argstr   | everything after the first word |
| args     | list of words in `argstr` |
| dobjstr  | direct object string |
| dobj     | direct object value |
| prepstr  | prepositional phrase found |
| iobjstr  | indirect object string |
| iobj     | indirect object value |

---

## Technical Note: Extending the Parser

> **Note:** mooR's command parser is implemented in Rust and can be extended by Rust programmers. This allows for custom parsing logic or new features beyond the standard MOO command syntax.

--
