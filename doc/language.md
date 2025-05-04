# MOO Programming Language Syntax Description

> **Warning**: Parts of this document were written with assistance from Large Language Models (LLMs). While efforts have
> been made to ensure accuracy, please verify critical information against the official MOO language specifications.

## Introduction

MOO is an object-oriented programming language designed for use in MOO environments. It is a dynamic, interpreted
language that allows for rapid development and prototyping.

Its syntax has some similarities in style to Wirth-style languages (like Pascal) because it has a keyword-based block
syntax and 1-based indexing, but it also has a more C-like syntax for expressions and operators, and some functional
programming inspirations (immutable collections, list comprehensions, etc.) though it does not (yet) have first-class
functions or lambdas.

Objects in MOO differ from objects in most other object-oriented programming languages you are likely familiar with.

In MOO, objects are persistent database entities. They are not garbage collected, and are referred to using literals
like `#123` or `$room`. Changes made to objects are permanent and can be accessed by other users or processes.

MOO also has a different inheritance model than most object-oriented languages. Objects can inherit properties and verbs
from other objects,
but there are no "classes." This model of inheritance is called "prototype inheritance" and is similar to the model
in the Self language. Each object has at most one parent, and properties and verbs are looked up in the parent if
they are not found in the object itself. New objects can be created with any other existing object as a parent,
providing
the user has the necessary permissions.

## Basic Structure

A MOO program is called a "verb" and consists of a series of statements that are executed in order. There are no
subroutines or functions in the traditional sense, but verbs can call other verbs on objects.

The following is a rather terse summary of the MOO syntax. For a more user-friendly overview, please see the
LambdaMOO Programmers Manual or various tutorials.

## Statements

MOO supports several types of statements:

1. **Control Flow Statements**:
   - `if`/`elseif`/`else`/`endif` - Conditional execution
   - `while`/`endwhile` - Loop as long as condition is true
   - `for`/`endfor` - Iteration over ranges or collections
   - `fork`/`endfork` - Parallel execution threads
   - `try`/`except`/`finally`/`endtry` - Exception handling
   - `break` and `continue` - Loop control
   - `return` - Return from the current verb

2. **Variable Declaration and Assignment**:
   - `let` - Declares local variables
   - `const` - Declares constants
   - `global` - Declares global variables

3. **Block Structure**:
   - `begin`/`end` - Groups statements into a block

4. **Expression Statements**:
   - Any expression followed by a semicolon

## Variables and Types

MOO supports several basic data types:

1. **Primitive Types**:
   - Integer (`INT`) - Whole numbers
   - Float (`FLOAT`) - Decimal numbers
   - String (`STR`) - Text in double quotes
   - Boolean (`BOOL`) - `true` or `false`
   - Object (`OBJ`) - References to objects in the DB, written as `#123` or `$room`. Note that `$` is a special prefix
     for "system" objects, which are objects referenced off the system object `#0`. `$room` is short-hand for
     `#0.room`.
   - Error (`ERR`) - Error values, starting with `E_`
   - Symbol (`SYM`) - Symbolic identifiers prefixed with a single quote, as in Scheme or Lisp, e.g. `'symbol`

2. **Complex Types**:

   - List (`LIST`) - Ordered collections in curly braces `{1, 2, 3}`. Lists can contain any type of value, including
     other
     lists.
     _Note that unlike most programming languages (and like Pascal, Lua, Julia, etc.) lists are 1-indexed, not
     zero-indexed._

   - Map (`MAP`) - Key-value collections in square brackets `[key -> value]`
   - Flyweight (`FLYWEIGHT`) - Lightweight objects with structure `<parent, [slots], {contents}>`

_Note that MOO's lists and maps have "opposite" syntax to most programming languages. Lists are
`{1, 2, 3}` and maps are `[key -> value]`._ This is a product of the age of the language, which predates the
introduction of Python and other similar languages that used square brackets for lists and curly braces for maps.

3. **Type Constants**:
   - `INT`, `NUM`, `FLOAT`, `STR`, `ERR`, `OBJ`, `LIST`, `MAP`, `BOOL`, `FLYWEIGHT`, `SYM`

## Expressions

Expressions can include:

1. **Arithmetic Operations**:
   - Addition (`+`), Subtraction (`-`), Multiplication (`*`), Division (`/`), Modulus (`%`), Power (`^`)

2. **Comparison Operations**:
   - Equal (`==`), Not Equal (`!=`), Less Than (`<`), Greater Than (`>`), Less Than or Equal (`<=`), Greater Than or
     Equal (`>=`)

