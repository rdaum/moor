# Built-in Functions

## What are built-in functions?

Built-in functions (also called "builtins") are functions that are built directly into the MOO server itself, written in the server's implementation language (Rust for mooR). They provide essential functionality that would be impossible or impractical to implement in MOO code.

### How are they different from verb calls?

**Built-in functions:**
- Are called using simple function syntax: `length(my_list)`, `tostr(42)`
- Are implemented in the server's native code (Rust)
- Execute very quickly since they don't involve MOO code interpretation
- Provide core functionality like math, string manipulation, list operations, etc.
- Cannot be modified or overridden by MOO programmers

**Verb calls:**
- Are called using colon syntax: `player:tell("Hello")`, `#123:move(here)`
- Are implemented in MOO code by programmers
- May execute more slowly since they involve interpreting MOO code
- Provide game-specific functionality and can be customized
- Can be modified, added, or removed by programmers with appropriate permissions

**Examples:**
```moo
// Built-in functions:
len = length({"a", "b", "c"});        // Returns 3
str = tostr(42);                      // Returns "42"
result = sqrt(16.0);                  // Returns 4.0

// Verb calls:
player:tell("Welcome!");              // Calls the 'tell' verb on player object
sword:wield(player);                  // Calls the 'wield' verb on sword object
```

There are a large number of built-in functions available for use by MOO programmers. Each one is discussed in detail in
this section. The presentation is broken up into subsections by grouping together functions with similar or related
uses.

For most functions, the expected types of the arguments are given; if the actual arguments are not of these types,
`E_TYPE` is raised. Some arguments can be of any type at all; in such cases, no type specification is given for the
argument. Also, for most functions, the type of the result of the function is given. Some functions do not return a
useful result; in such cases, the specification `none` is used. A few functions can potentially return any type of value
at all; in such cases, the specification `value` is used.

Most functions take a certain fixed number of required arguments and, in some cases, one or two optional arguments. If a
function is called with too many or too few arguments, `E_ARGS` is raised.

Functions are always called by the program for some verb; that program is running with the permissions of some player,
usually the owner of the verb in question (it is not always the owner, though; wizards can use `set_task_perms()` to
change the permissions _on the fly_). In the function descriptions below, we refer to the player whose permissions are
being used as the _programmer_.

Many built-in functions are described below as raising `E_PERM` unless the programmer meets certain specified criteria.
It is possible to restrict use of any function, however, so that only wizards can use it; see the chapter on server
assumptions about the database for details.

To query documentation for any builtin function at runtime, use the [`function_help()`](built-in-functions/server.md#function_help) function. This returns documentation extracted directly from the running server's compiled code.
