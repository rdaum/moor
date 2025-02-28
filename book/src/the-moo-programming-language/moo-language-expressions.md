# MOO Language Expressions

Expressions are those pieces of MOO code that generate values; for example, the MOO code

```
3 + 4
```

is an expression that generates (or "has" or "returns") the value 7. There are many kinds of expressions in MOO, all of them discussed below.

## Errors While Evaluating Expressions

Most kinds of expressions can, under some circumstances, cause an error to be generated. For example, the expression `x / y` will generate the error `E_DIV` if `y` is equal to zero. When an expression generates an error, the behavior of the server is controlled by setting of the `d` (debug) bit on the verb containing that expression. If the `d` bit is not set, then the error is effectively squelched immediately upon generation; the error value is simply returned as the value of the expression that generated it.

> Note: This error-squelching behavior is very error prone, since it affects _all_ errors, including ones the programmer may not have anticipated. The `d` bit exists only for historical reasons; it was once the only way for MOO programmers to catch and handle errors. The error-catching expression and the `try` -`except` statement, both described below, are far better ways of accomplishing the same thing.

If the `d` bit is set, as it usually is, then the error is _raised_ and can be caught and handled either by code surrounding the expression in question or by verbs higher up on the chain of calls leading to the current verb. If the error is not caught, then the server aborts the entire task and, by default, prints a message to the current player. See the descriptions of the error-catching expression and the `try`-`except` statement for the details of how errors can be caught, and the chapter on server assumptions about the database for details on the handling of uncaught errors.

## Writing Values Directly in Verbs

The simplest kind of expression is a literal MOO value, just as described in the section on values at the beginning of this document. For example, the following are all expressions:

- `17`
- `#893`
- `"This is a character string."`
- `E_TYPE`
- `["key" -> "value"]`
- `{"This", "is", "a", "list", "of", "words"}`

In the case of lists, like the last example above, note that the list expression contains other expressions, several character strings in this case. In general, those expressions can be of any kind at all, not necessarily literal values. For example,

```
{3 + 4, 3 - 4, 3 * 4}
```

is an expression whose value is the list `{7, -1, 12}`.

## Naming Values Within a Verb

As discussed earlier, it is possible to store values in properties on objects; the properties will keep those values forever, or until another value is explicitly put there. Quite often, though, it is useful to have a place to put a value for just a little while. MOO provides local variables for this purpose.

Variables are named places to hold values; you can get and set the value in a given variable as many times as you like. Variables are temporary, though; they only last while a particular verb is running; after it finishes, all of the variables given values there cease to exist and the values are forgotten.

Variables are also "local" to a particular verb; every verb has its own set of them. Thus, the variables set in one verb are not visible to the code of other verbs.

The name for a variable is made up entirely of letters, digits, and the underscore character (`_`) and does not begin with a digit. The following are all valid variable names:

- `foo`
- `_foo`
- `this2that`
- `M68000`
- `two_words`
- `This_is_a_very_long_multiword_variable_name`

Note that, along with almost everything else in MOO, the case of the letters in variable names is insignificant. For example, these are all names for the same variable:

- `fubar`
- `Fubar`
- `FUBAR`
- `fUbAr`

A variable name is itself an expression; its value is the value of the named variable. When a verb begins, almost no variables have values yet; if you try to use the value of a variable that doesn't have one, the error value `E_VARNF` is raised. (MOO is unlike many other programming languages in which one must _declare_ each variable before using it; MOO has no such declarations.) The following variables always have values:

| Variable |
| -------- |
| INT      |
| NUM      |
| FLOAT    |
| OBJ      |
| STR      |
| LIST     |
| ERR      |
| BOOL     |
| MAP      |
| WAIF     |
| ANON     |
| true     |
| false    |
| player   |
| this     |
| caller   |
| verb     |
| args     |
| argstr   |
| dobj     |
| dobjstr  |
| prepstr  |
| iobj     |
| iobjstr  |

> Note: `num` is a deprecated reference to `int` and has been presented only for completeness.

The values of some of these variables always start out the same:

| Variable           | Value | Description                                           |
| ------------------ | ----- | ----------------------------------------------------- |
| <code>INT</code>   | 0     | an integer, the type code for integers                |
| <code>NUM</code>   | 0     | (deprecated) an integer, the type code for integers   |
| <code>OBJ</code>   | 1     | an integer, the type code for objects                 |
| <code>STR</code>   | 2     | an integer, the type code for strings                 |
| <code>ERR</code>   | 3     | an integer, the type code for error values            |
| <code>LIST</code>  | 4     | an integer, the type code for lists                   |
| <code>FLOAT</code> | 9     | an integer, the type code for floating-point numbers  |
| <code>MAP</code>   | 10    | an integer, the type code for map values              |
| <code>ANON</code>  | 12    | an integer, the type code for anonymous object values |
| <code>WAIF</code>  | 13    | an integer, the type code for WAIF values             |
| <code>BOOL</code>  | 14    | an integer, the type code for bool values             |
| <code>true</code>  | true  | the boolean true                                      |
| <code>false</code> | false | the boolean false                                     |

> Note: The `typeof` function can is of note here and is described in the built-ins section.

For others, the general meaning of the value is consistent, though the value itself is different for different situations:

| Variable            | Value                                                                                                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>player</code> | an object, the player who typed the command that started the task that involved running this piece of code.                                                                                             |
| <code>this</code>   | an object, the object on which the currently-running verb was found.                                                                                                                                    |
| <code>caller</code> | an object, the object on which the verb that called the currently-running verb was found. For the first verb called for a given command, <code>caller</code> has the same value as <code>player</code>. |
| <code>verb</code>   | a string, the name by which the currently-running verb was identified.                                                                                                                                  |
| <code>args</code>   | a list, the arguments given to this verb. For the first verb called for a given command, this is a list of strings, the words on the command line.                                                      |

The rest of the so-called "built-in" variables are only really meaningful for the first verb called for a given command. Their semantics is given in the discussion of command parsing, above.

To change what value is stored in a variable, use an _assignment_ expression:

```
variable = expression
```

For example, to change the variable named `x` to have the value 17, you would write `x = 17` as an expression. An assignment expression does two things:

- it changes the value of of the named variable
- it returns the new value of that variable

Thus, the expression

```
13 + (x = 17)
```

changes the value of `x` to be 17 and returns 30.

## Arithmetic Operators

All of the usual simple operations on numbers are available to MOO programs:

```
+
-
*
/
%
```

These are, in order, addition, subtraction, multiplication, division, and remainder. In the following table, the expressions on the left have the corresponding values on the right:

```
5 + 2       =>   7
5 - 2       =>   3
5 * 2       =>   10
5 / 2       =>   2
5.0 / 2.0   =>   2.5
5 % 2       =>   1
5.0 % 2.0   =>   1.0
5 % -2      =>   1
-5 % 2      =>   -1
-5 % -2     =>   -1
-(5 + 2)    =>   -7
```

Note that integer division in MOO throws away the remainder and that the result of the remainder operator (`%`) has the same sign as the left-hand operand. Also, note that `-` can be used without a left-hand operand to negate a numeric expression.

Fine point: Integers and floating-point numbers cannot be mixed in any particular use of these arithmetic operators; unlike some other programming languages, MOO does not automatically coerce integers into floating-point numbers. You can use the `tofloat()` function to perform an explicit conversion.

The `+` operator can also be used to append two strings. The expression

`"foo" + "bar"`

has the value `"foobar"`

The `+` operator can also be used to append two lists. The expression

```
{1, 2, 3} + {4, 5, 6}
```

has the value `{1, 2, 3, 4, 5, 6}`

The `+` operator can also be used to append to a list. The expression

```
{1, 2} + #123
```

has the value of `{1, 2, #123}`
Unless both operands to an arithmetic operator are numbers of the same kind (or, for `+`, both strings), the error value `E_TYPE` is raised. If the right-hand operand for the division or remainder operators (`/` or `%`) is zero, the error value `E_DIV` is raised.

MOO also supports the exponentiation operation, also known as "raising to a power," using the `^` operator:

```
3 ^ 4       =>   81
3 ^ 4.5     error-->   E_TYPE
3.5 ^ 4     =>   150.0625
3.5 ^ 4.5   =>   280.741230801382
```

> Note: if the first operand is an integer, then the second operand must also be an integer. If the first operand is a floating-point number, then the second operand can be either kind of number. Although it is legal to raise an integer to a negative power, it is unlikely to be terribly useful.

## Bitwise Operators

MOO also supports bitwise operations on integer types:

| Operator | Meaning                              |
| -------- | ------------------------------------ |
| &.       | bitwise `and`                        |
| \|.      | bitwise `or`                         |
| ^.       | bitwise `xor`                        |
| >>       | logical (not arithmetic) right-shift |
| <<       | logical (not arithmetic) left-shift  |
| ~        | complement                           |

In the following table, the expressions on the left have the corresponding values on the right:

```
1 &. 2       =>  0
1 |. 2       =>  3
1 ^. 3       =>  1
8 << 1       =>  16
8 >> 1       =>  4
~0           =>  -1
```