3. **Logical Operations**:
   - Logical AND (`&&`), Logical OR (`||`), Logical NOT (`!`)

4. **Special Operations**:
   - Range (`..`) - Used in range selection and range iteration
   - In-range (`in`) - Tests if a value is in a sequence (list or map)

5. **Conditional Expression**:
   - `expr ? true_expr | false_expr` - Ternary conditional expression the same as C's `?:` operator.

6. **Variable Assignment**:
   - `var = expr` - Assigns value to variable

7. **Object Member Access**:
   - Property access: `obj.property` or `obj.(expr)`
   - Verb call: `obj:verb(args)` or `obj:(expr)(args)`
   - System property or verb: `$property` (looks on `#0` for the property)

8. **Collection Operations**:
   - Indexing: `collection[index]`
   - Range indexing: `collection[start..end]`
   - List or map assignment: `list[index] = value` - Assigns value to a specific index or key in a list or map
   - Scatter assignment: `{var1, var2} = list` - Unpacks a list into variables. Has support for optional and rest
     variables: `{var1, ?optional = default, @rest} = list`

9. **Special Forms**:
   - Try expression: `` `expr!codes => handler` `` - Evaluates `expr` and handles errors
   - Range comprehension: `{expr for var in range}` - Creates a list from a generator expression
   - Range end marker: `$` - Represents the end of a list in range operations

## Control Structures

### Conditional Execution

```
if (condition)
    statements
elseif (another_condition)
    statements
else
    statements
endif
```

### Loops

```
while (condition)
    statements
endwhile
```

Labeled loops (can be targeted by `break` and `continue`):

```
while label (condition)
    statements
endwhile
```

### For Loops

The `for` loop has several syntaxes depending on the type of iteration, and will work over both lists and maps.

Iteration over a collection:

```
for item in (collection)
    statements
endfor
```

Iteration with index/key:

For lists:

```
for value, index in (collection)
    statements
endfor
```

For maps:

```
for value, key in (collection)
    statements
endfor
```

(Note the "backwards" order of the arguments, which is done to preserve backwards compatibility with the original
iteration syntax.)

Iteration over a range:

```
for i in [start..end]
    statements
endfor
```

### Parallel Execution

```
fork (seconds)
    statements
endfork
```

Labeled forks:

```
fork label (seconds)
    statements
endfork
```

### Exception Handling

```
try
    statements
except (error_codes)
    statements
endtry
```

With a variable capturing the error:

```
try
    statements
except err_var (error_codes)
    statements
endtry
```

With a finally clause:

```
try
    statements
finally
    cleanup_statements
endtry
```

## Function Calls

1. **Built-in Functions**:

```
function_name(arg1, arg2)
```

2. **Verb Calls**:

```
object:verb(arg1, arg2)
```

3. **System verb Calls**:

```
$system_verb(arg1, arg2)
```

Performs an attempted dispatch to `#0:system_verb(arg1, arg2)`.

4. **Pass Expression** (delegates to a parent object's implementation):

```
pass(arg1, arg2)
```

## Variable Declaration and Assignment

1. **Local Variables**:

Variables can be declared either implicitly or explicitly.

Implicit variables are declared without a `let` keyword, and become present in the scope at their first use:

```
myvar = 5;
myvar = 10;  // Reassigns the variable
```

Explicit variables are declared with the `let` keyword, and become present in the scope at the point of declaration:

```
let var = expr;
let var;  // Default initialized
```

2. **Constants**:

Constants are declared with the `const` keyword and cannot be reassigned after their initial assignment:

```
const var = expr;
```

3. **Global Variables**:

Global variables are declared with the `global` keyword and can be accessed from any scope, but should be used
sparingly:

```
global var = expr;
```

4. **Scatter Assignment** (unpacking):

```
let {var1, var2} = list;
let {var1, ?optional = default, @rest} = list;
```

## Advanced Features

1. **Flyweights** - Lightweight objects with parent, properties, and contents:

```
<parent_obj, [prop1 -> value1, prop2 -> value2], {content1, content2}>
```

2. **Maps** - Key-value pairs:

```
[key1 -> value1, key2 -> value2]
```

3. **For List/Range Comprehensions**

List generation from iteration:

```
{expr for var in (collection)}
```

or

```
{expr for var in [start..end]}
```

e.g.

```
{ x * 2 for x in ({1, 2, 3, 4}) }
```

and

```
{ x * 2 for x in [1..10] }
```

### Conclusion

This overview captures the syntax of the MOO programming language as defined in the grammar. It's a rich language with
features for object-oriented programming, functional programming concepts, error handling, and parallel execution.
