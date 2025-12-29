# Error Handling in mooR

When you're programming in MOO, things can go wrong. You might try to divide by zero, access a property that doesn't exist, or call a verb with the wrong number of arguments. These situations create **errors**.

MOO handles errors in a unique way that might be different from other programming languages you know. Understanding how errors work will help you write more robust programs.

## What Causes Errors?

Here are some common situations that create errors:

- **Math problems**: Dividing by zero (`5 / 0` → `E_DIV`)
- **Type mismatches**: Adding a number to a string (`5 + "hello"` → `E_TYPE`)
- **Missing things**: Accessing a property that doesn't exist (`obj.nonexistent` → `E_PROPNF`)
- **Wrong arguments**: Calling a function with too many or too few arguments (`tostr()` → `E_ARGS`)
- **Permission problems**: Trying to do something you're not allowed to (`obj.owner = #123` → `E_PERM`)
- **Invalid operations**: Trying to move an object into itself (`#123:moveto(#123)` → `E_RECMOVE`)

## How MOO Handles Errors: Two Ways

MOO is unusual because it treats errors in two different ways:

### 1. Errors as Values (Like a Return Code)

Sometimes an error just becomes a special value that your program can check:

```moo
result = 5 / 0;  // This creates the error value E_DIV
if (typeof(result) == TYPE_ERR)
    player:tell("Oops, can't divide by zero!");
else
    player:tell("The answer is: ", result);
endif
```

### 2. Errors as Exceptions (Program Stops)

Other times, an error will immediately stop your program and print an error message:

```moo
result = 5 / 0;  // This might stop your program right here!
player:tell("This line might never run");
```

## The `d` Bit: What Controls This Behavior

Whether an error becomes a value or stops your program depends on something called the `d` bit on your verb. Think of it as a switch:

- **`d` bit OFF**: Errors become values you can check
- **`d` bit ON**: Errors can stop your program (but you can catch them)

> **Tip**: Almost all verbs should have the `d` bit turned on. The old way (turning it off) is error-prone and mainly exists for historical reasons.

## Handling Errors Gracefully

### The Best Way: try/except

The most reliable way to handle errors is with `try`/`except` blocks:

```moo
try
    result = player.score + 100;
    player:tell("Your new score is: ", result);
except err (E_PROPNF)
    player:tell("You don't have a score yet. Starting at 100!");
    player.score = 100;
except err (E_PERM)
    player:tell("You can't change your score!");
except err (ANY)
    player:tell("Something unexpected went wrong: ", err[2]);
endtry
```

This way, your program keeps running even when errors happen, and you can decide what to do about each type of error.

### Quick Fixes: Error-Catching Expressions

For simple cases, you can use a shortcut:

```moo
// If getting the score fails, use 0 instead
score = `player.score!E_PROPNF => 0`;
player:tell("Your score is: ", score);
```

### Checking Values (When d bit is off)

If you're working with older code that has the `d` bit turned off:

```moo
result = some_operation();
if (typeof(result) == TYPE_ERR)
    player:tell("That didn't work: ", error_message(result));
    return;
endif
// Continue with the result...
```

## mooR's Enhanced Errors

mooR adds some nice improvements to MOO's error system:

- **Custom error names**: You can create your own error types like `E_GAME_OVER` or `E_INVALID_MOVE`
- **Better error messages**: Errors can include detailed explanations and extra data
- **Examples**: `E_RANGE("Your list needs at least 3 items", the_list)`

## Quick Tips for Better Error Handling

1. **Always turn on the `d` bit** for new verbs (it's usually on by default)
2. **Use try/except blocks** instead of relying on the old error-value approach
3. **Be specific about which errors you catch** - don't just catch `ANY` unless you have to
4. **Give helpful error messages** to players so they know what went wrong
5. **Test your error handling** by deliberately causing errors during development

## When Things Go Really Wrong

If an error isn't caught anywhere, MOO will:
1. Stop running your program
2. Print a traceback showing where the error happened
3. Display an error message to the player

This helps you debug problems, but it's not very user-friendly, which is why proper error handling is important.

## Learn More

- [Error Types](../the-database/moo-value-types.md#error-type) - Complete list of built-in error codes
- [Try/Except Details](moo-language-statements.md#handling-errors-in-statements) - Full syntax and examples  
- [Error Functions](built-in-functions/values.md#error-handling-functions) - Built-in functions for working with errors
- [Custom Errors](extensions.md#custom-errors-and-errors-with-attached-messages) - mooR's error enhancements