For more information on Bitwise Operators, checkout the [Wikipedia](https://en.wikipedia.org/wiki/Bitwise_operation) page on them.

## Comparing Values

Any two values can be compared for equality using `==` and `!=`. The first of these returns 1 if the two values are equal and 0 otherwise; the second does the reverse:

```
3 == 4                              =>  0
3 != 4                              =>  1
3 == 3.0                            =>  0
"foo" == "Foo"                      =>  1
#34 != #34                          =>  0
{1, #34, "foo"} == {1, #34, "FoO"}  =>  1
E_DIV == E_TYPE                     =>  0
3 != "foo"                          =>  1
[1 -> 2] == [1 -> 2]                =>  1
[1 -> 2] == [2 -> 1]                =>  0
true == true                        =>  1
false == true                       =>  0
```

Note that integers and floating-point numbers are never equal to one another, even in the _obvious_ cases. Also note that comparison of strings (and list values containing strings) is case-insensitive; that is, it does not distinguish between the upper- and lower-case version of letters. To test two values for case-sensitive equality, use the `equal` function described later.

> Warning: It is easy (and very annoying) to confuse the equality-testing operator (`==`) with the assignment operator (`=`), leading to nasty, hard-to-find bugs. Don't do this.

> Warning: Comparing floating point numbers for equality can be tricky. Sometimes two floating point numbers will appear the same but be rounded up or down at some meaningful bit, and thus will not be exactly equal. This is especially true when comparing a number in memory (assigned to a variable) to a number that is formed from reading a value from a player, or pulled from a property. Be wary of this, if you ever encounter it, as it can be tedious to debug.

Integers, floats, object numbers, strings, and error values can also be compared for ordering purposes using the following operators:

| Operator | Meaning                           |
| -------- | --------------------------------- |
| &lt;     | meaning &quot;less than&quot;     |
| &lt;=    | &quot;less than or equal&quot;    |
| &gt;=    | &quot;greater than or equal&quot; |
| &gt;     | &quot;greater than&quot;          |

As with the equality operators, these return 1 when their operands are in the appropriate relation and 0 otherwise:

```
3 < 4           =>  1
3 < 4.0         =>  E_TYPE (an error)
#34 >= #32      =>  1
"foo" <= "Boo"  =>  0
E_DIV > E_TYPE  =>  1
```

Note that, as with the equality operators, strings are compared case-insensitively. To perform a case-sensitive string comparison, use the `strcmp` function described later. Also note that the error values are ordered as given in the table in the section on values. If the operands to these four comparison operators are of different types (even integers and floating-point numbers are considered different types), or if they are lists, then `E_TYPE` is raised.

## Values as True and False

There is a notion in MOO of _true_ and _false_ values; every value is one or the other. The true values are as follows:

- all integers other than zero (positive or negative)
- all floating-point numbers not equal to `0.0`
- all non-empty strings (i.e., other than `""`)
- all non-empty lists (i.e., other than `{}`)
- all non-empty maps (i.e, other than `[]`)
- the bool 'true'

All other values are false:

- the integer zero
- the floating-point numbers `0.0` and `-0.0`
- the empty string (`""`)
- the empty list (`{}`)
- all object numbers & object references
- all error values
- the bool 'false'

> Note: Objects are considered false. If you need to evaluate if a value is of the type object, you can use `typeof(potential_object) == OBJ` however, keep in mind that this does not mean that the object referenced actually exists. IE: #100000000 will return true, but that does not mean that object exists in your MOO.

> Note: Don't get confused between values evaluating to true or false, and the boolean values of `true` and `false`.

There are four kinds of expressions and two kinds of statements that depend upon this classification of MOO values. In describing them, I sometimes refer to the _truth value_ of a MOO value; this is just _true_ or _false_, the category into which that MOO value is classified.

The conditional expression in MOO has the following form:

```
expression-1 ? expression-2 | expression-3
```

> Note: This is commonly referred to as a ternary statement in most programming languages. In MOO the commonly used ! is replaced with a |.

First, expression-1 is evaluated. If it returns a true value, then expression-2 is evaluated and whatever it returns is returned as the value of the conditional expression as a whole. If expression-1 returns a false value, then expression-3 is evaluated instead and its value is used as that of the conditional expression.

```
1 ? 2 | 3           =>  2
0 ? 2 | 3           =>  3
"foo" ? 17 | {#34}  =>  17
```

Note that only one of expression-2 and expression-3 is evaluated, never both.

To negate the truth value of a MOO value, use the `!` operator:

```
! expression
```

If the value of expression is true, `!` returns 0; otherwise, it returns 1:

```
! "foo"     =>  0
! (3 >= 4)  =>  1
```

> Note: The "negation" or "not" operator is commonly referred to as "bang" in modern parlance.

It is frequently useful to test more than one condition to see if some or all of them are true. MOO provides two operators for this:

```
expression-1 && expression-2
expression-1 || expression-2
```

These operators are usually read as "and" and "or," respectively.

The `&&` operator first evaluates expression-1. If it returns a true value, then expression-2 is evaluated and its value becomes the value of the `&&` expression as a whole; otherwise, the value of expression-1 is used as the value of the `&&` expression.

> Note: expression-2 is only evaluated if expression-1 returns a true value.

The `&&` expression is equivalent to the conditional expression:

```
expression-1 ? expression-2 | expression-1
```

except that expression-1 is only evaluated once.

The `||` operator works similarly, except that expression-2 is evaluated only if expression-1 returns a false value. It is equivalent to the conditional expression:

```
expression-1 ? expression-1 | expression-2
```

except that, as with `&&`, expression-1 is only evaluated once.

These two operators behave very much like "and" and "or" in English:

```
1 && 1                  =>  1
0 && 1                  =>  0
0 && 0                  =>  0
1 || 1                  =>  1
0 || 1                  =>  1
0 || 0                  =>  0
17 <= 23  &&  23 <= 27  =>  1
```

## Indexing into Lists, Maps and Strings

Strings, lists, and maps can be seen as ordered sequences of MOO values. In the case of strings, each is a sequence of single-character strings; that is, one can view the string `"bar"` as a sequence of the strings `"b"`, `"a"`, and `"r"`. MOO allows you to refer to the elements of lists, maps, and strings by number, by the _index_ of that element in the list or string. The first element has index 1, the second has index 2, and so on.

> Warning: It is very important to note that unlike many programming languages (which use 0 as the starting index), MOO uses 1.

### Extracting an Element by Index

The indexing expression in MOO extracts a specified element from a list, map, or string:

```
expression-1[expression-2]
```

First, expression-1 is evaluated; it must return a list, map, or string (the _sequence_). Then, expression-2 is evaluated and must return an integer (the _index_) or the _key_ in the case of maps. If either of the expressions returns some other type of value, `E_TYPE` is returned.

For lists and strings the index must be between 1 and the length of the sequence, inclusive; if it is not, then `E_RANGE` is raised. The value of the indexing expression is the index'th element in the sequence. For maps, the key must be present, if it is not, then E_RANGE is raised. Within expression-2 you can use the symbol ^ as an expression returning the index or key of the first element in the sequence and you can use the symbol $ as an expression returning the index or key of the last element in expression-1.

```
"fob"[2]                =>  "o"
[1 -> "A"][1]           =>  "A"
"fob"[1]                =>  "f"
{#12, #23, #34}[$ - 1]  =>  #23
```

Note that there are no legal indices for the empty string or list, since there are no integers between 1 and 0 (the length of the empty string or list).

Fine point: The ^ and $ expressions return the first/last index/key of the expression just before the nearest enclosing [...] indexing or subranging brackets. For example:

```
"frob"[{3, 2, 4}[^]]     =>  "o"
"frob"[{3, 2, 4}[$]]     =>  "b"
```

is possible because $ in this case represents the 3rd index of the list next to it, which evaluates to the value 4, which in turn is applied as the index to the string, which evaluates to the b.

### Replacing an Element of a List, Map, or String

It often happens that one wants to change just one particular slot of a list or string, which is stored in a variable or a property. This can be done conveniently using an _indexed assignment_ having one of the following forms:

```
variable[index-expr] = result-expr
object-expr.name[index-expr] = result-expr
object-expr.(name-expr)[index-expr] = result-expr
$name[index-expr] = result-expr
```

The first form writes into a variable, and the last three forms write into a property. The usual errors (`E_TYPE`, `E_INVIND`, `E_PROPNF` and `E_PERM` for lack of read/write permission on the property) may be raised, just as in reading and writing any object property; see the discussion of object property expressions below for details.

Correspondingly, if variable does not yet have a value (i.e., it has never been assigned to), `E_VARNF` will be raised.

If index-expr is not an integer (for lists and strings) or is a collection value (for maps), or if the value of `variable` or the property is not a list, map or string, `E_TYPE` is raised. If `result-expr` is a string, but not of length 1, E_INVARG is raised. Suppose `index-expr` evaluates to a value `k`. If `k` is an integer and is outside the range of the list or string (i.e. smaller than 1 or greater than the length of the list or string), `E_RANGE` is raised. If `k` is not a valid key of the map, `E_RANGE` is raised. Otherwise, the actual assignment takes place.

For lists, the variable or the property is assigned a new list that is identical to the original one except at the k-th position, where the new list contains the result of result-expr instead. Likewise for maps, the variable or the property is assigned a new map that is identical to the original one except for the k key, where the new map contains the result of result-expr instead. For strings, the variable or the property is assigned a new string that is identical to the original one, except the k-th character is changed to be result-expr.

If index-expr is not an integer, or if the value of variable or the property is not a list or string, `E_TYPE` is raised. If result-expr is a string, but not of length 1, `E_INVARG` is raised. Now suppose index-expr evaluates to an integer n. If n is outside the range of the list or string (i.e. smaller than 1 or greater than the length of the list or string), `E_RANGE` is raised. Otherwise, the actual assignment takes place.

For lists, the variable or the property is assigned a new list that is identical to the original one except at the n-th position, where the new list contains the result of result-expr instead. For strings, the variable or the property is assigned a new string that is identical to the original one, except the n-th character is changed to be result-expr.

The assignment expression itself returns the value of result-expr. For the following examples, assume that `l` initially contains the list `{1, 2, 3}`, that `m` initially contains the map `["one" -> 1, "two" -> 2]` and that `s` initially contains the string "foobar":

```
l[5] = 3          =>   E_RANGE (error)
l["first"] = 4    =>   E_TYPE  (error)
s[3] = "baz"      =>   E_INVARG (error)
l[2] = l[2] + 3   =>   5
l                 =>   {1, 5, 3}
l[2] = "foo"      =>   "foo"
l                 =>   {1, "foo", 3}
s[2] = "u"        =>   "u"
s                 =>   "fuobar"
s[$] = "z"        =>   "z"
s                 =>   "fuobaz"
m                 =>   ["foo" -> "bar"]
m[1] = "baz"      =>   ["foo" -> "baz"]
```

> Note: (error) is only used for formatting and identification purposes in these examples and is not present in an actual raised error on the MOO.

> Note: The `$` expression may also be used in indexed assignments with the same meaning as before.

Fine point: After an indexed assignment, the variable or property contains a _new_ list or string, a copy of the original list in all but the n-th place, where it contains a new value. In programming-language jargon, the original list is not mutated, and there is no aliasing. (Indeed, no MOO value is mutable and no aliasing ever occurs.)

In the list and map case, indexed assignment can be nested to many levels, to work on nested lists and maps. Assume that `l` initially contains the following

```
{{1, 2, 3}, {4, 5, 6}, "foo", ["bar" -> "baz"]}
```

in the following examples:

```
l[7] = 4             =>   E_RANGE (error)
l[1][8] = 35         =>   E_RANGE (error)
l[3][2] = 7          =>   E_TYPE (error)
l[1][1][1] = 3       =>   E_TYPE (error)
l[2][2] = -l[2][2]   =>   -5
l                    =>   {{1, 2, 3}, {4, -5, 6}, "foo", ["bar" -> "baz"]}
l[2] = "bar"         =>   "bar"
l                    =>   {{1, 2, 3}, "bar", "foo", ["bar" -> "baz"]}
l[2][$] = "z"        =>   "z"
l                    =>   {{1, 2, 3}, "baz", "foo", ["bar" -> "baz"]}
l[$][^] = #3         =>   #3
l                    =>   {{1, 2, 3}, "baz", "foo", ["bar" -> #3]}
```

The first two examples raise E_RANGE because 7 is out of the range of `l` and 8 is out of the range of `l[1]`. The next two examples raise `E_TYPE` because `l[3]` and `l[1][1]` are not lists.

### Extracting a Subsequence of a List, Map or String

The range expression extracts a specified subsequence from a list, map or string:

```
expression-1[expression-2..expression-3]
```

The three expressions are evaluated in order. Expression-1 must return a list, map or string (the _sequence_) and the other two expressions must return integers (the _low_ and _high_ indices, respectively) for lists and strings, or non-collection values (the `begin` and `end` keys in the ordered map, respectively) for maps; otherwise, `E_TYPE` is raised. The `^` and `$` expression can be used in either or both of expression-2 and expression-3 just as before.

If the low index is greater than the high index, then the empty string, list or map is returned, depending on whether the sequence is a string, list or map. Otherwise, both indices must be between 1 and the length of the sequence (for lists or strings) or valid keys (for maps); `E_RANGE` is raised if they are not. A new list, map or string is returned that contains just the elements of the sequence with indices between the low/high and high/end bounds.

```
"foobar"[2..$]                       =>  "oobar"
"foobar"[3..3]                       =>  "o"
"foobar"[17..12]                     =>  ""
{"one", "two", "three"}[$ - 1..$]    =>  {"two", "three"}
{"one", "two", "three"}[3..3]        =>  {"three"}
{"one", "two", "three"}[17..12]      =>  {}
[1 -> "one", 2 -> "two"][1..1]       =>  [1 -> "one"]
```

### Replacing a Subsequence of a List, Map or String

The subrange assignment replaces a specified subsequence of a list, map or string with a supplied subsequence. The allowed forms are:

```
variable[start-index-expr..end-index-expr] = result-expr
object-expr.name[start-index-expr..end-index-expr] = result-expr
object-expr.(name-expr)[start-index-expr..end-index-expr] = result-expr
$name[start-index-expr..end-index-expr] = result-expr
```

As with indexed assignments, the first form writes into a variable, and the last three forms write into a property. The same errors (`E_TYPE`, `E_INVIND`, `E_PROPNF` and `E_PERM` for lack of read/write permission on the property) may be raised. If variable does not yet have a value (i.e., it has never been assigned to), `E_VARNF` will be raised. As before, the `^` and `$` expression can be used in either start-index-expr or end-index-expr.

If start-index-expr or end-index-expr is not an integer (for lists and strings) or a collection value (for maps), if the value of variable or the property is not a list, map, or string, or result-expr is not the same type as variable or the property, `E_TYPE` is raised. For lists and strings, `E_RANGE` is raised if end-index-expr is less than zero or if start-index-expr is greater than the length of the list or string plus one. Note: the length of result-expr does not need to be the same as the length of the specified range. For maps, `E_RANGE` is raised if `start-index-expr` or `end-index-expr` are not keys in the map.

In precise terms, the subrange assignment

```
v[start..end] = value
```

is equivalent to

```
v = {@v[1..start - 1], @value, @v[end + 1..$]}
```

if v is a list and to

```
v = v[1..start - 1] + value + v[end + 1..$]
```

if v is a string.

There is no literal representation of the operation if v is a map. In this case the range given by start-index-expr and end-index-expr is removed, and the the values in result-expr are added.

The assignment expression itself returns the value of result-expr.

> Note: The use of preceding a list with the @ symbol is covered in just a bit.

For the following examples, assume that `l` initially contains the list `{1, 2, 3}`, that `m` initially contains the map [1 -> "one", 2 -> "two", 3 -> "three"] and that `s` initially contains the string "foobar":

```
l[5..6] = {7, 8}       =>   E_RANGE (error)
l[2..3] = 4            =>   E_TYPE (error)
l[#2..3] = {7}         =>   E_TYPE (error)
s[2..3] = {6}          =>   E_TYPE (error)
l[2..3] = {6, 7, 8, 9} =>   {6, 7, 8, 9}
l                      =>   {1, 6, 7, 8, 9}
l[2..1] = {10, "foo"}  =>   {10, "foo"}
l                      =>   {1, 10, "foo", 6, 7, 8, 9}
l[3][2..$] = "u"       =>   "u"
l                      =>   {1, 10, "fu", 6, 7, 8, 9}
s[7..12] = "baz"       =>   "baz"
s                      =>   "foobarbaz"
s[1..3] = "fu"         =>   "fu"
s                      =>   "fubarbaz"
s[1..0] = "test"       =>   "test"
s                      =>   "testfubarbaz"
m[1..2] = ["abc" -> #1]=>   ["abc" -> #1]
m                      =>   [3 -> "three", "abc" -> #1]
```

## Other Operations on Lists

As was mentioned earlier, lists can be constructed by writing a comma-separated sequence of expressions inside curly braces:

```
{expression-1, expression-2, ..., expression-N}
```

The resulting list has the value of expression-1 as its first element, that of expression-2 as the second, etc.

```
{3 < 4, 3 <= 4, 3 >= 4, 3 > 4}  =>  {1, 1, 0, 0}
```

The addition operator works with lists. When adding two lists together, the two will be concatenated:

```
{1, 2, 3} + {4, 5, 6} => {1, 2, 3, 4, 5, 6})
```

When adding another type to a list, it will append that value to the end of the list:

```
{1, 2} + #123 => {1, 2, #123}
```

Additionally, one may precede any of these expressions by the splicing operator, `@`. Such an expression must return a list; rather than the old list itself becoming an element of the new list, all of the elements of the old list are included in the new list. This concept is easy to understand, but hard to explain in words, so here are some examples. For these examples, assume that the variable `a` has the value `{2, 3, 4}` and that `b` has the value `{"Foo", "Bar"}`:

```
{1, a, 5}   =>  {1, {2, 3, 4}, 5}
{1, @a, 5}  =>  {1, 2, 3, 4, 5}
{a, @a}     =>  {{2, 3, 4}, 2, 3, 4}
{@a, @b}    =>  {2, 3, 4, "Foo", "Bar"}
```

If the splicing operator (`@`) precedes an expression whose value is not a list, then `E_TYPE` is raised as the value of the list construction as a whole.

The list membership expression tests whether or not a given MOO value is an element of a given list and, if so, with what index:

```
expression-1 in expression-2
```

Expression-2 must return a list; otherwise, `E_TYPE` is raised. If the value of expression-1 is in that list, then the index of its first occurrence in the list is returned; otherwise, the `in` expression returns 0.

```
2 in {5, 8, 2, 3}               =>  3
7 in {5, 8, 2, 3}               =>  0
"bar" in {"Foo", "Bar", "Baz"}  =>  2
```

Note that the list membership operator is case-insensitive in comparing strings, just like the comparison operators. To perform a case-sensitive list membership test, use the `is_member` function described later. Note also that since it returns zero only if the given value is not in the given list, the `in` expression can be used either as a membership test or as an element locator.

## Spreading List Elements Among Variables

It is often the case in MOO programming that you will want to access the elements of a list individually, with each element stored in a separate variables. This desire arises, for example, at the beginning of almost every MOO verb, since the arguments to all verbs are delivered all bunched together in a single list. In such circumstances, you _could_ write statements like these:

```
first = args[1];
second = args[2];
if (length(args) > 2)
  third = args[3];
else
  third = 0;
endif
```

This approach gets pretty tedious, both to read and to write, and it's prone to errors if you mistype one of the indices. Also, you often want to check whether or not any _extra_ list elements were present, adding to the tedium.

MOO provides a special kind of assignment expression, called _scattering assignment_ made just for cases such as these. A scattering assignment expression looks like this:

```
{target, ...} = expr
```

where each target describes a place to store elements of the list that results from evaluating expr. A target has one of the following forms:

| Target                                | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>variable</code>                 | This is the simplest target, just a simple variable; the list element in the corresponding position is assigned to the variable. This is called a <em>required</em> target, since the assignment is required to put one of the list elements into the variable.                                                                                                                                                                                                                                                                                                |
| <code>?variable</code>                | This is called an <em>optional</em> target, since it doesn't always get assigned an element. If there are any list elements left over after all of the required targets have been accounted for (along with all of the other optionals to the left of this one), then this variable is treated like a required one and the list element in the corresponding position is assigned to the variable. If there aren't enough elements to assign one to this target, then no assignment is made to this variable, leaving it with whatever its previous value was. |
| <code>?variable = default-expr</code> | This is also an optional target, but if there aren't enough list elements available to assign one to this target, the result of evaluating default-expr is assigned to it instead. Thus, default-expr provides a <em>default value</em> for the variable. The default value expressions are evaluated and assigned working from left to right <em>after</em> all of the other assignments have been performed.                                                                                                                                                 |
| <code>@variable</code>                | By analogy with the <code>@</code> syntax in list construction, this variable is assigned a list of all of the 'leftover' list elements in this part of the list after all of the other targets have been filled in. It is assigned the empty list if there aren't any elements left over. This is called a <em>rest</em> target, since it gets the rest of the elements. There may be at most one rest target in each scattering assignment expression.                                                                                                       |

If there aren't enough list elements to fill all of the required targets, or if there are more than enough to fill all of the required and optional targets but there isn't a rest target to take the leftover ones, then `E_ARGS` is raised.

Here are some examples of how this works. Assume first that the verb `me:foo()` contains the following code:

```
b = c = e = 17;
{a, ?b, ?c = 8, @d, ?e = 9, f} = args;
return {a, b, c, d, e, f};
```

Then the following calls return the given values:

```
me:foo(1)                        =>   E_ARGS (error)
me:foo(1, 2)                     =>   {1, 17, 8, {}, 9, 2}
me:foo(1, 2, 3)                  =>   {1, 2, 8, {}, 9, 3}
me:foo(1, 2, 3, 4)               =>   {1, 2, 3, {}, 9, 4}
me:foo(1, 2, 3, 4, 5)            =>   {1, 2, 3, {}, 4, 5}
me:foo(1, 2, 3, 4, 5, 6)         =>   {1, 2, 3, {4}, 5, 6}
me:foo(1, 2, 3, 4, 5, 6, 7)      =>   {1, 2, 3, {4, 5}, 6, 7}
me:foo(1, 2, 3, 4, 5, 6, 7, 8)   =>   {1, 2, 3, {4, 5, 6}, 7, 8}
```

Using scattering assignment, the example at the beginning of this section could be rewritten more simply, reliably, and readably:

```
{first, second, ?third = 0} = args;
```

Fine point: If you are familiar with JavaScript, the 'rest' and 'spread' functionality should look pretty familiar. It is good MOO programming style to use a scattering assignment at the top of nearly every verb (at least ones that are 'this none this'), since it shows so clearly just what kinds of arguments the verb expects.

## Operations on BOOLs

ToastStunt offers a `bool` type. This type can be either `true` which is considered `1` or `false` which is considered `0`. Boolean values can be set in your code/props much the same way any other value can be assigned to a variable or property.

```
;true                   => true
;false                  => false
;true == true           => 1
;false == false         => 1
;true == false          => 0
;1 == true              => 1
;5 == true              => 0
;0 == false             => 1
;-1 == false            => 0
!true                   => 0
!false                  => 1
!false == true          => 1
!true == false          => 1
```

The true and false variables are set at task runtime (or your code) and can be overridden within verbs if needed. This will not carryover after the verb is finished executing.

> Fine Point: As mentioned earlier, there are constants like STR which resolved to the integer code 2. OBJ resolves to the integer code of 1. Thus if you were to execute code such as `typeof(#15840) == TRUE` you would get a truthy response, as typeof() would return `1` to denote the object's integer code. This is a side effect of `true` always equaling 1, for compatibility reasons.

## Getting and Setting the Values of Properties

Usually, one can read the value of a property on an object with a simple expression:

```
expression.name
```

Expression must return an object number; if not, `E_TYPE` is raised. If the object with that number does not exist, `E_INVIND` is raised. Otherwise, if the object does not have a property with that name, then `E_PROPNF` is raised. Otherwise, if the named property is not readable by the owner of the current verb, then `E_PERM` is raised. Finally, assuming that none of these terrible things happens, the value of the named property on the given object is returned.

I said "usually" in the paragraph above because that simple expression only works if the name of the property obeys the same rules as for the names of variables (i.e., consists entirely of letters, digits, and underscores, and doesn't begin with a digit). Property names are not restricted to this set, though. Also, it is sometimes useful to be able to figure out what property to read by some computation. For these more general uses, the following syntax is also allowed:

```
expression-1.(expression-2)
```

As before, expression-1 must return an object number. Expression-2 must return a string, the name of the property to be read; `E_TYPE` is raised otherwise. Using this syntax, any property can be read, regardless of its name.

Note that, as with almost everything in MOO, case is not significant in the names of properties. Thus, the following expressions are all equivalent:

```
foo.bar
foo.Bar
foo.("bAr")
```

The ToastCore database uses several properties on `#0`, the _system object_, for various special purposes. For example, the value of `#0.room` is the "generic room" object, `#0.exit` is the "generic exit" object, etc. This allows MOO programs to refer to these useful objects more easily (and more readably) than using their object numbers directly. To make this usage even easier and more readable, the expression

```
$name
```

(where name obeys the rules for variable names) is an abbreviation for

```
#0.name
```

Thus, for example, the value `$nothing` mentioned earlier is really `#-1`, the value of `#0.nothing`.

As with variables, one uses the assignment operator (`=`) to change the value of a property. For example, the expression

```
14 + (#27.foo = 17)
```

changes the value of the `foo` property of the object numbered 27 to be 17 and then returns 31. Assignments to properties check that the owner of the current verb has write permission on the given property, raising `E_PERM` otherwise. Read permission is not required.

## Calling Built-in Functions and Other Verbs

MOO provides a large number of useful functions for performing a wide variety of operations; a complete list, giving their names, arguments, and semantics, appears in a separate section later. As an example to give you the idea, there is a function named `length` that returns the length of a given string or list.

The syntax of a call to a function is as follows:

```
name(expr-1, expr-2, ..., expr-N)
```

where name is the name of one of the built-in functions. The expressions between the parentheses, called _arguments_, are each evaluated in turn and then given to the named function to use in its appropriate way. Most functions require that a specific number of arguments be given; otherwise, `E_ARGS` is raised. Most also require that certain of the arguments have certain specified types (e.g., the `length()` function requires a list or a string as its argument); `E_TYPE` is raised if any argument has the wrong type.

As with list construction, the splicing operator `@` can precede any argument expression. The value of such an expression must be a list; `E_TYPE` is raised otherwise. The elements of this list are passed as individual arguments, in place of the list as a whole.

Verbs can also call other verbs, usually using this syntax:

```
expr-0:name(expr-1, expr-2, ..., expr-N)
```

Expr-0 must return an object number; `E_TYPE` is raised otherwise. If the object with that number does not exist, `E_INVIND` is raised. If this task is too deeply nested in verbs calling verbs calling verbs, then `E_MAXREC` is raised; the default limit is 50 levels, but this can be changed from within the database; see the chapter on server assumptions about the database for details. If neither the object nor any of its ancestors defines a verb matching the given name, `E_VERBNF` is raised. Otherwise, if none of these nasty things happens, the named verb on the given object is called; the various built-in variables have the following initial values in the called verb:

| Variable            | Description                                                                                                                                                                      |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>this</code>   | an object, the value of expr-0                                                                                                                                                   |
| <code>verb</code>   | a string, the name used in calling this verb                                                                                                                                     |
| <code>args</code>   | a list, the values of expr-1, expr-2, etc.                                                                                                                                       |
| <code>caller</code> | an object, the value of <code>this</code> in the calling verb                                                                                                                    |
| <code>player</code> | an object, the same value as it had initially in the calling verb or, if the calling verb is running with wizard permissions, the same as the current value in the calling verb. |

All other built-in variables (`argstr`, `dobj`, etc.) are initialized with the same values they have in the calling verb.

As with the discussion of property references above, I said "usually" at the beginning of the previous paragraph because that syntax is only allowed when the name follows the rules for allowed variable names. Also as with property reference, there is a syntax allowing you to compute the name of the verb:

```
expr-0:(expr-00)(expr-1, expr-2, ..., expr-N)
```

The expression expr-00 must return a string; `E_TYPE` is raised otherwise.

The splicing operator (`@`) can be used with verb-call arguments, too, just as with the arguments to built-in functions.

In many databases, a number of important verbs are defined on `#0`, the _system object_. As with the `$foo` notation for properties on `#0`, the server defines a special syntax for calling verbs on `#0`:

```
$name(expr-1, expr-2, ..., expr-N)
```

(where name obeys the rules for variable names) is an abbreviation for

```
#0:name(expr-1, expr-2, ..., expr-N)
```

## Verb Calls on Primitive Types

The server supports verbs calls on primitive types (numbers, strings, etc.) so calls like `"foo bar":split()` can be implemented and work as expected (they were always syntactically correct in LambdaMOO but resulted in an E_TYPE error). Verbs are implemented on prototype object delegates ($int_proto, $float_proto, $str_proto, etc.). The server transparently invokes the correct verb on the appropriate prototype -- the primitive value is the value of `this'.

This also includes supporting calling verbs on an object prototype ($obj_proto). Counterintuitively, this will only work for types of OBJ that are invalid. This can come in useful for un-logged-in connections (i.e. creating a set of convenient utilities for dealing with negative connections in-MOO).

> Fine Point: Utilizing verbs on primitives is a matter of style. Some people like it, some people don't. The author suggests you keep a utility object (like $string_utils) and simply forward verb calls from your primitive to this utility, which keeps backwards compatibility with how ToastCore and LambdaCore are generally built. By default in ToastCore, the primitives just wrap around their `type`_utils counterparts.

## Catching Errors in Expressions

It is often useful to be able to _catch_ an error that an expression raises, to keep the error from aborting the whole task, and to keep on running as if the expression had returned some other value normally. The following expression accomplishes this:

```
` expr-1 ! codes => expr-2 '
```

> Note: The open- and close-quotation marks in the previous line are really part of the syntax; you must actually type them as part of your MOO program for this kind of expression.

The codes part is either the keyword `ANY` or else a comma-separated list of expressions, just like an argument list. As in an argument list, the splicing operator (`@`) can be used here. The `=> expr-2` part of the error-catching expression is optional.

First, the codes part is evaluated, yielding a list of error codes that should be caught if they're raised; if codes is `ANY`, then it is equivalent to the list of all possible MOO values.

Next, expr-1 is evaluated. If it evaluates normally, without raising an error, then its value becomes the value of the entire error-catching expression. If evaluating expr-1 results in an error being raised, then call that error E. If E is in the list resulting from evaluating codes, then E is considered _caught_ by this error-catching expression. In such a case, if expr-2 was given, it is evaluated to get the outcome of the entire error-catching expression; if expr-2 was omitted, then E becomes the value of the entire expression. If E is _not_ in the list resulting from codes, then this expression does not catch the error at all and it continues to be raised, possibly to be caught by some piece of code either surrounding this expression or higher up on the verb-call stack.

Here are some examples of the use of this kind of expression:

```
`x + 1 ! E_TYPE => 0'
```

Returns `x + 1` if `x` is an integer, returns `0` if `x` is not an integer, and raises `E_VARNF` if `x` doesn't have a value.

```
`x.y ! E_PROPNF, E_PERM => 17'
```

Returns `x.y` if that doesn't cause an error, `17` if `x` doesn't have a `y` property or that property isn't readable, and raises some other kind of error (like `E_INVIND`) if `x.y` does.

```
`1 / 0 ! ANY'
```

Returns `E_DIV`.

> Note: It's important to mention how powerful this compact syntax for writing error catching code can be. When used properly you can write very complex and elegant code. For example imagine that you have a set of objects from different parents, some of which define a specific verb, and some of which do not. If for instance, your code wants to perform some function _if_ the verb exists, you can write `obj:verbname() ! E_VERBNF' to allow the MOO to attempt to execute that verb and then if it fails, catch the error and continue operations normally.

## Parentheses and Operator Precedence

As shown in a few examples above, MOO allows you to use parentheses to make it clear how you intend for complex expressions to be grouped. For example, the expression

```
3 * (4 + 5)
```

performs the addition of 4 and 5 before multiplying the result by 3.

If you leave out the parentheses, MOO will figure out how to group the expression according to certain rules. The first of these is that some operators have higher _precedence_ than others; operators with higher precedence will more tightly bind to their operands than those with lower precedence. For example, multiplication has higher precedence than addition; thus, if the parentheses had been left out of the expression in the previous paragraph, MOO would have grouped it as follows:

```
(3 * 4) + 5
```

The table below gives the relative precedence of all of the MOO operators; operators on higher lines in the table have higher precedence and those on the same line have identical precedence:

```
!       - (without a left operand)
^
*       /       %
+       -
==      !=      <       <=      >       >=      in
&&      ||
... ? ... | ... (the conditional expression)
=
```

Thus, the horrendous expression

```
x = a < b && c > d + e * f ? w in y | - q - r
```

would be grouped as follows:

```
x = (((a < b) && (c > (d + (e * f)))) ? (w in y) | ((- q) - r))
```

It is best to keep expressions simpler than this and to use parentheses liberally to make your meaning clear to other humans.
