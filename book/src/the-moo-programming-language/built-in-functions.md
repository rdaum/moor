## Built-in Functions

There are a large number of built-in functions available for use by MOO programmers. Each one is discussed in detail in this section. The presentation is broken up into subsections by grouping together functions with similar or related uses.

For most functions, the expected types of the arguments are given; if the actual arguments are not of these types, `E_TYPE` is raised. Some arguments can be of any type at all; in such cases, no type specification is given for the argument. Also, for most functions, the type of the result of the function is given. Some functions do not return a useful result; in such cases, the specification `none` is used. A few functions can potentially return any type of value at all; in such cases, the specification `value` is used.

Most functions take a certain fixed number of required arguments and, in some cases, one or two optional arguments. If a function is called with too many or too few arguments, `E_ARGS` is raised.

Functions are always called by the program for some verb; that program is running with the permissions of some player, usually the owner of the verb in question (it is not always the owner, though; wizards can use `set_task_perms()` to change the permissions _on the fly_). In the function descriptions below, we refer to the player whose permissions are being used as the _programmer_.

Many built-in functions are described below as raising `E_PERM` unless the programmer meets certain specified criteria. It is possible to restrict use of any function, however, so that only wizards can use it; see the chapter on server assumptions about the database for details.

### Object-Oriented Programming

One of the most important facilities in an object-oriented programming language is ability for a child object to make use of a parent's implementation of some operation, even when the child provides its own definition for that operation. The `pass()` function provides this facility in MOO.

**Function: `pass`**

pass -- calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb.

value `pass` (arg, ...)

Often, it is useful for a child object to define a verb that _augments_ the behavior of a verb on its parent object. For example, in the ToastCore database, the root object (which is an ancestor of every other object) defines a verb called `description` that simply returns the value of `this.description`; this verb is used by the implementation of the `look` command. In many cases, a programmer would like the
    description of some object to include some non-constant part; for example, a sentence about whether or not the object was 'awake' or 'sleeping'. This sentence should be added onto the end of the normal description. The programmer would like to have a means of calling the normal `description` verb and then appending the sentence onto the end of that description. The function `pass()` is for exactly such situations.

`pass` calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb. The arguments given to `pass` are the ones given to the called verb and the returned value of the called verb is returned from the call to `pass`.  The initial value of `this` in the called verb is the same as in the calling verb.

Thus, in the example above, the child-object's `description` verb might have the following implementation:

```
return pass() + "  It is " + (this.awake ? "awake." | "sleeping.");
```

That is, it calls its parent's `description` verb and then appends to the result a sentence whose content is computed based on the value of a property on the object.

In almost all cases, you will want to call `pass()` with the same arguments as were given to the current verb. This is easy to write in MOO; just call `pass(@args)`.

### Manipulating MOO Values

There are several functions for performing primitive operations on MOO values, and they can be cleanly split into two kinds: those that do various very general operations that apply to all types of values, and those that are specific to one particular type. There are so many operations concerned with objects that we do not list them in this section but rather give them their own section following this one.

#### General Operations Applicable to All Values

**Function: `typeof`**

typeof -- Takes any MOO value and returns an integer representing the type of value.

int `typeof` (value)

The result is the same as the initial value of one of these built-in variables: `INT`, `FLOAT`, `STR`, `LIST`, `OBJ`, or `ERR`, `BOOL`, `MAP`, `WAIF`, `ANON`.  Thus, one usually writes code like this:

```
if (typeof(x) == LIST) ...
```

and not like this:

```
if (typeof(x) == 3) ...
```

because the former is much more readable than the latter.

**Function: `tostr`**

tostr -- Converts all of the given MOO values into strings and returns the concatenation of the results.

str `tostr` (value, ...)

```
tostr(17)                  =>   "17"
tostr(1.0/3.0)             =>   "0.333333333333333"
tostr(#17)                 =>   "#17"
tostr("foo")               =>   "foo"
tostr({1, 2})              =>   "{list}"
tostr([1 -> 2]             =>   "[map]"
tostr(E_PERM)              =>   "Permission denied"
tostr("3 + 4 = ", 3 + 4)   =>   "3 + 4 = 7"
```

Warning `tostr()` does not do a good job of converting lists and maps  into strings; all lists, including the empty list, are converted into the string `"{list}"` and all maps are converted into the string `"[map]"`. The function `toliteral()`, below, is better for this purpose.

**Function: `toliteral`**

Returns a string containing a MOO literal expression that, when evaluated, would be equal to value.

str `toliteral` (value)

```
toliteral(17)         =>   "17"
toliteral(1.0/3.0)    =>   "0.333333333333333"
toliteral(#17)        =>   "#17"
toliteral("foo")      =>   "\"foo\""
toliteral({1, 2})     =>   "{1, 2}"
toliteral([1 -> 2]    =>   "[1 -> 2]"
toliteral(E_PERM)     =>   "E_PERM"
```

**Function: `toint`**

toint -- Converts the given MOO value into an integer and returns that integer.

int `toint` (value)

Floating-point numbers are rounded toward zero, truncating their fractional parts. Object numbers are converted into the equivalent integers. Strings are parsed as the decimal encoding of a real number which is then converted to an integer. Errors are converted into integers obeying the same ordering (with respect to `<=` as the errors themselves. `toint()` raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number, then `toint()` returns 0.

```
toint(34.7)        =>   34
toint(-34.7)       =>   -34
toint(#34)         =>   34
toint("34")        =>   34
toint("34.7")      =>   34
toint(" - 34  ")   =>   -34
toint(E_TYPE)      =>   1
```

**Function: `toobj`**

toobj -- Converts the given MOO value into an object number and returns that object number.

obj `toobj` (value)

The conversions are very similar to those for `toint()` except that for strings, the number _may_ be preceded by `#`.

```
toobj("34")       =>   #34
toobj("#34")      =>   #34
toobj("foo")      =>   #0
toobj({1, 2})     =>   E_TYPE (error)
```

**Function: `tofloat`**

tofloat -- Converts the given MOO value into a floating-point number and returns that number.

float `tofloat` (value)

Integers and object numbers are converted into the corresponding integral floating-point numbers. Strings are parsed as the decimal encoding of a real number which is then represented as closely as possible as a floating-point number. Errors are first converted to integers as in `toint()` and then converted as integers are. `tofloat()` raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number, then `tofloat()` returns 0.

```
tofloat(34)          =>   34.0
tofloat(#34)         =>   34.0
tofloat("34")        =>   34.0
tofloat("34.7")      =>   34.7
tofloat(E_TYPE)      =>   1.0
```

**Function: `equal`**

equal -- Returns true if value1 is completely indistinguishable from value2.

int `equal` (value, value2)

This is much the same operation as `value1 == value2` except that, unlike `==`, the `equal()` function does not treat upper- and lower-case characters in strings as equal and thus, is case-sensitive.

```
"Foo" == "foo"         =>   1
equal("Foo", "foo")    =>   0
equal("Foo", "Foo")    =>   1
```

**Function: `value_bytes`**

value_bytes -- Returns the number of bytes of the server's memory required to store the given value.

int `value_bytes` (value)

**Function: `value_hash`**

value_hash -- Returns the same string as `string_hash(toliteral(value))`.

str `value_hash` (value, [, str algo] [, binary])

See the description of `string_hash()` for details.

**Function: `value_hmac`**

value_hmac -- Returns the same string as string_hmac(toliteral(value), key)

str `value_hmac` (value, STR key [, STR algo [, binary]])

See the description of string_hmac() for details.  

**Function: `generate_json`**

generate_json -- Returns the JSON representation of the MOO value.

str generate_json (value [, str mode])

Returns the JSON representation of the MOO value.

MOO supports a richer set of values than JSON allows. The optional mode specifies how this function handles the conversion of MOO values into their JSON representation.

The common subset mode, specified by the literal mode string "common-subset", is the default conversion mode. In this mode, only the common subset of types (strings and numbers) are translated with fidelity between MOO types and JSON types. All other types are treated as alternative representations of the string type. This mode is useful for integration with non-MOO applications.

The embedded types mode, specified by the literal mode string "embedded-types", adds type information. Specifically, values other than strings and numbers, which carry implicit type information, are converted into strings with type information appended. The converted string consists of the string representation of the value (as if tostr() were applied) followed by the pipe (|) character and the type. This mode is useful for serializing/deserializing objects and collections of MOO values.
    
```
generate_json([])                                           =>  "{}"
generate_json(["foo" -> "bar"])                             =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "common-subset")            =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "embedded-types")           =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> 1.1])                               =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "common-subset")              =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "embedded-types")             =>  "{\"foo\":1.1}"
generate_json(["foo" -> #1])                                =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "common-subset")               =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "embedded-types")              =>  "{\"foo\":\"#1|obj\"}"
generate_json(["foo" -> E_PERM])                            =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "common-subset")           =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "embedded-types")          =>  "{\"foo\":\"E_PERM|err\"}"
```

JSON keys must be strings, so regardless of the mode, the key will be converted to a string value.

```
generate_json([1 -> 2])                                     =>  "{\"1\":2}"
generate_json([1 -> 2], "common-subset")                    =>  "{\"1\":2}"
generate_json([1 -> 2], "embedded-types")                   =>  "{\"1|int\":2}"
generate_json([#1 -> 2], "embedded-types")                  =>  "{\"#1|obj\":2}"
```

> Warning: generate_json does not support WAIF or ANON types.

**Function: `parse_json`**

parse_json -- Returns the MOO value representation of the JSON string. 

value parse_json (str json [, str mode])

If the specified string is not valid JSON, E_INVARG is raised.

The optional mode specifies how this function handles conversion of MOO values into their JSON representation. The options are the same as for generate_json().

```
parse_json("{}")                                            =>  []
parse_json("{\"foo\":\"bar\"}")                             =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "common-subset")            =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "embedded-types")           =>  ["foo" -> "bar"]
parse_json("{\"foo\":1.1}")                                 =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "common-subset")                =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "embedded-types")               =>  ["foo" -> 1.1]
parse_json("{\"foo\":\"#1\"}")                              =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1\"}", "common-subset")             =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1|obj\"}", "embedded-types")        =>  ["foo" -> #1]
parse_json("{\"foo\":\"E_PERM\"}")                          =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM\"}", "common-subset")         =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM|err\"}", "embedded-types")    =>  ["foo" -> E_PERM]
```

In embedded types mode, key values can be converted to MOO types by appending type information. The full set of supported types are obj, str, err, float and int.
    
```
parse_json("{\"1\":2}")                                     =>   ["1" -> 2]
parse_json("{\"1\":2}", "common-subset")                    =>   ["1" -> 2]
parse_json("{\"1|int\":2}", "embedded-types")               =>   [1 -> 2]
parse_json("{\"#1|obj\":2}", "embedded-types")              =>   [#1 -> 2]
```

> Note: JSON converts `null` to the string "null". 

> Warning: WAIF and ANON types are not supported.

#### Operations on Numbers

**Function: `random`**

random -- Return a random integer

int `random` ([int mod, [int range]])

mod must be a positive integer; otherwise, `E_INVARG` is raised.  If mod is not provided, it defaults to the largest MOO integer, which will depend on if you are running 32 or 64-bit.

if range is provided then an integer in the range of mod to range (inclusive) is returned.

```
random(10)                  => integer between 1-10
random()                    => integer between 1 and maximum integer supported
random(1, 5000)             => integer between 1 and 5000
```

**Function: `frandom`**

float `frandom` (FLOAT mod1 [, FLOAT mod2)

If only one argument is given, a floating point number is chosen randomly from the range `[1.0..mod1]` and returned. If two arguments are given, a floating point number is randomly chosen from the range `[mod1..mod2]`.

**Function: `random_bytes`**

int `random_bytes` (int count)

Returns a binary string composed of between one and 10000 random bytes. count specifies the number of bytes and must be a positive integer; otherwise, E_INVARG is raised. 

**Function: `reseed_random`**

reseed_random -- Provide a new seed to the pseudo random number generator.

void `reseed_random`()

**Function: `min`**

min -- Return the smallest of it's arguments.

int `min` (int x, ...)

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

**Function: `max`**

max -- Return the largest of it's arguments.

int `max` (int x, ...)

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

**Function: `abs`**

abs -- Returns the absolute value of x.

int `abs` (int x)

If x is negative, then the result is `-x`; otherwise, the result is x. The number x can be either integer or floating-point; the result is of the same kind.

**Function: `exp`**

exp -- Returns E (Eulers number) raised to the power of x.

float exp (FLOAT x)

**Function: `floatstr`**

floatstr -- Converts x into a string with more control than provided by either `tostr()` or `toliteral()`.

str `floatstr` (float x, int precision [, scientific])

Precision is the number of digits to appear to the right of the decimal point, capped at 4 more than the maximum available precision, a total of 19 on most machines; this makes it possible to avoid rounding errors if the resulting string is subsequently read back as a floating-point value. If scientific is false or not provided, the result is a string in the form `"MMMMMMM.DDDDDD"`, preceded by a minus sign if and only if x is negative. If scientific is provided and true, the result is a string in the form `"M.DDDDDDe+EEE"`, again preceded by a minus sign if and only if x is negative.

**Function: `sqrt`**

sqrt -- Returns the square root of x.

float `sqrt` (float x)

Raises `E_INVARG` if x is negative.

**Function: `sin`**

sin -- Returns the sine of x.

float `sin` (float x)

**Function: `cos`**

cos -- Returns the cosine of x.

float `cos` (float x)

**Function: `tangent`**

tan -- Returns the tangent of x.

float `tan` (float x)

**Function: `asin`**

asin -- Returns the arc-sine (inverse sine) of x, in the range `[-pi/2..pi/2]`

float `asin` (float x)

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

**Function: `acos`**

acos -- Returns the arc-cosine (inverse cosine) of x, in the range `[0..pi]`

float `acos` (float x)

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

**Function: `atan`**

atan -- Returns the arc-tangent (inverse tangent) of y in the range `[-pi/2..pi/2]`.

float `atan` (float y [, float x])

if x is not provided, or of `y/x` in the range `[-pi..pi]` if x is provided.

**Function: `sinh`**

sinh -- Returns the hyperbolic sine of x.

float `sinh` (float x)

**Function: `cosh`**

cosh -- Returns the hyperbolic cosine of x.

float `cosh` (float x)

**Function: `tanh`**

tanh -- Returns the hyperbolic tangent of x.

float `tanh` (float x)

**Function: `exp`**

exp -- Returns e raised to the power of x.

float `exp` (float x)

**Function: `log`**

log -- Returns the natural logarithm of x.

float `log` (float x)

Raises `E_INVARG` if x is not positive.

**Function: `log10`**

log10 -- Returns the base 10 logarithm of x.

float `log10` (float x)

Raises `E_INVARG` if x is not positive.

**Function: `ceil`**

ceil -- Returns the smallest integer not less than x, as a floating-point number.

float `ceil` (float x)

**Function: `floor`**

floor -- Returns the largest integer not greater than x, as a floating-point number.

float `floor` (float x)

**Function: `trunc`**

trunc -- Returns the integer obtained by truncating x at the decimal point, as a floating-point number.

float `trunc` (float x)

For negative x, this is equivalent to `ceil()`; otherwise it is equivalent to `floor()`.

#### Operations on Strings

**Function: `length`**

length -- Returns the number of characters in string.

int `length` (str string)

It is also permissible to pass a list to `length()`; see the description in the next section.

```
length("foo")   =>   3
length("")      =>   0
```

**Function: `strsub`**

strsub -- Replaces all occurrences of what in subject with with, performing string substitution.

str `strsub` (str subject, str what, str with [, int case-matters])

The occurrences are found from left to right and all substitutions happen simultaneously. By default, occurrences of what are searched for while ignoring the upper/lower case distinction. If case-matters is provided and true, then case is treated as significant in all comparisons.

```
strsub("%n is a fink.", "%n", "Fred")   =>   "Fred is a fink."
strsub("foobar", "OB", "b")             =>   "fobar"
strsub("foobar", "OB", "b", 1)          =>   "foobar"
```

**Function: `index`**

**Function: `rindex`**

index -- Returns the index of the first character of the first occurrence of str2 in str1.

rindex -- Returns the index of the first character of the last occurrence of str2 in str1.

int `index` (str str1, str str2, [, int case-matters [, int skip])

int `rindex` (str str1, str str2, [, int case-matters [, int skip])

These functions will return zero if str2 does not occur in str1 at all.

By default the search for an occurrence of str2 is done while ignoring the upper/lower case distinction. If case-matters is provided and true, then case is treated as significant in all comparisons.

By default the search starts at the beginning (end) of str1. If skip is provided, the search skips the first (last) skip characters and starts at an offset from the beginning (end) of str1. The skip must be a positive integer for index() and a negative integer for rindex(). The default value of skip is 0 (skip no characters).

```
index("foobar", "o")            ⇒   2
index("foobar", "o", 0, 0)      ⇒   2
index("foobar", "o", 0, 2)      ⇒   1
rindex("foobar", "o")           ⇒   3
rindex("foobar", "o", 0, 0)     ⇒   3
rindex("foobar", "o", 0, -4)    ⇒   2
index("foobar", "x")            ⇒   0
index("foobar", "oba")          ⇒   3
index("Foobar", "foo", 1)       ⇒   0
```

**Function: `strtr`**

strtr -- Transforms the string source by replacing the characters specified by str1 with the corresponding characters specified by str2.

int `strtr` (str source, str str1, str str2 [, case-matters])

All other characters are not transformed. If str2 has fewer characters than str1 the unmatched characters are simply removed from source. By default the transformation is done on both upper and lower case characters no matter the case. If case-matters is provided and true, then case is treated as significant.

```
strtr("foobar", "o", "i")           ⇒    "fiibar"
strtr("foobar", "ob", "bo")         ⇒    "fbboar"
strtr("foobar", "", "")             ⇒    "foobar"
strtr("foobar", "foba", "")         ⇒    "r"
strtr("5xX", "135x", "0aBB", 0)     ⇒    "BbB"
strtr("5xX", "135x", "0aBB", 1)     ⇒    "BBX"
strtr("xXxX", "xXxX", "1234", 0)    ⇒    "4444"
strtr("xXxX", "xXxX", "1234", 1)    ⇒    "3434"
```

**Function: `strcmp`**

strcmp -- Performs a case-sensitive comparison of the two argument strings.

int `strcmp` (str str1, str str2)

If str1 is [lexicographically](https://en.wikipedia.org/wiki/Lexicographical_order) less than str2, the `strcmp()` returns a negative integer. If the two strings are identical, `strcmp()` returns zero. Otherwise, `strcmp()` returns a positive integer. The ASCII character ordering is used for the comparison.

**Function: `explode`**

explode -- Returns a list of substrings of subject that are separated by break. break defaults to a space.

list  `explode`(STR subject [, STR break [, INT include-sequential-occurrences])

Only the first character of `break` is considered:

```
explode("slither%is%wiz", "%")      => {"slither", "is", "wiz"}
explode("slither%is%%wiz", "%%")    => {"slither", "is", "wiz"}
```

You can use include-sequential-occurrences to get back an empty string as part of your list if `break` appears multiple times with nothing between it, or there is a leading/trailing `break` in your string:

```
explode("slither%is%%wiz", "%%", 1)  => {"slither", "is", "", "wiz"}
explode("slither%is%%wiz%", "%", 1)  => {"slither", "is", "", "wiz", ""}
explode("%slither%is%%wiz%", "%", 1) => {"", "slither", "is", "", "wiz", ""}
```

> Note: This can be used as a replacement for `$string_utils:explode`.

**Function: `decode_binary`**

decode_binary -- Returns a list of strings and/or integers representing the bytes in the binary string bin_string in order.

list `decode_binary` (str bin-string [, int fully])

If fully is false or omitted, the list contains an integer only for each non-printing, non-space byte; all other characters are grouped into the longest possible contiguous substrings. If  fully is provided and true, the list contains only integers, one for each byte represented in bin_string. Raises `E_INVARG` if bin_string is not a properly-formed binary string. (See the early section on MOO value types for a full description of binary strings.)

```
decode_binary("foo")               =>   {"foo"}
decode_binary("~~foo")             =>   {"~foo"}
decode_binary("foo~0D~0A")         =>   {"foo", 13, 10}
decode_binary("foo~0Abar~0Abaz")   =>   {"foo", 10, "bar", 10, "baz"}
decode_binary("foo~0D~0A", 1)      =>   {102, 111, 111, 13, 10}
```

**Function: `encode_binary`**

encode_binary -- Translates each integer and string in turn into its binary string equivalent, returning the concatenation of all these substrings into a single binary string.

str `encode_binary` (arg, ...)

Each argument must be an integer between 0 and 255, a string, or a list containing only legal arguments for this function. This function   (See the early section on MOO value types for a full description of binary strings.)

```
encode_binary("~foo")                     =>   "~7Efoo"
encode_binary({"foo", 10}, {"bar", 13})   =>   "foo~0Abar~0D"
encode_binary("foo", 10, "bar", 13)       =>   "foo~0Abar~0D"
```

**Function: `decode_base64`**

decode_base64 -- Returns the binary string representation of the supplied Base64 encoded string argument.

str `decode_base64` (str base64 [, int safe])

Raises E_INVARG if base64 is not a properly-formed Base64 string. If safe is provide and is true, a URL-safe version of Base64 is used (see RFC4648).

```
decode_base64("AAEC")      ⇒    "~00~01~02"
decode_base64("AAE", 1)    ⇒    "~00~01"
```

**Function: `encode_base64`**

encode_base64 -- Returns the Base64 encoded string representation of the supplied binary string argument.

str `encode_base64` (str binary [, int safe])

Raises E_INVARG if binary is not a properly-formed binary string. If safe is provide and is true, a URL-safe version of Base64 is used (see [RFC4648](https://datatracker.ietf.org/doc/html/rfc4648)).

```
encode_base64("~00~01~02")    ⇒    "AAEC"
encode_base64("~00~01", 1)    ⇒    "AAE"
```

**Function: `spellcheck`**

spellcheck -- This function checks the English spelling of word.

int | list `spellcheck`(STR word)

If the spelling is correct, the function will return a 1. If the spelling is incorrect, a LIST of suggestions for correct spellings will be returned instead. If the spelling is incorrect and no suggestions can be found, an empty LIST is returned.

**Function: `chr`**

chr -- This function translates integers into ASCII characters. Each argument must be an integer between 0 and 255.

int `chr`(INT arg, ...)

If the programmer is not a wizard, and integers less than 32 are provided, E_INVARG is raised. This prevents control characters or newlines from being written to the database file by non-trusted individuals.

**Function: `match`**

match --  Searches for the first occurrence of the regular expression pattern in the string subject

list `match` (str subject, str pattern [, int case-matters])

If pattern is syntactically malformed, then `E_INVARG` is raised.  The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and true, then case is treated as significant in all comparisons.

The list that `match()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match, replacements is a list described below, and subject is the same string that was given as the first argument to `match()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on parenthesized sub-patterns.

```
match("foo", "^f*o$")        =>  {}
match("foo", "^fo*$")        =>  {1, 3, {{0, -1}, ...}, "foo"}
match("foobar", "o*b")       =>  {2, 4, {{0, -1}, ...}, "foobar"}
match("foobar", "f%(o*%)b")
        =>  {1, 4, {{2, 3}, {0, -1}, ...}, "foobar"}
```

**Function: `rmatch`**

rmatch --  Searches for the last occurrence of the regular expression pattern in the string subject

list `rmatch` (str subject, str pattern [, int case-matters])

If pattern is syntactically malformed, then `E_INVARG` is raised.  The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and true, then case is treated as significant in all comparisons.

The list that `match()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match, replacements is a list described below, and subject is the same string that was given as the first argument to `match()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on parenthesized sub-patterns.

```
rmatch("foobar", "o*b")      =>  {4, 4, {{0, -1}, ...}, "foobar"}
```

#### Perl Compatible Regular Expressions

ToastStunt has two methods of operating on regular expressions. The classic style (outdated, more difficult to use, detailed in the next section) and the preferred Perl Compatible Regular Expression library. It is beyond the scope of this document to teach regular expressions, but an internet search should provide all the information you need to get started on what will surely become a lifelong journey of either love or frustration.

ToastCore offers two primary methods of interacting with regular expressions.

**Function: `pcre_match`**

pcre_match -- The function `pcre_match()` searches `subject` for `pattern` using the Perl Compatible Regular Expressions library. 

LIST `pcre_match`(STR subject, STR pattern [, ?case matters=0] [, ?repeat until no matches=1])

The return value is a list of maps containing each match. Each returned map will have a key which corresponds to either a named capture group or the number of the capture group being matched. The full match is always found in the key "0". The value of each key will be another map containing the keys 'match' and 'position'. Match corresponds to the text that was matched and position will return the indices of the substring within `subject`.

If `repeat until no matches` is 1, the expression will continue to be evaluated until no further matches can be found or it exhausts the iteration limit. This defaults to 1.

Additionally, wizards can control how many iterations of the loop are possible by adding a property to $server_options. $server_options.pcre_match_max_iterations is the maximum number of loops allowed before giving up and allowing other tasks to proceed. CAUTION: It's recommended to keep this value fairly low. The default value is 1000. The minimum value is 100.

Examples:

Extract dates from a string:

```
pcre_match("09/12/1999 other random text 01/21/1952", "([0-9]{2})/([0-9]{2})/([0-9]{4})")

=> {["0" -> ["match" -> "09/12/1999", "position" -> {1, 10}], "1" -> ["match" -> "09", "position" -> {1, 2}], "2" -> ["match" -> "12", "position" -> {4, 5}], "3" -> ["match" -> "1999", "position" -> {7, 10}]], ["0" -> ["match" -> "01/21/1952", "position" -> {30, 39}], "1" -> ["match" -> "01", "position" -> {30, 31}], "2" -> ["match" -> "21", "position" -> {33, 34}], "3" -> ["match" -> "1952", "position" -> {36, 39}]]}
```

Explode a string (albeit a contrived example):

```
;;ret = {}; for x in (pcre_match("This is a string of words, with punctuation, that should be exploded. By space. --zippy--", "[a-zA-Z]+", 0, 1)) ret = {@ret, x["0"]["match"]}; endfor return ret;

=> {"This", "is", "a", "string", "of", "words", "with", "punctuation", "that", "should", "be", "exploded", "By", "space", "zippy"}
```

**Function: `pcre_replace`**

pcre_replace -- The function `pcre_replace()` replaces `subject` with replacements found in `pattern` using the Perl Compatible Regular Expressions library.

STR `pcre_replace` (STR `subject`, STR `pattern`)

The pattern string has a specific format that must be followed, which should be familiar if you have used the likes of Vim, Perl, or sed. The string is composed of four elements, each separated by a delimiter (typically a slash (/) or an exclamation mark (!)), that tell PCRE how to parse your replacement. We'll break the string down and mention relevant options below:

1. Type of search to perform. In MOO, only 's' is valid. This parameter is kept for the sake of consistency.

2. The text you want to search for a replacement.

3. The regular expression you want to use for your replacement text.

4. Optional modifiers:
    * Global. This will replace all occurrences in your string rather than stopping at the first.
    * Case-insensitive. Uppercase, lowercase, it doesn't matter. All will be replaced.

Examples:

Replace one word with another:

```
pcre_replace("I like banana pie. Do you like banana pie?", "s/banana/apple/g")

=> "I like apple pie. Do you like apple pie?"
```

If you find yourself wanting to replace a string that contains slashes, it can be useful to change your delimiter to an exclamation mark:

```
pcre_replace("Unix, wow! /bin/bash is a thing.", "s!/bin/bash!/bin/fish!g")

=> "Unix, wow! /bin/fish is a thing."
```

#### Legacy MOO Regular Expressions

_Regular expression_ matching allows you to test whether a string fits into a specific syntactic shape. You can also search a string for a substring that fits a pattern.

A regular expression describes a set of strings. The simplest case is one that describes a particular string; for example, the string `foo` when regarded as a regular expression matches `foo` and nothing else. Nontrivial regular expressions use certain special constructs so that they can match more than one string. For example, the regular expression `foo%|bar` matches either the string `foo` or the string `bar`; the regular expression `c[ad]*r` matches any of the strings `cr`, `car`, `cdr`, `caar`, `cadddar` and all other such strings with any number of `a`'s and `d`'s.

Regular expressions have a syntax in which a few characters are special constructs and the rest are _ordinary_. An ordinary character is a simple regular expression that matches that character and nothing else. The special characters are `$`, `^`, `.`, `*`, `+`, `?`, `[`, `]` and `%`. Any other character appearing in a regular expression is ordinary, unless a `%` precedes it.

For example, `f` is not a special character, so it is ordinary, and therefore `f` is a regular expression that matches the string `f` and no other string. (It does _not_, for example, match the string `ff`.)  Likewise, `o` is a regular expression that matches only `o`.

Any two regular expressions a and b can be concatenated. The result is a regular expression which matches a string if a matches some amount of the beginning of that string and b matches the rest of the string.

As a simple example, we can concatenate the regular expressions `f` and `o` to get the regular expression `fo`, which matches only the string `fo`. Still trivial.

The following are the characters and character sequences that have special meaning within regular expressions. Any character not mentioned here is not special; it stands for exactly itself for the purposes of searching and matching.

| Character Sequences   | Special Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |                                                                                                                 |                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| <code>.</code>        | is a special character that matches any single character. Using concatenation, we can make regular expressions like <code>a.b</code>, which matches any three-character string that begins with <code>a</code> and ends with <code>b</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                 |                                                                                     |
| <code>*</code>        | is not a construct by itself; it is a suffix that means that the preceding regular expression is to be repeated as many times as possible. In <code>fo*</code>, the <code>*</code> applies to the <code>o</code>, so <code>fo*</code> matches <code>f</code> followed by any number of <code>o</code>&apos;s. The case of zero <code>o</code>&apos;s is allowed: <code>fo*</code> does match <code>f</code>.  <code>*</code> always applies to the <em>smallest</em> possible preceding expression.  Thus, <code>fo*</code> has a repeating <code>o</code>, not a repeating <code>fo</code>.  The matcher processes a <code>*</code> construct by matching, immediately, as many repetitions as can be found. Then it continues with the rest of the pattern.  If that fails, it backtracks, discarding some of the matches of the <code>*</code>&apos;d construct in case that makes it possible to match the rest of the pattern. For example, matching <code>c[ad]*ar</code> against the string <code>caddaar</code>, the <code>[ad]*</code> first matches <code>addaa</code>, but this does not allow the next <code>a</code> in the pattern to match. So the last of the matches of <code>[ad]</code> is undone and the following <code>a</code> is tried again. Now it succeeds.                                                                                                     |                                                                                                                 |                                                                                     |
| <code>+</code>        | <code>+</code> is like <code>*</code> except that at least one match for the preceding pattern is required for <code>+</code>. Thus, <code>c[ad]+r</code> does not match <code>cr</code> but does match anything else that <code>c[ad]*r</code> would match.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                 |                                                                                     |
| <code>?</code>        | <code>?</code> is like <code>*</code> except that it allows either zero or one match for the preceding pattern. Thus, <code>c[ad]?r</code> matches <code>cr</code> or <code>car</code> or <code>cdr</code>, and nothing else.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |                                                                                                                 |                                                                                     |
| <code>[ ... ]</code>  | <code>[</code> begins a <em>character set</em>, which is terminated by a <code>]</code>. In the simplest case, the characters between the two brackets form the set. Thus, <code>[ad]</code> matches either <code>a</code> or <code>d</code>, and <code>[ad]*</code> matches any string of <code>a</code>&apos;s and <code>d</code>&apos;s (including the empty string), from which it follows that <code>c[ad]*r</code> matches <code>car</code>, etc.<br>Character ranges can also be included in a character set, by writing two characters with a <code>-</code> between them. Thus, <code>[a-z]</code> matches any lower-case letter. Ranges may be intermixed freely with individual characters, as in <code>[a-z$%.]</code>, which matches any lower case letter or <code>$</code>, <code>%</code> or period.<br> Note that the usual special characters are not special any more inside a character set. A completely different set of special characters exists inside character sets: <code>]</code>, <code>-</code> and <code>^</code>.<br> To include a <code>]</code> in a character set, you must make it the first character.  For example, <code>[]a]</code> matches <code>]</code> or <code>a</code>. To include a <code>-</code>, you must use it in a context where it cannot possibly indicate a range: that is, as the first character, or immediately after a range. |                                                                                                                 |                                                                                     |
| <code>[^ ... ]</code> | <code>[^</code> begins a <em>complement character set</em>, which matches any character except the ones specified. Thus, <code>[^a-z0-9A-Z]</code> matches all characters <em>except</em> letters and digits.<br><code>^</code> is not special in a character set unless it is the first character.  The character following the <code>^</code> is treated as if it were first (it may be a <code>-</code> or a <code>]</code>).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                 |                                                                                     |
| <code>^</code>        | is a special character that matches the empty string -- but only if at the beginning of the string being matched. Otherwise it fails to match anything.  Thus, <code>^foo</code> matches a <code>foo</code> which occurs at the beginning of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                 |                                                                                     |
| <code>$</code>        | is similar to <code>^</code> but matches only at the <em>end</em> of the string. Thus, <code>xx*$</code> matches a string of one or more <code>x</code>&apos;s at the end of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                 |                                                                                     |
| <code>%</code>        | has two functions: it quotes the above special characters (including <code>%</code>), and it introduces additional special constructs.<br> Because <code>%</code> quotes special characters, <code>%$</code> is a regular expression that matches only <code>$</code>, and <code>%[</code> is a regular expression that matches only <code>[</code>, and so on.<br> For the most part, <code>%</code> followed by any character matches only that character. However, there are several exceptions: characters that, when preceded by <code>%</code>, are special constructs. Such characters are always ordinary when encountered on their own.<br>  No new special characters will ever be defined. All extensions to the regular expression syntax are made by defining new two-character constructs that begin with <code>%</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |                                                                                                                 |                                                                                     |
| <code>%\|</code>      | specifies an alternative. Two regular expressions a and b with <code>%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | </code> in between form an expression that matches anything that either a or b will match.<br> Thus, <code>foo% | bar</code> matches either <code>foo</code> or <code>bar</code> but no other string. |
<code>%|</code> applies to the largest possible surrounding expressions. Only a surrounding <code>%( ... %)</code> grouping can limit the grouping power of <code>%|</code>.<br> Full backtracking capability exists for when multiple <code>%|</code>&apos;s are used. |
| <code>%( ... %)</code> | is a grouping construct that serves three purposes:<br> * To enclose a set of <code>%\|</code> alternatives for other operations. Thus, <code>%(foo%\|bar%)x</code> matches either <code>foox</code> or <code>barx</code>.<br> * To enclose a complicated expression for a following <code>*</code>, <code>+</code>, or <code>?</code> to operate on. Thus, <code>ba%(na%)*</code> matches <code>bananana</code>, etc., with any number of <code>na</code>&apos;s, including none.<br> * To mark a matched substring for future reference.<br> This last application is not a consequence of the idea of a parenthetical grouping; it is a separate feature that happens to be assigned as a second meaning to the same <code>%( ... %)</code> construct because there is no conflict in practice between the two meanings. Here is an explanation of this feature:                                                                                                                                                                               |                                                                                                                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>%digit</code>    | After the end of a <code>%( ... %)</code> construct, the matcher remembers the beginning and end of the text matched by that construct. Then, later on in the regular expression, you can use <code>%</code> followed by digit to mean &quot;match the same text matched by the digit&apos;th <code>%( ... %)</code> construct in the pattern.&quot;  The <code>%( ... %)</code> constructs are numbered in the order that their <code>%(</code>&apos;s appear in the pattern.<br> The strings matching the first nine <code>%( ... %)</code> constructs appearing in a regular expression are assigned numbers 1 through 9 in order of their beginnings. <code>%1</code> through <code>%9</code> may be used to refer to the text matched by the corresponding <code>%( ... %)</code> construct.<br> For example, <code>%(.*%)%1</code> matches any string that is composed of two identical halves. The <code>%(.*%)</code> matches the first half, which may be anything, but the <code>%1</code> that follows must match the same exact text. |                                                                                                                                                                                                               |
| <code>%b</code>        | matches the empty string, but only if it is at the beginning or end of a word. Thus, <code>%bfoo%b</code> matches any occurrence of <code>foo</code> as a separate word. <code>%bball%(s%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | %)%b</code> matches <code>ball</code> or <code>balls</code> as a separate word.<br> For the purposes of this construct and the five that follow, a word is defined to be a sequence of letters and/or digits. |
| <code>%B</code>        | matches the empty string, provided it is <em>not</em> at the beginning or end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |                                                                                                                                                                                                               |
| <code>%&lt;</code>     | matches the empty string, but only if it is at the beginning of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                                                                                                               |
| <code>%&gt;</code>     | matches the empty string, but only if it is at the end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |                                                                                                                                                                                                               |
| <code>%w</code>        | matches any word-constituent character (i.e., any letter or digit).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                                                                                                               |
| <code>%W</code>        | matches any character that is not a word constituent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |                                                                                                                                                                                                               |

**Function: `substitute`**

substitute -- Performs a standard set of substitutions on the string template, using the information contained in subs, returning the resulting, transformed template.

str `substitute` (str template, list subs)

Subs should be a list like those returned by `match()` or `rmatch()` when the match succeeds; otherwise, `E_INVARG` is raised.

In template, the strings `%1` through `%9` will be replaced by the text matched by the first through ninth parenthesized sub-patterns when `match()` or `rmatch()` was called. The string `%0` in template will be replaced by the text matched by the pattern as a whole when `match()` or `rmatch()` was called. The string `%%` will be replaced by a single `%` sign. If `%` appears in template followed by any other character, `E_INVARG` will be raised.

```
subs = match("*** Welcome to ToastStunt!!!", "%(%w*%) to %(%w*%)");
substitute("I thank you for your %1 here in %2.", subs)
        =>   "I thank you for your Welcome here in ToastStunt."
```

**Function: `salt`**

salt -- Generate a crypt() compatible salt string for the specified salt format using the specified binary random input.

str `salt` (str format, str input)

The specific set of formats supported depends on the libraries used to build the server, but will always include the standard salt format, indicated by the format string "" (the empty string), and the BCrypt salt format, indicated by the format string "$2a$NN$" (where "NN" is the work factor). Other possible formats include MD5 ("$1$"), SHA256 ("$5$") and SHA512 ("$6$"). Both the SHA256 and SHA512 formats support optional rounds.

```
salt("", ".M")                                           ⇒    "iB"
salt("$1$", "~183~1E~C6/~D1")                            ⇒    "$1$MAX54zGo"
salt("$5$", "x~F2~1Fv~ADj~92Y~9E~D4l~C3")                ⇒    "$5$s7z5qpeOGaZb"
salt("$5$rounds=2000$", "G~7E~A7~F5Q5~B7~0Aa~80T")       ⇒    "$5$rounds=2000$5trdp5JBreEM"
salt("$6$", "U7~EC!~E8~85~AB~CD~B5+~E1?")                ⇒    "$6$JR1vVUSVfqQhf2yD"
salt("$6$rounds=5000$", "~ED'~B0~BD~B9~DB^,\\~BD~E7")    ⇒    "$6$rounds=5000$hT0gxavqSl0L"
salt("$2a$08$", "|~99~86~DEq~94_~F3-~1A~D2#~8C~B5sx")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc."
```

> Note: To ensure proper security, the random input must be from a sufficiently random source.

**Function: `crypt`**

crypt -- Encrypts the given text using the standard UNIX encryption method.

str `crypt` (str text [, str salt])

Encrypts (hashes) the given text using the standard UNIX encryption method. If provided, salt should be a string at least two characters long, and it may dictate a specific algorithm to use. By default, crypt uses the original, now insecure, DES algorithm. ToastStunt specifically includes the BCrypt algorithm (identified by salts that start with "$2a$"), and may include MD5, SHA256, and SHA512 algorithms depending on the libraries used to build the server. The salt used is returned as the first part of the resulting encrypted string.

Aside from the possibly-random input in the salt, the encryption algorithms are entirely deterministic. In particular, you can test whether or not a given string is the same as the one used to produce a given piece of encrypted text; simply extract the salt from the front of the encrypted text and pass the candidate string and the salt to crypt(). If the result is identical to the given encrypted text, then you`ve got a match.

```
crypt("foobar", "iB")                               ⇒    "iBhNpg2tYbVjw"
crypt("foobar", "$1$MAX54zGo")                      ⇒    "$1$MAX54zGo$UKU7XRUEEiKlB.qScC1SX0"
crypt("foobar", "$5$s7z5qpeOGaZb")                  ⇒    "$5$s7z5qpeOGaZb$xkxjnDdRGlPaP7Z ... .pgk/pXcdLpeVCYh0uL9"
crypt("foobar", "$5$rounds=2000$5trdp5JBreEM")      ⇒    "$5$rounds=2000$5trdp5JBreEM$Imi ... ckZPoh7APC0Mo6nPeCZ3"
crypt("foobar", "$6$JR1vVUSVfqQhf2yD")              ⇒    "$6$JR1vVUSVfqQhf2yD$/4vyLFcuPTz ... qI0w8m8az076yMTdl0h."
crypt("foobar", "$6$rounds=5000$hT0gxavqSl0L")      ⇒    "$6$rounds=5000$hT0gxavqSl0L$9/Y ... zpCATppeiBaDxqIbAN7/"
crypt("foobar", "$2a$08$dHkE1lESV9KrErGhhJTxc.")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc.QnrW/bHp8mmBl5vxGVUcsbjo3gcKlf6"
```

> Note: The specific set of supported algorithms depends on the libraries used to build the server. Only the BCrypt algorithm, which is distributed with the server source code, is guaranteed to exist. BCrypt is currently mature and well tested, and is recommended for new development when the Argon2 library is unavailable. (See next section).

> Warning: The entire salt (of any length) is passed to the operating system`s low-level crypt function. It is unlikely, however, that all operating systems will return the same string when presented with a longer salt. Therefore, identical calls to crypt() may generate different results on different platforms, and your password verification systems will fail. Use a salt longer than two characters at your own risk. 

**Function: `argon2`**

argon2 -- Hashes a password using the Argon2id password hashing algorithm.

The function `argon2()' hashes a password using the Argon2id password hashing algorithm. It is parametrized by three optional arguments:

str `argon2` (STR password, STR salt [, iterations = 3] [, memory usage in KB = 4096] [, CPU threads = 1])

 * Time: This is the number of times the hash will get run. This defines the amount of computation required and, as a result, how long the function will take to complete.
 * Memory: This is how much RAM is reserved for hashing.
 * Parallelism: This is the number of CPU threads that will run in parallel.

The salt for the password should, at minimum, be 16 bytes for password hashing. It is recommended to use the random_bytes() function.

```
salt = random_bytes(20);
return argon2(password, salt, 3, 4096, 1);
```

> Warning: The MOO is single threaded in most cases, and this function can take significant time depending on how you call it. While it is working, nothing else is going to be happening on your MOO. It is possible to build the server with the `THREAD_ARGON2` option which will mitigate lag. This has major caveats however, see the section below on `argon2_verify` for more information.

**Function: `argon2_verify`**

argon2_verify -- Compares password to the previously hashed hash. 

int argon2_verify (STR hash, STR password)

Returns 1 if the two match or 0 if they don't. 

This is a more secure way to hash passwords than the `crypt()` builtin.

> Note: ToastCore defines some sane defaults for how to utilize `argon2` and `argon2_verify`. You can `@grep argon2` from within ToastCore to find these.

> Warning: It is possible to build the server with the `THREAD_ARGON2` option. This will enable this built-in to run in a background thread and mitigate lag that these functions can cause. However, this comes with some major caveats. `do_login_command` (where you will typically be verifying passwords) cannot be suspended. Since threading implicitly suspends the MOO task, you won't be able to directly use Argon2 in do_login_command. Instead, you'll have to devise a new solution for logins that doesn't directly involve calling Argon2 in do_login_command.

> Note: More information on Argon2 can be found in the [Argon2 Github](https://github.com/P-H-C/phc-winner-argon2).

**Function: `string_hash`**

**Function: `binary_hash`**

string_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

binary_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

str `string_hash` (str string, [, algo [, binary]]) 

str `binary_hash` (str bin-string, [, algo [, binary])

 If algo is provided, it specifies the hashing algorithm to use. "MD5", "SHA1", "SHA224", "SHA256", "SHA384", "SHA512" and "RIPEMD160" are all supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

Note that the MD5 hash algorithm is broken from a cryptographic standpoint, as is SHA1. Both are included for interoperability with existing applications (both are still popular).

All supported hash functions have the property that, if

`string_hash(x) == string_hash(y)`

then, almost certainly,

`equal(x, y)`

This can be useful, for example, in certain networking applications: after sending a large piece of text across a connection, also send the result of applying string_hash() to the text; if the destination site also applies string_hash() to the text and gets the same result, you can be quite confident that the large text has arrived unchanged. 

**Function: `string_hmac`**

**Function: `binary_hmac`**

str `string_hmac` (str text, str key [, str algo [, binary]])

str binary_hmac (str bin-string, str key [, str algo [, binary]])

Returns a string encoding the result of applying the HMAC-SHA256 cryptographically secure HMAC function to the contents of the string text or the binary string bin-string with the specified secret key. If algo is provided, it specifies the hashing algorithm to use. Currently, only "SHA1" and "SHA256" are supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

All cryptographically secure HMACs have the property that, if

`string_hmac(x, a) == string_hmac(y, b)`

then, almost certainly,

`equal(x, y)`

and furthermore,

`equal(a, b)`

This can be useful, for example, in applications that need to verify both the integrity of the message (the text) and the authenticity of the sender (as demonstrated by the possession of the secret key).

#### Operations on Lists

**Function: `length`**

length -- Returns the number of elements in list.

int `length` (list list)

It is also permissible to pass a string to `length()`; see the description in the previous section.

```
length({1, 2, 3})   =>   3
length({})          =>   0
```

**Function: `is_member`**

is_member -- Returns true if there is an element of list that is completely indistinguishable from value.

int `is_member` (ANY value, LIST list [, INT case-sensitive])

This is much the same operation as " `value in list`" except that, unlike `in`, the `is_member()` function does not treat upper- and lower-case characters in strings as equal. This treatment of strings can be controlled with the `case-sensitive` argument; setting `case-sensitive` to false will effectively disable this behavior.

Raises E_ARGS if two values are given or if more than three arguments are given. Raises E_TYPE if the second argument is not a list. Otherwise returns the index of `value` in `list`, or 0 if it's not in there.

```
is_member(3, {3, 10, 11})                  => 1
is_member("a", {"A", "B", "C"})            => 0
is_member("XyZ", {"XYZ", "xyz", "XyZ"})    => 3
is_member("def", {"ABC", "DEF", "GHI"}, 0) => 2 
```

**Function: `all_members`**

all_members -- Returns the indices of every instance of `value` in `alist`.

LIST `all_members`(ANY `value`, LIST `alist`)

Example:

```
all_members("a", {"a", "b", "a", "c", "a", "d"}) => {1, 3, 5}
```

**Function: `listinsert`**

**Function: `listappend`**

listinsert -- This functions return a copy of list with value added as a new element.

listappend -- This functions return a copy of list with value added as a new element.

list `listinsert` (list list, value [, int index]) list `listappend` (list list, value [, int index])

`listinsert()` and `listappend()` add value before and after (respectively) the existing element with the given index, if provided.

The following three expressions always have the same value:

```
listinsert(list, element, index)
listappend(list, element, index - 1)
{@list[1..index - 1], element, @list[index..length(list)]}
```

If index is not provided, then `listappend()` adds the value at the end of the list and `listinsert()` adds it at the beginning; this usage is discouraged, however, since the same intent can be more clearly expressed using the list-construction expression, as shown in the examples below.

```
x = {1, 2, 3};
listappend(x, 4, 2)   =>   {1, 2, 4, 3}
listinsert(x, 4, 2)   =>   {1, 4, 2, 3}
listappend(x, 4)      =>   {1, 2, 3, 4}
listinsert(x, 4)      =>   {4, 1, 2, 3}
{@x, 4}               =>   {1, 2, 3, 4}
{4, @x}               =>   {4, 1, 2, 3}
```

**Function: `listdelete`**

listdelete -- Returns a copy of list with the indexth element removed.

list `listdelete` (list list, int index)

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listdelete(x, 2)   =>   {"foo", "baz"}
```

**Function: `listset`**

listset -- Returns a copy of list with the indexth element replaced by value.

list `listset` (list list, value, int index)

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listset(x, "mumble", 2)   =>   {"foo", "mumble", "baz"}
```

This function exists primarily for historical reasons; it was used heavily before the server supported indexed assignments like `x[i] = v`. New code should always use indexed assignment instead of `listset()` wherever possible.

**Function: `setadd`**<br>
**Function: `setremove`**

setadd -- Returns a copy of list with the given value added.

setremove -- Returns a copy of list with the given value removed.

list `setadd` (list list, value) list `setremove` (list list, value)

`setadd()` only adds value if it is not already an element of list; list is thus treated as a mathematical set. value is added at the end of the resulting list, if at all.  Similarly, `setremove()` returns a list identical to list if value is not an element. If value appears more than once in list, only the first occurrence is removed in the returned copy.

```
setadd({1, 2, 3}, 3)         =>   {1, 2, 3}
setadd({1, 2, 3}, 4)         =>   {1, 2, 3, 4}
setremove({1, 2, 3}, 3)      =>   {1, 2}
setremove({1, 2, 3}, 4)      =>   {1, 2, 3}
setremove({1, 2, 3, 2}, 2)   =>   {1, 3, 2}
```

**Function: `reverse`**

reverse -- Return a reversed list or string

str | list `reverse`(LIST alist)

Examples:

```
reverse({1,2,3,4}) => {4,3,2,1}
reverse("asdf") => "fdsa"
```

**Function: `slice`**

list `slice`(LIST alist [, INT | LIST | STR index, ANY default map value])

Return the index-th elements of alist. By default, index will be 1. If index is a list of integers, the returned list will have those elements from alist. This is the built-in equivalent of LambdaCore's $list_utils:slice verb.

If alist is a list of maps, index can be a string indicating a key to return from each map in alist.

If default map value is specified, any maps not containing the key index will have default map value returned in their place. This is useful in situations where you need to maintain consistency with a list index and can't have gaps in your return list.

Examples:

```
slice({{"z", 1}, {"y", 2}, {"x",5}}, 2)                                 => {1, 2, 5}
slice({{"z", 1, 3}, {"y", 2, 4}}, {2, 1})                               => {{1, "z"}, {2, "y"}}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6]}, "a")                => {1, 5}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6], ["b" -> 8]}, "a", 0) => {1, 5, 0}
```
**Function: `sort`**

sort -- Sorts list either by keys or using the list itself.

list `sort`(LIST list [, LIST keys, INT natural sort order?, INT reverse])

When sorting list by itself, you can use an empty list ({}) for keys to specify additional optional arguments.

If natural sort order is true, strings containing multi-digit numbers will consider those numbers to be a single character. So, for instance, this means that 'x2' would come before 'x11' when sorted naturally because 2 is less than 11. This argument defaults to 0.

If reverse is true, the sort order is reversed. This argument defaults to 0.

Examples:

Sort a list by itself:

```
sort({"a57", "a5", "a7", "a1", "a2", "a11"}) => {"a1", "a11", "a2", "a5", "a57", "a7"}
```

Sort a list by itself with natural sort order:

```
sort({"a57", "a5", "a7", "a1", "a2", "a11"}, {}, 1) => {"a1", "a2", "a5", "a7", "a11", "a57"}
```

Sort a list of strings by a list of numeric keys:

```
sort({"foo", "bar", "baz"}, {123, 5, 8000}) => {"bar", "foo", "baz"}
```

> Note: This is a threaded function.

#### Operations on Maps

When using the functions below, it's helpful to remember that maps are ordered.

**Function: `mapkeys`**

mapkeys -- returns the keys of the elements of a map.

list `mapkeys` (map map)

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapkeys(x)   =>  {"bar", "baz", "foo"}
```

**Function: `mapvalues`**

mapvalues -- returns the values of the elements of a map.

list `mapvalues` (MAP `map` [, ... STR `key`])

If you only want the values of specific keys in the map, you can specify them as optional arguments. See examples below.

Examples:  

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapvalues(x)               =>  {2, 3, 1}
mapvalues(x, "foo", "baz") => {1, 3}
```

**Function: `mapdelete`**
mapdelete -- Returns a copy of map with the value corresponding to key removed. If key is not a valid key, then E_RANGE is raised.

map `mapdelete` (map map, key)

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapdelete(x, "bar")   ⇒   ["baz" -> 3, "foo" -> 1]
```

**Function: `maphaskey`**

maphaskey -- Returns 1 if key exists in map. When not dealing with hundreds of keys, this function is faster (and easier to read) than something like: !(x in mapkeys(map))

int `maphaskey` (MAP map, STR key)

### Manipulating Objects

Objects are, of course, the main focus of most MOO programming and, largely due to that, there are a lot of built-in functions for manipulating them.

#### Fundamental Operations on Objects

**Function: `create`**

create -- Creates and returns a new object whose parent (or parents) is parent (or parents) and whose owner is as described below.

obj `create` (obj parent [, obj owner] [, int anon-flag] [, list init-args])

obj `create` (list parents [, obj owner] [, int anon-flag] [, list init-args])

Creates and returns a new object whose parents are parents (or whose parent is parent) and whose owner is as described below. If any of the given parents are not valid, or if the given parent is neither valid nor #-1, then E_INVARG is raised. The given parents objects must be valid and must be usable as a parent (i.e., their `a` or `f` bits must be true) or else the programmer must own parents or be a wizard; otherwise E_PERM is raised. Furthermore, if anon-flag is true then `a` must be true; and, if anon-flag is false or not present, then `f` must be true. Otherwise, E_PERM is raised unless the programmer owns parents or is a wizard. E_PERM is also raised if owner is provided and not the same as the programmer, unless the programmer is a wizard. 

After the new object is created, its initialize verb, if any, is called. If init-args were given, they are passed as args to initialize. The new object is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

> Note: This is not strictly true, especially if you are using ToastCore and the `$recycler`, which is a great idea.  If you don't, you end up with extremely high object numbers. However, if you plan on reusing object numbers you need to consider this carefully in your code. You do not want to include object numbers in your code if this is the case, as object numbers could change. Use corified references instead. For example, you can use `@corify #objnum as $my_object` and then be able to reference $my_object in your code. Alternatively you can do ` @prop $sysobj.my_object #objnum`. If the object number ever changes, you can change the reference without updating all of your code.)

> Note: $sysobj is typically #0. Though it can technically be changed to something else, there is no reason that the author knows of to break from convention here.

If anon-flag is false or not present, the new object is a permanent object and is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

If anon-flag is true, the new object is an anonymous object and is not assigned an object number. Anonymous objects are automatically recycled when they are no longer used.

The owner of the new object is either the programmer (if owner is not provided), the new object itself (if owner was given and is invalid, or owner (otherwise). 

The other built-in properties of the new object are initialized as follows:

```
name         ""
location     #-1
contents     {}
programmer   0
wizard       0
r            0
w            0
f            0
```

The function `is_player()` returns false for newly created objects.

In addition, the new object inherits all of the other properties on its parents. These properties have the same permission bits as on the parents. If the `c` permissions bit is set, then the owner of the property on the new object is the same as the owner of the new object itself; otherwise, the owner of the property on the new object is the same as that on the parent. The initial value of every inherited property is clear; see the description of the built-in function clear_property() for details.

If the intended owner of the new object has a property named `ownership_quota` and the value of that property is an integer, then create() treats that value as a quota. If the quota is less than or equal to zero, then the quota is considered to be exhausted and create() raises E_QUOTA instead of creating an object. Otherwise, the quota is decremented and stored back into the `ownership_quota` property as a part of the creation of the new object. 

> Note: In ToastStunt, this is disabled by default with the "OWNERSHIP_QUOTA" option in options.h

**Function: `owned_objects`**

owned_objects -- Returns a list of all objects in the database owned by `owner`. Ownership is defined by the value of .owner on the object.

list `owned_objects`(OBJ owner)

**Function: `chparent`**

**Function: `chparents`**

chparent -- Changes the parent of object to be new-parent.

chparents -- Changes the parent of object to be new-parents.

none `chparent` (obj object, obj new-parent)

none `chparents` (obj object, list new-parents)

If object is not valid, or if new-parent is neither valid nor equal to `#-1`, then `E_INVARG` is raised. If the programmer is neither a wizard or the owner of object, or if new-parent is not fertile (i.e., its `f` bit is not set) and the programmer is neither the owner of new-parent nor a wizard, then `E_PERM` is raised. If new-parent is equal to `object` or one of its current ancestors, `E_RECMOVE` is raised. If object or one of its descendants defines a property with the same name as one defined either on new-parent or on one of its ancestors, then `E_INVARG` is raised.

Changing an object's parent can have the effect of removing some properties from and adding some other properties to that object and all of its descendants (i.e., its children and its children's children, etc.). Let common be the nearest ancestor that object and new-parent have in common before the parent of object is changed. Then all properties defined by ancestors of object under common (that is, those ancestors of object that are in turn descendants of common) are removed from object and all of its descendants. All properties defined by new-parent or its ancestors under common are added to object and all of its descendants. As with `create()`, the newly-added properties are given the same permission bits as they have on new-parent, the owner of each added property is either the owner of the object it's added to (if the `c` permissions bit is set) or the owner of that property on new-parent, and the value of each added property is _clear_; see the description of the built-in function `clear_property()` for details. All properties that are not removed or added in the reparenting process are completely unchanged.

If new-parent is equal to `#-1`, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

If new-parents is equal to {}, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

> Warning: On the subject of multiple inheritance, the author (Slither) thinks you should completely avoid it. Prefer [composition over inheritance](https://en.wikipedia.org/wiki/Composition_over_inheritance).

**Function: `valid`**

valid -- Return a non-zero integer if object is valid and not yet recycled.

int `valid` (obj object)

Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.

```
valid(#0)    =>   1
valid(#-1)   =>   0
```

**Function: `parent`**

**Function: `parents`**

parent -- return the parent of object

parents -- return the parents of object

obj `parent` (obj object)

list `parents` (obj object)

**Function: `children`**

children -- return a list of the children of object.

list `children` (obj object)

**Function: `isa`**

int isa(OBJ object, OBJ parent)

obj isa(OBJ object, LIST parent list [, INT return_parent])

Returns true if object is a descendant of parent, otherwise false.

If a third argument is present and true, the return value will be the first parent that object1 descends from in the `parent list`.

```
isa(#2, $wiz)                           => 1
isa(#2, {$thing, $wiz, $container})     => 1
isa(#2, {$thing, $wiz, $container}, 1)  => #57 (generic wizard)
isa(#2, {$thing, $room, $container}, 1) => #-1 
```

**Function: `locate_by_name`**

locate_by_name -- This function searches every object in the database for those containing `object name` in their .name property.

list `locate_by_name` (STR object name)

> Warning: Take care when using this when thread mode is active, as this is a threaded function and that means it implicitly suspends. `set_thread_mode(0)` if you want to use this without suspending.

**Function: `locations`**

list `locations`(OBJ object [, OBJ stop [, INT is-parent]])

Recursively build a list of an object's location, its location's location, and so forth until finally hitting $nothing.

Example:

```
locations(me) => {#20381, #443, #104735}

$string_utils:title_list(locations(me)) => "\"Butterknife Ballet\" Control Room FelElk, the one-person celestial birther \"Butterknife Ballet\", and Uncharted Space: Empty Space"
```

If `stop` is in the locations found, it will stop before there and return the list (exclusive of the stop object). 

If the third argument is true, `stop` is assumed to be a PARENT. And if any of your locations are children of that parent, it stops there.

**Function: `occupants`**

list `occupants`(LIST objects [, OBJ | LIST parent, INT player flag set?])

Iterates through the list of objects and returns those matching a specific set of criteria:

1. If only objects is specified, the occupants function will return a list of objects with the player flag set.

2. If the parent argument is specified, a list of objects descending from parent> will be returned. If parent is a list, object must descend from at least one object in the list.

3. If both parent and player flag set are specified, occupants will check both that an object is descended from parent and also has the player flag set.

**Function: `recycle`**

recycle -- destroy object irrevocably.

none `recycle` (obj object)

The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, `E_PERM` is raised. If object is not valid, then `E_INVARG` is raised. The children of object are reparented to the parent of object. Before object is recycled, each object in its contents is moved to `#-1` (implying a call to object's `exitfunc` verb, if any) and then object's `recycle` verb, if any, is called with no arguments.

After object is recycled, if the owner of the former object has a property named `ownership_quota` and the value of that property is a integer, then `recycle()` treats that value as a _quota_ and increments it by one, storing the result back into the `ownership_quota` property.

**Function: `recreate`**

recreate -- Recreate invalid object old (one that has previously been recycle()ed) as parent, optionally owned by owner.

obj `recreate`(OBJ old, OBJ parent [, OBJ owner])

This has the effect of filling in holes created by recycle() that would normally require renumbering and resetting the maximum object.

The normal rules apply to parent and owner. You either have to own parent, parent must be fertile, or you have to be a wizard. Similarly, to change owner, you should be a wizard. Otherwise it's superfluous.

**Function: `next_recycled_object`**

next_recycled_object -- Return the lowest invalid object. If start is specified, no object lower than start will be considered. If there are no invalid objects, this function will return 0.

obj | int `next_recycled_object`(OBJ start)

**Function: `recycled_objects`**

recycled_objects -- Return a list of all invalid objects in the database. An invalid object is one that has been destroyed with the recycle() function.

list `recycled_objects`()

**Function: `ancestors`**

ancestors -- Return a list of all ancestors of `object` in order ascending up the inheritance hiearchy. If `full` is true, `object` will be included in the list.

list `ancestors`(OBJ object [, INT full])

**Function: `clear_ancestor_cache`**

void `clear_ancestor_cache`()

The ancestor cache contains a quick lookup of all of an object's ancestors which aids in expediant property lookups. This is an experimental feature and, as such, you may find that something has gone wrong. If that's that case, this function will completely clear the cache and it will be rebuilt as-needed.

**Function: `descendants`**

list `descendants`(OBJ object [, INT full])

Return a list of all nested children of object. If full is true, object will be included in the list.

**Function: `object_bytes`**

object_bytes -- Returns the number of bytes of the server's memory required to store the given object.

int `object_bytes` (obj object)

The space calculation includes the space used by the values of all of the objects non-clear properties and by the verbs and properties defined directly on the object.

Raises `E_INVARG` if object is not a valid object and `E_PERM` if the programmer is not a wizard.

**Function: `respond_to`**

int | list respond_to(OBJ object, STR verb)

Returns true if verb is callable on object, taking into account inheritance, wildcards (star verbs), etc. Otherwise, returns false.  If the caller is permitted to read the object (because the object's `r' flag is true, or the caller is the owner or a wizard) the true value is a list containing the object number of the object that defines the verb and the full verb name(s).  Otherwise, the numeric value `1' is returned.

**Function: `max_object`**

max_object -- Returns the largest object number ever assigned to a created object.

obj `max_object`()

//TODO update for how Toast handles recycled objects if it is different
Note that the object with this number may no longer exist; it may have been recycled.  The next object created will be assigned the object number one larger than the value of `max_object()`. The next object getting the number one larger than `max_object()` only applies if you are using built-in functions for creating objects and does not apply if you are using the `$recycler` to create objects.

#### Object Movement

**Function: `move`**

move -- Changes what's location to be where.

none `move` (obj what, obj where [, INT position)

This is a complex process because a number of permissions checks and notifications must be performed.  The actual movement takes place as described in the following paragraphs.

what should be a valid object and where should be either a valid object or `#-1` (denoting a location of 'nowhere'); otherwise `E_INVARG` is raised. The programmer must be either the owner of what or a wizard; otherwise, `E_PERM` is raised.

If where is a valid object, then the verb-call

```
where:accept(what)
```

is performed before any movement takes place. If the verb returns a false value and the programmer is not a wizard, then where is considered to have refused entrance to what; `move()` raises `E_NACC`. If where does not define an `accept` verb, then it is treated as if it defined one that always returned false.

If moving what into where would create a loop in the containment hierarchy (i.e., what would contain itself, even indirectly), then `E_RECMOVE` is raised instead.

The `location` property of what is changed to be where, and the `contents` properties of the old and new locations are modified appropriately. Let old-where be the location of what before it was moved. If old-where is a valid object, then the verb-call

```
old-where:exitfunc(what)
```

is performed and its result is ignored; it is not an error if old-where does not define a verb named `exitfunc`. Finally, if where and what are still valid objects, and where is still the location of what, then the verb-call

```
where:enterfunc(what)
```

is performed and its result is ignored; again, it is not an error if where does not define a verb named `enterfunc`.

Passing `position` into move will effectively listinsert() the object into that position in the .contents list.

#### Operations on Properties

**Function: `properties`**

properties -- Returns a list of the names of the properties defined directly on the given object, not inherited from its parent.

list `properties` (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

**Function: `property_info`**

property_info -- Get the owner and permission bits for the property named prop-name on the given object

list `property_info` (obj object, str prop-name)

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `property_info()` raises `E_PERM`.

**Function: `set_property_info`**

set_property_info -- Set the owner and permission bits for the property named prop-name on the given object

none `set_property_info` (obj object, str prop-name, list info)

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `set_property_info()` raises `E_PERM`. Property info has the following form:

```
{owner, perms [, new-name]}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, and `c`, and new-name is a string; new-name is never part of the value returned by `property_info()`, but it may optionally be given as part of the value provided to `set_property_info()`. This list is the kind of value returned by property_info() and expected as the third argument to `set_property_info()`; the latter function raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or, when new-name is given, if prop-name is not defined directly on object or new-name names an existing property defined on object or any of its ancestors or descendants.

**Function: `add_property`**

add_property -- Defines a new property on the given object

none `add_property` (obj object, str prop-name, value, list info)

The property is inherited by all of its descendants; the property is named prop-name, its initial value is value, and its owner and initial permission bits are given by info in the same format as is returned by `property_info()`, described above.

If object is not valid or info does not specify a valid owner and well-formed permission bits or object or its ancestors or descendants already defines a property named prop-name, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

**Function: `delete_property`**

delete_property -- Removes the property named prop-name from the given object and all of its descendants.

none `delete_property` (obj object, str prop-name)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not directly define a property named prop-name (as opposed to inheriting one from its parent), then `E_PROPNF` is raised.

**Function: `is_clear_property`**

is_clear_property -- Test the specified property for clear

int `is_clear_property` (obj object, str prop-name) **Function: `clear_property`**

clear_property -- Set the specified property to clear

none `clear_property` (obj object, str prop-name)

These two functions test for clear and set to clear, respectively, the property named prop-name on the given object. If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `is_clear_property()` (`clear_property()`) raises `E_PERM`.

If a property is clear, then when the value of that property is queried the value of the parent's property of the same name is returned. If the parent's property is clear, then the parent's parent's value is examined, and so on.  If object is the definer of the property prop-name, as opposed to an inheritor of the property, then `clear_property()` raises `E_INVARG`.

#### Operations on Verbs

**Function: `verbs`**

verbs -- Returns a list of the names of the verbs defined directly on the given object, not inherited from its parent

list verbs (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

Most of the remaining operations on verbs accept a string containing the verb's name to identify the verb in question. Because verbs can have multiple names and because an object can have multiple verbs with the same name, this practice can lead to difficulties. To most unambiguously refer to a particular verb, one can instead use a positive integer, the index of the verb in the list returned by `verbs()`, described above.

For example, suppose that `verbs(#34)` returns this list:

```
{"foo", "bar", "baz", "foo"}
```

Object `#34` has two verbs named `foo` defined on it (this may not be an error, if the two verbs have different command syntaxes). To refer unambiguously to the first one in the list, one uses the integer 1; to refer to the other one, one uses 4.

In the function descriptions below, an argument named verb-desc is either a string containing the name of a verb or else a positive integer giving the index of that verb in its defining object's `verbs()` list.
For historical reasons, there is also a second, inferior mechanism for referring to verbs with numbers, but its use is strongly discouraged. If the property `$server_options.support_numeric_verbname_strings` exists with a true value, then functions on verbs will also accept a numeric string (e.g., `"4"`) as a verb descriptor. The decimal integer in the string works more-or-less like the positive integers described above, but with two significant differences:

The numeric string is a _zero-based_ index into `verbs()`; that is, in the string case, you would use the number one less than what you would use in the positive integer case.

When there exists a verb whose actual name looks like a decimal integer, this numeric-string notation is ambiguous; the server will in all cases assume that the reference is to the first verb in the list for which the given string could be a name, either in the normal sense or as a numeric index.

Clearly, this older mechanism is more difficult and risky to use; new code should only be written to use the current mechanism, and old code using numeric strings should be modified not to do so.

**Function: `verb_info`**

verb_info -- Get the owner, permission bits, and name(s) for the verb as specified by verb-desc on the given object

list `verb_info` (obj object, str|int verb-desc) 

**Function: `set_verb_info`**

set_verb_info -- Set the owner, permissions bits, and names(s) for the verb as verb-desc on the given object

none `set_verb_info` (obj object, str|int verb-desc, list info)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_info()` (`set_verb_info()`) raises `E_PERM`.

Verb info has the following form:

```
{owner, perms, names}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, `x`, and `d`, and names is a string. This is the kind of value returned by `verb_info()` and expected as the third argument to `set_verb_info()`. `set_verb_info()` raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or if names is the empty string or consists entirely of spaces; it raises `E_PERM` if owner is not the programmer and the programmer is not a wizard.

**Function: `verb_args`**

verb_args -- get the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

list `verb_args` (obj object, str|int verb-desc) 

**Function: `set_verb_args`**

verb_args -- set the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

none `set_verb_args` (obj object, str|int verb-desc, list args)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then the function raises `E_PERM`.

Verb args specifications have the following form:

```
{dobj, prep, iobj}
```

where dobj and iobj are strings drawn from the set `"this"`, `"none"`, and `"any"`, and prep is a string that is either `"none"`, `"any"`, or one of the prepositional phrases listed much earlier in the description of verbs in the first chapter. This is the kind of value returned by `verb_args()` and expected as the third argument to `set_verb_args()`. Note that for `set_verb_args()`, prep must be only one of the prepositional phrases, not (as is shown in that table) a set of such phrases separated by `/` characters. `set_verb_args` raises `E_INVARG` if any of the dobj, prep, or iobj strings is illegal.

```
verb_args($container, "take")
                    =>   {"any", "out of/from inside/from", "this"}
set_verb_args($container, "take", {"any", "from", "this"})
```

**Function: `add_verb`**

add_verb -- defines a new verb on the given object

none `add_verb` (obj object, list info, list args)

The new verb's owner, permission bits and name(s) are given by info in the same format as is returned by `verb_info()`, described above. The new verb's direct-object, preposition, and indirect-object specifications are given by args in the same format as is returned by `verb_args`, described above. The new verb initially has the empty program associated with it; this program does nothing but return an unspecified value.

If object is not valid, or info does not specify a valid owner and well-formed permission bits and verb names, or args is not a legitimate syntax specification, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

**Function: `delete_verb`**

delete_verb -- removes the verb as specified by verb-desc from the given object

none `delete_verb` (obj object, str|int verb-desc)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised.

**Function: `verb_code`**

verb_code -- get the MOO-code program associated with the verb as specified by verb-desc on object

list `verb_code` (obj object, str|int verb-desc [, fully-paren [, indent]]) 

**Function: `set_verb_code`**

set_verb_code -- set the MOO-code program associated with the verb as specified by verb-desc on object

list `set_verb_code` (obj object, str|int verb-desc, list code)

The program is represented as a list of strings, one for each line of the program; this is the kind of value returned by `verb_code()` and expected as the third argument to `set_verb_code()`. For `verb_code()`, the expressions in the returned code are usually written with the minimum-necessary parenthesization; if full-paren is true, then all expressions are fully parenthesized.

Also for `verb_code()`, the lines in the returned code are usually not indented at all; if indent is true, each line is indented to better show the nesting of statements.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_code()` (`set_verb_code()`) raises `E_PERM`. If the programmer is not, in fact. a programmer, then `E_PERM` is raised.

For `set_verb_code()`, the result is a list of strings, the error messages generated by the MOO-code compiler during processing of code. If the list is non-empty, then `set_verb_code()` did not install code; the program associated with the verb in question is unchanged.

**Function: `disassemble`**

disassemble -- returns a (longish) list of strings giving a listing of the server's internal "compiled" form of the verb as specified by verb-desc on object

list `disassemble` (obj object, str|int verb-desc)

This format is not documented and may indeed change from release to release, but some programmers may nonetheless find the output of `disassemble()` interesting to peruse as a way to gain a deeper appreciation of how the server works.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read permission on the verb in question, then `disassemble()` raises `E_PERM`.

#### Operations on WAIFs

**Function: `new_waif`**

new_waif -- The `new_waif()` builtin creates a new WAIF whose class is the calling object and whose owner is the perms of the calling verb.

waif `new_waif`()

This wizardly version causes it to be owned by the caller of the verb.

**Function: `waif_stats`**

waif_stats -- Returns a MAP of statistics about instantiated waifs.

map `waif_stats`()

Each waif class will be a key in the MAP and its value will be the number of waifs of that class currently instantiated. Additionally, there is a `total' key that will return the total number of instantiated waifs, and a `pending_recycle' key that will return the number of waifs that have been destroyed and are awaiting the call of their :recycle verb.

#### Operations on Player Objects

**Function: `players`**

players -- returns a list of the object numbers of all player objects in the database

list `players` ()

**Function: `is_player`**

is_player -- returns a true value if the given object is a player object and a false value otherwise.

int `is_player` (obj object)

If object is not valid, `E_INVARG` is raised.

**Function: `set_player_flag`**

set_player_flag -- confers or removes the "player object" status of the given object, depending upon the truth value of value

none `set_player_flag` (obj object, value)

If object is not valid, `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised.

If value is true, then object gains (or keeps) "player object" status: it will be an element of the list returned by `players()`, the expression `is_player(object)` will return true, and the server will treat a call to `$do_login_command()` that returns object as logging in the current connection.

If value is false, the object loses (or continues to lack) "player object" status: it will not be an element of the list returned by `players()`, the expression `is_player(object)` will return false, and users cannot connect to object by name when they log into the server. In addition, if a user is connected to object at the time that it loses "player object" status, then that connection is immediately broken, just as if `boot_player(object)` had been called (see the description of `boot_player()` below).

### Operations on Files

There are several administrator-only builtins for manipulating files from inside the MOO.  Security is enforced by making these builtins executable with wizard permissions only as well as only allowing access to a directory under the current directory (the one the server is running in). The new builtins are structured similarly to the stdio library for C. This allows MOO-code to perform stream-oriented I/O to files.

Granting MOO code direct access to files opens a hole in the otherwise fairly good wall that the ToastStunt server puts up between the OS and the database.  The security is fairly well mitigated by restricting where files can be opened and allowing the builtins to be called by wizard permissions only. It is still possible execute various forms denial of service attacks, but the MOO server allows this form of attack as well.

> Warning: Depending on what Core you are using (ToastCore, LambdaMOO, etc) you may have a utility that acts as a wrapper around the FileIO code. This is the preferred method for dealing with files and directly using the built-ins is discouraged. On ToastCore you may have a $file WAIF you can utilize for this purpose.

> Warning: The FileIO code looks for a 'files' directory in the same directory as the MOO executable. This directory must exist for your code to work.

> Note: More detailed information regarding the FileIO code can be found in the docs/FileioDocs.txt folder of the ToastStunt repo.

The FileIO system has been updated in ToastCore and includes a number of enhancements over earlier LambdaMOO and Stunt versions.
* Faster reading
* Open as many files as you want, configurable with FILE_IO_MAX_FILES or $server_options.file_io_max_files

**FileIO Error Handling**

Errors are always handled by raising some kind of exception. The following exceptions are defined:

`E_FILE`

This is raised when a stdio call returned an error value. CODE is set to E_FILE, MSG is set to the return of strerror() (which may vary from system to system), and VALUE depends on which function raised the error.  When a function fails because the stdio function returned EOF, VALUE is set to "EOF".

`E_INVARG`

This is raised for a number of reasons.  The common reasons are an invalid FHANDLE being passed to a function and an invalid pathname specification.  In each of these cases MSG will be set to the cause and VALUE will be the offending value.

`E_PERM`

This is raised when any of these functions are called with non- wizardly permissions.

**General Functions**

**Function: `file_version`**

file_version -- Returns the package shortname/version number of this package e.g.

str `file_version`()

`file_version() => "FIO/1.7"`

**Opening and closing of files and related functions**

File streams are associated with FHANDLES.  FHANDLES are similar to the FILE\* using stdio.  You get an FHANDLE from file_open.  You should not depend on the actual type of FHANDLEs (currently TYPE_INT).  FHANDLEs are not persistent across server restarts.  That is, files open when the server is shut down are closed when it comes back up and no information about open files is saved in the DB.

**Function: `file_open`**

file_open -- Open a file 

FHANDLE `file_open`(STR pathname, STR mode)

Raises: E_INVARG if mode is not a valid mode, E_QUOTA if too many files are open.

This opens a file specified by pathname and returns an FHANDLE for it.  It ensures pathname is legal.  Mode is a string of characters indicating what mode the file is opened in. The mode string is four characters.

The first character must be (r)ead, (w)rite, or (a)ppend.  The second must be '+' or '-'.  This modifies the previous argument.

* r- opens the file for reading and fails if the file does not exist.
* r+ opens the file for reading and writing and fails if the file does not exist.
* w- opens the file for writing, truncating if it exists and creating if not.
* w+ opens the file for reading and writing, truncating if it exists and creating if not.
* a- opens a file for writing, creates it if it does not exist and positions the stream at the end of the file.
* a+ opens the file for reading and writing, creates it if does not exist and positions the stream at the end of the file.

The third character is either (t)ext or (b)inary.  In text mode, data is written as-is from the MOO and data read in by the MOO is stripped of unprintable characters.  In binary mode, data is written filtered through the binary-string->raw-bytes conversion and data is read filtered through the raw-bytes->binary-string conversion.  For example, in text mode writing " 1B" means three bytes are written: ' ' Similarly, in text mode reading " 1B" means the characters ' ' '1' 'B' were present in the file.  In binary mode reading " 1B" means an ASCII ESC was in the file.  In text mode, reading an ESC from a file results in the ESC getting stripped.

It is not recommended that files containing unprintable ASCII  data be read in text mode, for obvious reasons.

The final character is either 'n' or 'f'.  If this character is 'f', whenever data is written to the file, the MOO will force it to finish writing to the physical disk before returning.  If it is 'n' then this won't happen.

This is implemented using fopen().

** Function: `file_close`**

file_close -- Close a file 

void `file_close`(FHANDLE fh)

Closes the file associated with fh.

This is implemented using fclose().

** Function: `file_name`**

file_name -- Returns the pathname originally associated with fh by file_open().  This is not necessarily the file's current name if it was renamed or unlinked after the fh was opened.

STR `file_name`(FHANDLE fh)

** Function: `file_openmode`**

file_open_mode -- Returns the mode the file associated with fh was opened in.

str `file_openmode`(FHANDLE fh)

** Function: `file_handles`**

file_handles -- Return a list of open files

LIST `file_handles` ()

**Input and Output Operations**

** Function: `file_readline`**

file_readline -- Reads the next line in the file and returns it (without the newline).  

str `file_readline`(FHANDLE fh)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

** Function: `file_readlines`**

file_readlines -- Rewinds the file and then reads the specified lines from the file, returning them as a list of strings.  After this operation, the stream is positioned right after the last line read.

list `file_readlines`(FHANDLE fh, INT start, INT end)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

** Function: `file_writeline`**

file_writeline -- Writes the specified line to the file (adding a newline).

void `file_writeline`(FHANDLE fh, STR line)

Not recommended for use on files in binary mode.

This is implemented using fputs()

** Function: `file_read`**

file_read -- Reads up to the specified number of bytes from the file and returns them.

str `file_read`(FHANDLE fh, INT bytes)

Not recommended for use on files in text mode.

This is implemented using fread().

** Function: `file_write`**

file_write -- Writes the specified data to the file. Returns number of bytes written.

int `file_write`(FHANDLE fh, STR data)

Not recommended for use on files in text mode.

This is implemented using fwrite().

** Function: `file_count_lines`**

file_count_lines -- count the lines in a file

INT `file_count_lines` (FHANDLER fh)

** Function: `file_grep`**

file_grep -- search for a string in a file

LIST `file_grep`(FHANDLER fh, STR search [,?match_all = 0])

Assume we have a file `test.txt` with the contents:

```
asdf asdf 11
11
112
```

And we have an open file handler from running:

```
;file_open("test.txt", "r-tn")
```

If we were to execute a file grep:

```
;file_grep(1, "11")
```

We would get the first result:

```
{{"asdf asdf 11", 1}}
```

The resulting LIST is of the form {{STR match, INT line-number}}

If you pass in the optional third argument

```
;file_grep(1, "11", 1)
```

we will receive all the matching results:

```
{{"asdf asdf 11", 1}, {"11", 2}, {"112", 3}}
```

**Getting and setting stream position**

** Function: `file_tell`**

file_tell -- Returns position in file.

INT `file_tell`(FHANDLE fh)

This is implemented using ftell().

** Function: `file_seek`**

file_seek -- Seeks to a particular location in a file.  

void `file_seek`(FHANDLE fh, INT loc, STR whence)

whence is one of the strings:

* "SEEK_SET" - seek to location relative to beginning
* "SEEK_CUR" - seek to location relative to current
* "SEEK_END" - seek to location relative to end

This is implemented using fseek().

** Function: `file_eof`**

file_eof -- Returns true if and only if fh's stream is positioned at EOF.

int `file_eof`(FHANDLE fh)

This is implemented using feof().

**Housekeeping operations**

** Function: `file_size`**

** Function: `file_last_access`**

** Function: `file_last_modify`**

** Function: `file_last_change`**

** Function: `file_size`**

int `file_size`(STR pathname)

int `file_last_access`(STR pathname)

int `file_last_modify`(STR pathname)

int `file_last_change`(STR pathname)

int `file_size`(FHANDLE filehandle)

int `file_last_access`(FHANDLE filehandle)

int `file_last_modify`(FHANDLE filehandle)

int `file_last_change`(FHANDLE filehandle)

Returns the size, last access time, last modify time, or last change time of the specified file.   All of these functions also take FHANDLE arguments and then operate on the open file.

** Function: `file_mode`**

int `file_mode`(STR filename)

int `file_mode`(FHANDLE fh)

Returns octal mode for a file (e.g. "644").

This is implemented using stat().

**file_stat**

void `file_stat`(STR pathname)

void `file_stat`(FHANDLE fh)

Returns the result of stat() (or fstat()) on the given file.

Specifically a list as follows:

`{file size in bytes, file type, file access mode, owner, group, last access, last modify, and last change}`

owner and group are always the empty string.

It is recommended that the specific information functions file_size, file_type, file_mode, file_last_access, file_last_modify, and file_last_change be used instead.  In most cases only one of these elements is desired and in those cases there's no reason to make and free a list.

** Function: `file_rename`**

file_rename - Attempts to rename the oldpath to newpath.

void `file_rename`(STR oldpath, STR newpath)

This is implemented using rename().

**file_remove**

file_remove -- Attempts to remove the given file.
 
void `file_remove`(STR pathname)

This is implemented using remove().

**Function: `file_mkdir`**

file_mkdir -- Attempts to create the given directory.

void `file_mkdir`(STR pathname)

This is implemented using mkdir().

**Function: `file_rmdir`**

file_rmdir -- Attempts to remove the given directory.

void `file_rmdir`(STR pathname)

This is implemented using rmdir().

**Function: `file_list`**

file_list -- Attempts to list the contents of the given directory.

LIST `file_list`(STR pathname, [ANY detailed])

Returns a list of files in the directory.  If the detailed argument is provided and true, then the list contains detailed entries, otherwise it contains a simple list of names.

detailed entry:

`{STR filename, STR file type, STR file mode, INT file size}`

normal entry:

STR filename

This is implemented using scandir().

**Function: `file_type`**

file_type -- Returns the type of the given pathname, one of "reg", "dir", "dev", "fifo", or "socket".

STR `file_type`(STR pathname)

This is implemented using stat().

**Function: `file_chmod`**

file_chmod -- Attempts to set mode of a file using mode as an octal string of exactly three characters.

void `file_chmod`(STR filename, STR mode)

This is implemented using chmod().

#### Operations on SQLite

SQLite allows you to store information in locally hosted SQLite databases.

**Function: `sqlite_open`**

sqlite_open -- The function `sqlite_open` will attempt to open the database at path for use with SQLite.

int `sqlite_open`(STR path to database, [INT options])

The second argument is a bitmask of options. Options are:

SQLITE_PARSE_OBJECTS [4]:    Determines whether strings beginning with a pound symbol (#) are interpreted as MOO object numbers or not. The default is true, which means that any queries that would return a string (such as "#123") will be returned as objects.

SQLITE_PARSE_TYPES [2]:      If unset, no parsing of rows takes place and only strings are returned.

SQLITE_SANITIZE_STRINGS [8]: If set, newlines (\n) are converted into tabs (\t) to avoid corrupting the MOO database. Default is unset.

> Note: If the MOO doesn't support bitmasking, you can still specify options. You'll just have to manipulate the int yourself. e.g. if you want to parse objects and types, arg[2] would be a 6. If you only want to parse types, arg[2] would be 2.

If successful, the function will return the numeric handle for the open database.

If unsuccessful, the function will return a helpful error message.

If the database is already open, a traceback will be thrown that contains the already open database handle.

**Function: `sqlite_close`**

sqlite_close -- This function will close an open database.

int `sqlite_close`(INT database handle)

If successful, return 1;

If unsuccessful, returns E_INVARG.

**Function: `sqlite_execute`**

sqlite_execute -- This function will attempt to create and execute the prepared statement query given in query on the database referred to by handle with the values values.

list | str `sqlite_execute`(INT database handle, STR SQL prepared statement query, LIST values)

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

`sqlite_execute` uses prepared statements, so it's the preferred function to use for security and performance reasons.

Example:

```
sqlite_execute(0, "INSERT INTO users VALUES (?, ?, ?);", {#7, "lisdude", "Albori Sninvel"})
```

ToastStunt supports the REGEXP pattern matching operator:

```
sqlite_execute(4, "SELECT rowid FROM notes WHERE body REGEXP ?;", {"albori (sninvel)?"})
```

> Note: This is a threaded function.

**Function: `sqlite_query`**

sqlite_query -- This function will attempt to execute the query given in query on the database referred to by handle.

list | str `sqlite_query`(INT database handle, STR database query[, INT show columns])

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

If show columns is true, the return list will include the name of the column before its results.

> Warning: sqlite_query does NOT use prepared statements and should NOT be used on queries that contain user input.

> Note: This is a threaded function.

**Function: `sqlite_limit`**

sqlite_limit -- This function allows you to specify various construct limitations on a per-database basis.

int `sqlite_limit`(INT database handle, STR category INT new value)

If new value is a negative number, the limit is unchanged. Each limit category has a hardcoded upper bound. Attempts to increase a limit above its hard upper bound are silently truncated to the hard upper bound.

Regardless of whether or not the limit was changed, the sqlite_limit() function returns the prior value of the limit. Hence, to find the current value of a limit without changing it, simply invoke this interface with the third parameter set to -1.

As of this writing, the following limits exist:

| Limit                     | Description                                                                                                                                                                                                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| LIMIT_LENGTH              | The maximum size of any string or BLOB or table row, in bytes.                                                                                                                                                                                                           |
| LIMIT_SQL_LENGTH          | The maximum length of an SQL statement, in bytes.                                                                                                                                                                                                                        |
| LIMIT_COLUMN              | The maximum number of columns in a table definition or in the result set of a SELECT or the maximum number of columns in an index or in an ORDER BY or GROUP BY clause.                                                                                                  |
| LIMIT_EXPR_DEPTH          | The maximum depth of the parse tree on any expression.                                                                                                                                                                                                                   |
| LIMIT_COMPOUND_SELECT     | The maximum number of terms in a compound SELECT statement.                                                                                                                                                                                                              |
| LIMIT_VDBE_OP             | The maximum number of instructions in a virtual machine program used to implement an SQL statement. If sqlite3_prepare_v2() or the equivalent tries to allocate space for more than this many opcodes in a single prepared statement, an SQLITE_NOMEM error is returned. |
| LIMIT_FUNCTION_ARG        | The maximum number of arguments on a function.                                                                                                                                                                                                                           |
| LIMIT_ATTACHED            | The maximum number of attached databases.                                                                                                                                                                                                                                |
| LIMIT_LIKE_PATTERN_LENGTH | The maximum length of the pattern argument to the LIKE or GLOB operators.                                                                                                                                                                                                |
| LIMIT_VARIABLE_NUMBER     | The maximum index number of any parameter in an SQL statement.                                                                                                                                                                                                           |
| LIMIT_TRIGGER_DEPTH       | The maximum depth of recursion for triggers.                                                                                                                                                                                                                             |
| LIMIT_WORKER_THREADS | The maximum number of auxiliary worker threads that a single prepared statement may start. |

For an up-to-date list of limits, see the [SQLite documentation](https://www.sqlite.org/c3ref/c_limit_attached.html).

**Function: `sqlite_last_insert_row_id`**

sqlite_last_insert_row_id -- This function identifies the row ID of the last insert command executed on the database.

int `sqlite_last_insert_row_id`(INT database handle)

**Function: `sqlite_interrupt`**

sqlite_interrupt -- This function causes any pending database operation to abort at its earliest opportunity.

none `sqlite_interrupt`(INT database handle)

If the operation is nearly finished when sqlite_interrupt is called, it might not have an opportunity to be interrupted and could continue to completion.

This can be useful when you execute a long-running query and want to abort it.

> NOTE: As of this writing (server version 2.7.0) the @kill command WILL NOT abort operations taking place in a helper thread. If you want to interrupt an SQLite query, you must use sqlite_interrupt and NOT the @kill command.

**Function: `sqlite_info`**

sqlite_info -- This function returns a map of information about the database at handle

map `sqlite_info`(INT database handle)

The information returned is:

* Database Path
* Type parsing enabled?
* Object parsing enabled?
* String sanitation enabled?

**Function: `sqlite_handles`**

sqlite_handles -- Returns a list of open SQLite database handles.

list `sqlite_handles()`

#### Operations on The Server Environment

**Function: `exec`**

exec -- Asynchronously executes the specified external executable, optionally sending input.

list `exec` (list command[, str input])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

When the process exits, it returns a list of the form:

`{code, output, error}`

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                   ⇒   {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                  ⇒   {0, "foo", ""}
exec({"echo", "one", "two"})                          ⇒   {0, "one two~0A", ""}
```

You are able to set environmental variables with `exec`, imagine you had a `vars.sh` (in your executables directory):

```
#!/bin/bash
echo "pizza = ${pizza}"
```

And then you did:

```
exec({"vars.sh"}, "", {"pizza=tasty"}) => {0, "pizza = tasty~0A", ""}
exec({"vars.sh"}) => {0, "pizza = ~0A", ""}
```

The second time pizza doesn't exist. The darkest timeline.

**Function: `getenv`**

getenv -- Returns the value of the named environment variable. 

str `getenv` (str name)

If no such environment variable exists, 0 is returned. If the programmer is not a wizard, then E_PERM is raised.

```
getenv("HOME")                                          ⇒   "/home/foobar"
getenv("XYZZY")      
```

#### Operations on Network Connections

**Function: `connected_players`**

connected_players -- returns a list of the object numbers of those player objects with currently-active connections

list `connected_players` ([include-all])

If include-all is provided and true, then the list includes the object numbers associated with _all_ current connections, including ones that are outbound and/or not yet logged-in.

**Function: `connected_seconds`**

connected_seconds -- return the number of seconds that the currently-active connection to player has existed

int `connected_seconds` (obj player) **Function: `idle_seconds`**

idle_seconds -- return the number of seconds that the currently-active connection to player has been idle

int `idle_seconds` (obj player)

If player is not the object number of a player object with a currently-active connection, then `E_INVARG` is raised.

**Function: `notify`**

notify -- enqueues string for output (on a line by itself) on the connection conn

none `notify` (obj conn, str string [, INT no-flush [, INT suppress-newline])

If the programmer is not conn or a wizard, then `E_PERM` is raised. If conn is not a currently-active connection, then this function does nothing. Output is normally written to connections only between tasks, not during execution.

The server will not queue an arbitrary amount of output for a connection; the `MAX_QUEUED_OUTPUT` compilation option (in `options.h`) controls the limit (`MAX_QUEUED_OUTPUT` can be overridden in-database by adding the property `$server_options.max_queued_output` and calling `load_server_options()`). When an attempt is made to enqueue output that would take the server over its limit, it first tries to write as much output as possible to the connection without having to wait for the other end. If that doesn't result in the new output being able to fit in the queue, the server starts throwing away the oldest lines in the queue until the new output will fit. The server remembers how many lines of output it has 'flushed' in this way and, when next it can succeed in writing anything to the connection, it first writes a line like `>> Network buffer overflow: X lines of output to you have been lost <<` where X is the number of flushed lines.

If no-flush is provided and true, then `notify()` never flushes any output from the queue; instead it immediately returns false. `Notify()` otherwise always returns true.

If suppress-newline is provided and true, then `notify()` does not add a newline add the end of the string.

**Function: `buffered_output_length`**

buffered_output_length -- returns the number of bytes currently buffered for output to the connection conn

int `buffered_output_length` ([obj conn])

If conn is not provided, returns the maximum number of bytes that will be buffered up for output on any connection.

**Function: `read`**

read -- reads and returns a line of input from the connection conn (or, if not provided, from the player that typed the command that initiated the current task)

str `read` ([obj conn [, non-blocking]])

If non-blocking is false or not provided, this function suspends the current task, resuming it when there is input available to be read. If non-blocking is provided and true, this function never suspends the calling task; if there is no input currently available for input, `read()` simply returns 0 immediately.

If player is provided, then the programmer must either be a wizard or the owner of `player`; if `player` is not provided, then `read()` may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, `E_PERM` is raised.

If the given `player` is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then `read()` raises `E_INVARG`.

The restriction on the use of `read()` without any arguments preserves the following simple invariant: if input is being read from a player, it is for the task started by the last command that player typed. This invariant adds responsibility to the programmer, however. If your program calls another verb before doing a `read()`, then either that verb must not suspend or else you must arrange that no commands will be read from the connection in the meantime. The most straightforward way to do this is to call

```
set_connection_option(player, "hold-input", 1)
```

before any task suspension could happen, then make all of your calls to `read()` and other code that might suspend, and finally call

```
set_connection_option(player, "hold-input", 0)
```

to allow commands once again to be read and interpreted normally.

**Function: `force_input`**

force_input -- inserts the string line as an input task in the queue for the connection conn, just as if it had arrived as input over the network

none `force_input` (obj conn, str line [, at-front])

If at_front is provided and true, then the new line of input is put at the front of conn's queue, so that it will be the very next line of input processed even if there is already some other input in that queue. Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

**Function: `flush_input`**

flush_input -- performs the same actions as if the connection conn's defined flush command had been received on that connection

none `flush_input` (obj conn [show-messages])

I.E., removes all pending lines of input from conn's queue and, if show-messages is provided and true, prints a message to conn listing the flushed lines, if any. See the chapter on server assumptions about the database for more information about a connection's defined flush command.

**Function: `output_delimiters`**

output_delimiters -- returns a list of two strings, the current _output prefix_ and _output suffix_ for player.

list `output_delimiters` (obj player)

If player does not have an active network connection, then `E_INVARG` is raised. If either string is currently undefined, the value `""` is used instead. See the discussion of the `PREFIX` and `SUFFIX` commands in the next chapter for more information about the output prefix and suffix.

**Function: `boot_player`**

boot_player -- marks for disconnection any currently-active connection to the given player

none `boot_player` (obj player)

The connection will not actually be closed until the currently-running task returns or suspends, but all MOO functions (such as `notify()`, `connected_players()`, and the like) immediately behave as if the connection no longer exists. If the programmer is not either a wizard or the same as player, then `E_PERM` is raised. If there is no currently-active connection to player, then this function does nothing.

If there was a currently-active connection, then the following verb call is made when the connection is actually closed:

```
$user_disconnected(player)
```

It is not an error if this verb does not exist; the call is simply skipped.

**Function: `connection_info`**

connection_info -- Returns a MAP of network connection information for `connection`. At the time of writing, the following information is returned:

list `connection_info` (OBJ `connection`)

| Key                 | Value                                                                                                                                                                                          |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| destination_address | The hostname of the connection. For incoming connections, this is the hostname of the connected user. For outbound connections, this is the hostname of the outbound connection's destination. |
| destination_ip      | The unresolved numeric IP address of the connection.                                                                                                                                           |
| destination_port    | For incoming connections, this is the local port used to make the connection. For outbound connections, this is the port the connection was made to.                                           |
| source_address      | This is the hostname of the interface an incoming connection was made on. For outbound connections, this value is meaningless.                                                                 |
| source_ip           | The unresolved numeric IP address of the interface a connection was made on. For outbound connections, this value is meaningless.                                                              |
| source_port         | The local port a connection connected to. For outbound connections, this value is meaningless.                                                                                                 |
| protocol            | Describes the protocol used to make the connection. At the time of writing, this could be IPv4 or IPv6.                                                                                        |
| outbound | Indicates whether a connection is outbound or not |

**Function: `connection_name`**

connection_name -- returns a network-specific string identifying the connection being used by the given player

str `connection_name` (obj player, [INT method])

When provided just a player object this function only returns obj's hostname (e.g. `1-2-3-6.someplace.com`). An optional argument allows you to specify 1 if you want a numeric IP address, or 2 if you want to return the legacy connection_name string.

> Warning: If you are using a LambdaMOO core, this is a semi-breaking change. You'll want to update any code on your server that runs `connection_name` to pass in the argument for returning the legacy connection_name string if you want things to work exactly the same.

If the programmer is not a wizard and not player, then `E_PERM` is raised. If player is not currently connected, then `E_INVARG` is raised.

Legacy Connection String Information:

For the TCP/IP networking configurations, for in-bound connections, the string has the form:

```
"port lport from host, port port"
```

where lport is the decimal TCP listening port on which the connection arrived, host is either the name or decimal TCP address of the host from which the player is connected, and port is the decimal TCP port of the connection on that host.

For outbound TCP/IP connections, the string has the form

```
"port lport to host, port port"
```

where lport is the decimal local TCP port number from which the connection originated, host is either the name or decimal TCP address of the host to which the connection was opened, and port is the decimal TCP port of the connection on that host.

For the System V 'local' networking configuration, the string is the UNIX login name of the connecting user or, if no such name can be found, something of the form:

```
"User #number"
```

where number is a UNIX numeric user ID.

For the other networking configurations, the string is the same for all connections and, thus, useless.

**Function: `connection_name_lookup`**

connection_name_lookup - This function performs a DNS name lookup on connection's IP address.

str `connection_name_lookup` (OBJ connection [, INT record_result])

If a hostname can't be resolved, the function simply returns the numeric IP address. Otherwise, it will return the resolved hostname.

If record_result is true, the resolved hostname will be saved with the connection and will overwrite it's existing 'connection_name()'. This means that you can call 'connection_name_lookup()' a single time when a connection is created and then continue to use 'connection_name()' as you always have in the past.

This function is primarily intended for use when the 'NO_NAME_LOOKUP' server option is set. Barring temporarily failures in your nameserver, very little will be gained by calling this when the server is performing DNS name lookups for you.

> Note: This function runs in a separate thread. While this is good for performance (long lookups won't lock your MOO like traditional pre-2.6.0 name lookups), it also means it will require slightly more work to create an entirely in-database DNS lookup solution. Because it explicitly suspends, you won't be able to use it in 'do_login_command()' without also using the 'switch_player()' function. For an example of how this can work, see '#0:do_login_command()' in ToastCore.

**Function: `switch_player`**

switch_player -- Silently switches the player associated with this connection from object1 to object2.

`switch_player`(OBJ object1, OBJ object2 [, INT silent])

object1 must be connected and object2 must be a player. This can be used in do_login_command() verbs that read or suspend (which prevents the normal player selection mechanism from working.

If silent is true, no connection messages will be printed.

> Note: This calls the listening object's user_disconnected and user_connected verbs when appropriate.

**Function: `set_connection_option`**

set_connection_option -- controls a number of optional behaviors associated the connection conn

none `set_connection_option` (obj conn, str option, value)

Raises E_INVARG if conn does not specify a current connection and E_PERM if the programmer is neither conn nor a wizard. Unless otherwise specified below, options can only be set (value is true) or unset (otherwise). The following values for option are currently supported: 

The following values for option are currently supported:

`"binary"`
When set, the connection is in binary mode, in which case both input from and output to conn can contain arbitrary bytes. Input from a connection in binary mode is not broken into lines at all; it is delivered to either the read() function or normal command parsing as binary strings, in whatever size chunks come back from the operating system. (See fine point on binary strings, for a description of the binary string representation.) For output to a connection in binary mode, the second argument to `notify()` must be a binary string; if it is malformed, E_INVARG is raised.

> Fine point: If the connection mode is changed at any time when there is pending input on the connection, said input will be delivered as per the previous mode (i.e., when switching out of binary mode, there may be pending “lines” containing tilde-escapes for embedded linebreaks, tabs, tildes and other characters; when switching into binary mode, there may be pending lines containing raw tabs and from which nonprintable characters have been silently dropped as per normal mode. Only during the initial invocation of $do_login_command() on an incoming connection or immediately after the call to open_network_connection() that creates an outgoing connection is there guaranteed not to be pending input. At other times you will probably want to flush any pending input immediately after changing the connection mode. 

`"hold-input"`

When set, no input received on conn will be treated as a command; instead, all input remains in the queue until retrieved by calls to read() or until this connection option is unset, at which point command processing resumes. Processing of out-of-band input lines is unaffected by this option. 

 `"disable-oob"`

When set, disables all out of band processing (see section Out-of-Band Processing). All subsequent input lines until the next command that unsets this option will be made available for reading tasks or normal command parsing exactly as if the out-of-band prefix and the out-of-band quoting prefix had not been defined for this server.

`"client-echo"`
The setting of this option is of no significance to the server. However calling set_connection_option() for this option sends the Telnet Protocol `WONT ECHO` or `WILL ECHO` according as value is true or false, respectively. For clients that support the Telnet Protocol, this should toggle whether or not the client echoes locally the characters typed by the user. Note that the server itself never echoes input characters under any circumstances. (This option is only available under the TCP/IP networking configurations.) 

`"flush-command"`
This option is string-valued. If the string is non-empty, then it is the flush command for this connection, by which the player can flush all queued input that has not yet been processed by the server. If the string is empty, then conn has no flush command at all. set_connection_option also allows specifying a non-string value which is equivalent to specifying the empty string. The default value of this option can be set via the property `$server_options.default_flush_command`; see Flushing Unprocessed Input for details. 

`"intrinsic-commands"`

This option value is a list of strings, each being the name of one of the available server intrinsic commands (see section Command Lines That Receive Special Treatment). Commands not on the list are disabled, i.e., treated as normal MOO commands to be handled by $do_command and/or the built-in command parser

set_connection_option also allows specifying an integer value which, if zero, is equivalent to specifying the empty list, and otherwise is taken to be the list of all available intrinsic commands (the default setting).

Thus, one way to make the verbname `PREFIX` available as an ordinary command is as follows:

```
set_connection_option(
  player, "intrinsic-commands",
  setremove(connection_options(player, "intrinsic-commands"),
            "PREFIX"));
```

Note that connection_options() with no second argument will return a list while passing in the second argument will return the value of the key requested.

```
save = connection_options(player,"intrinsic-commands");
set_connection_options(player, "intrinsic-commands, 1);
full_list = connection_options(player,"intrinsic-commands");
set_connection_options(player,"intrinsic-commands", save);
return full_list;
```

is a way of getting the full list of intrinsic commands available in the server while leaving the current connection unaffected. 

**Function: `connection_options`**

connection_options -- returns a list of `{name, value}` pairs describing the current settings of all of the allowed options for the connection conn or the value if `name` is provided

ANY `connection_options` (obj conn [, STR name])

Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

Calling connection options without a name will return a LIST. Passing in name will return only the value for the option `name` requested.

**Function: `open_network_connection`**

open_network_connection -- establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there

obj `open_network_connection` (STR host, INT port [, MAP options])

Establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there.  The new connection, as usual, will not be logged in initially and will have a negative object number associated with it for use with `read()', `notify()', and `boot_player()'.  This object number is the value returned by this function.

If the programmer is not a wizard or if the `OUTBOUND_NETWORK' compilation option was not used in building the server, then `E_PERM' is raised.

`host` refers to a string naming a host (possibly a numeric IP address) and `port` is an integer referring to a TCP port number.  If a connection cannot be made because the host does not exist, the port does not exist, the host is not reachable or refused the connection, `E_INVARG' is raised.  If the connection cannot be made for other reasons, including resource limitations, then `E_QUOTA' is raised.

Optionally, you can specify a map with any or all of the following options:

  listener: An object whose listening verbs will be called at appropriate points. (See HELP LISTEN() for more details.)

  tls:      If true, establish a secure TLS connection.

  ipv6:     If true, utilize the IPv6 protocol rather than the IPv4 protocol.

The outbound connection process involves certain steps that can take quite a long time, during which the server is not doing anything else, including responding to user commands and executing MOO tasks.  See the chapter on server assumptions about the database for details about how the server limits the amount of time it will wait for these steps to successfully complete.

It is worth mentioning one tricky point concerning the use of this function.  Since the server treats the new connection pretty much like any normal player connection, it will naturally try to parse any input from that connection as commands in the usual way.  To prevent this treatment, you should use `set_connection_option()' to set the `hold-input' option true on the connection.

Example:

```
open_network_connection("2607:5300:60:4be0::", 1234, ["ipv6" -> 1, "listener" -> #6, "tls" -> 1])
```

Open a new connection to the IPv6 address 2607:5300:60:4be0:: on port 1234 using TLS. Relevant verbs will be called on #6.

**Function: `curl`**

str `curl`(STR url [, INT include_headers, [ INT timeout])

The curl builtin will download a webpage and return it as a string. If include_headers is true, the HTTP headers will be included in the return string.

It's worth noting that the data you get back will be binary encoded. In particular, you will find that line breaks appear as ~0A. You can easily convert a page into a list by passing the return string into the decode_binary() function.

CURL_TIMEOUT is defined in options.h to specify the maximum amount of time a CURL request can take before failing. For special circumstances, you can specify a longer or shorter timeout using the third argument of curl().

**Function: `read_http`**

map `read_http` (request-or-response [, OBJ conn])

Reads lines from the connection conn (or, if not provided, from the player that typed the command that initiated the current task) and attempts to parse the lines as if they are an HTTP request or response. request-or-response must be either the string "request" or "response". It dictates the type of parsing that will be done.

Just like read(), if conn is provided, then the programmer must either be a wizard or the owner of conn; if conn is not provided, then read_http() may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, E_PERM is raised. Likewise, if conn is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then read_http() raises E_INVARG.

If parsing fails because the request or response is syntactically incorrect, read_http() will return a map with the single key "error" and a list of values describing the reason for the error. If parsing succeeds, read_http() will return a map with an appropriate subset of the following keys, with values parsed from the HTTP request or response: "method", "uri", "headers", "body", "status" and "upgrade".

 > Fine point: read_http() assumes the input strings are binary strings. When called interactively, as in the example below, the programmer must insert the literal line terminators or parsing will fail. 

The following example interactively reads an HTTP request from the players connection.

```
read_http("request", player)
GET /path HTTP/1.1~0D~0A
Host: example.com~0D~0A
~0D~0A
```

In this example, the string ~0D~0A ends the request. The call returns the following (the request has no body):

```
["headers" -> ["Host" -> "example.com"], "method" -> "GET", "uri" -> "/path"]
```

The following example interactively reads an HTTP response from the players connection.

```
read_http("response", player)
HTTP/1.1 200 Ok~0D~0A
Content-Length: 10~0D~0A
~0D~0A
1234567890
```

The call returns the following:

```
["body" -> "1234567890", "headers" -> ["Content-Length" -> "10"], "status" -> 200]
```

**Function: `listen`**

listen -- create a new point at which the server will listen for network connections, just as it does normally

value `listen` (obj object, port [, MAP options])

Create a new point at which the server will listen for network connections, just as it does normally. `Object` is the object whose verbs `do_login_command', `do_command', `do_out_of_band_command', `user_connected', `user_created', `user_reconnected', `user_disconnected', and `user_client_disconnected' will be called at appropriate points as these verbs are called on #0 for normal connections. (See the chapter in the LambdaMOO Programmer's Manual on server assumptions about the database for the complete story on when these functions are called.) `Port` is a TCP port number on which to listen. The listen() function will return `port` unless `port` is zero, in which case the return value is a port number assigned by the operating system.

An optional third argument allows you to set various miscellaneous options for the listening point. These are:

  print-messages: If true, the various database-configurable messages (also detailed in the chapter on server assumptions) will be printed on connections received at the new listening port.

  ipv6:           Use the IPv6 protocol rather than IPv4.

  tls:            Only accept valid secure TLS connections.

  certificate:    The full path to a TLS certificate. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the certificate differs from the one specified in options.h.

  key:            The full path to a TLS private key. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the key differs from the one specified in options.h.

listen() raises E_PERM if the programmer is not a wizard, E_INVARG if `object` is invalid or there is already a listening point described by `point`, and E_QUOTA if some network-configuration-specific error occurred.

Example:

```
listen(#0, 1234, ["ipv6" -> 1, "tls" -> 1, "certificate" -> "/etc/certs/something.pem", "key" -> "/etc/certs/privkey.pem", "print-messages" -> 1]
```

Listen for IPv6 connections on port 1234 and print messages as appropriate. These connections must be TLS and will use the private key and certificate found in /etc/certs.

**Function: `unlisten`**

unlisten -- stop listening for connections on the point described by canon, which should be the second element of some element of the list returned by `listeners()`

none `unlisten` (canon)

Raises `E_PERM` if the programmer is not a wizard and `E_INVARG` if there does not exist a listener with that description.

**Function: `listeners`**

listeners -- returns a list describing all existing listening points, including the default one set up automatically by the server when it was started (unless that one has since been destroyed by a call to `unlisten()`)

list `listeners` ()

Each element of the list has the following form:

```
{object, canon, print-messages}
```

where object is the first argument given in the call to `listen()` to create this listening point, print-messages is true if the third argument in that call was provided and true, and canon was the value returned by that call. (For the initial listening point, object is `#0`, canon is determined by the command-line arguments or a network-configuration-specific default, and print-messages is true.)

Please note that there is nothing special about the initial listening point created by the server when it starts; you can use `unlisten()` on it just as if it had been created by `listen()`. This can be useful; for example, under one of the TCP/IP configurations, you might start up your server on some obscure port, say 12345, connect to it by yourself for a while, and then open it up to normal users by evaluating the statements:

```
unlisten(12345); listen(#0, 7777, 1)
```

#### Operations Involving Times and Dates

**Function: `time`**

time -- returns the current time, represented as the number of seconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time

int `time` ()

**Function: `ftime`**

ftime -- Returns the current time represented as the number of seconds and nanoseconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time.

float `ftime` ([INT monotonic])

If the `monotonic` argument is supplied and set to 1, the time returned will be monotonic. This means that will you will always get how much time has elapsed from an arbitrary, fixed point in the past that is unaffected by clock skew or other changes in the wall-clock. This is useful for benchmarking how long an operation takes, as it's unaffected by the actual system time.

The general rule of thumb is that you should use ftime() with no arguments for telling time and ftime() with the monotonic clock argument for measuring the passage of time.

**Function: `ctime`**

ctime -- interprets time as a time, using the same representation as given in the description of `time()`, above, and converts it into a 28-character, human-readable string

str `ctime` ([int time])

The string will be in the following format:

```
Mon Aug 13 19:13:20 1990 PDT
```

If the current day of the month is less than 10, then an extra blank appears between the month and the day:

```
Mon Apr  1 14:10:43 1991 PST
```

If time is not provided, then the current time is used.

Note that `ctime()` interprets time for the local time zone of the computer on which the MOO server is running.

#### MOO-Code Evaluation and Task Manipulation

**Function: `raise`**

raise -- raises code as an error in the same way as other MOO expressions, statements, and functions do

none `raise` (code [, str message [, value]])

Message, which defaults to the value of `tostr(code)`, and value, which defaults to zero, are made available to any `try`-`except` statements that catch the error. If the error is not caught, then message will appear on the first line of the traceback printed to the user.

**Function: `call_function`**

call_function -- calls the built-in function named func-name, passing the given arguments, and returns whatever that function returns

value `call_function` (str func-name, arg, ...)

Raises `E_INVARG` if func-name is not recognized as the name of a known built-in function.  This allows you to compute the name of the function to call and, in particular, allows you to write a call to a built-in function that may or may not exist in the particular version of the server you're using.

**Function: `function_info`**

function_info -- returns descriptions of the built-in functions available on the server

list `function_info` ([str name])

If name is provided, only the description of the function with that name is returned. If name is omitted, a list of descriptions is returned, one for each function available on the server. Raised `E_INVARG` if name is provided but no function with that name is available on the server.

Each function description is a list of the following form:

```
{name, min-args, max-args, types
```

where name is the name of the built-in function, min-args is the minimum number of arguments that must be provided to the function, max-args is the maximum number of arguments that can be provided to the function or `-1` if there is no maximum, and types is a list of max-args integers (or min-args if max-args is `-1`), each of which represents the type of argument required in the corresponding position. Each type number is as would be returned from the `typeof()` built-in function except that `-1` indicates that any type of value is acceptable and `-2` indicates that either integers or floating-point numbers may be given. For example, here are several entries from the list:

```
{"listdelete", 2, 2, {4, 0}}
{"suspend", 0, 1, {0}}
{"server_log", 1, 2, {2, -1}}
{"max", 1, -1, {-2}}
{"tostr", 0, -1, {}}
```

`listdelete()` takes exactly 2 arguments, of which the first must be a list (`LIST == 4`) and the second must be an integer (`INT == 0`).  `suspend()` has one optional argument that, if provided, must be a number (integer or float). `server_log()` has one required argument that must be a string (`STR == 2`) and one optional argument that, if provided, may be of any type.  `max()` requires at least one argument but can take any number above that, and the first argument must be either an integer or a floating-point number; the type(s) required for any other arguments can't be determined from this description. Finally, `tostr()` takes any number of arguments at all, but it can't be determined from this description which argument types would be acceptable in which positions.

**Function: `eval`**

eval -- the MOO-code compiler processes string as if it were to be the program associated with some verb and, if no errors are found, that fictional verb is invoked

list `eval` (str string)

If the programmer is not, in fact, a programmer, then `E_PERM` is raised. The normal result of calling `eval()` is a two element list.  The first element is true if there were no compilation errors and false otherwise. The second element is either the result returned from the fictional verb (if there were no compilation errors) or a list of the compiler's error messages (otherwise).

When the fictional verb is invoked, the various built-in variables have values as shown below:

player    the same as in the calling verb
this      #-1
caller    the same as the initial value of this in the calling verb

args      {}
argstr    ""

verb      ""
dobjstr   ""
dobj      #-1
prepstr   ""
iobjstr   ""
iobj      #-1

The fictional verb runs with the permissions of the programmer and as if its `d` permissions bit were on.

```
eval("return 3 + 4;")   =>   {1, 7}
```

**Function: `set_task_perms`**

set_task_perms -- changes the permissions with which the currently-executing verb is running to be those of who

one `set_task_perms` (obj who)

If the programmer is neither who nor a wizard, then `E_PERM` is raised.
> Note: This does not change the owner of the currently-running verb, only the permissions of this particular invocation. It is used in verbs owned by wizards to make themselves run with lesser (usually non-wizard) permissions.

**Function: `caller_perms`**

caller_perms -- returns the permissions in use by the verb that called the currently-executing verb

obj `caller_perms` ()

If the currently-executing verb was not called by another verb (i.e., it is the first verb called in a command or server task), then `caller_perms()` returns `#-1`.

**Function: `set_task_local`**

set_task_local -- Sets a value that gets associated with the current running task. 

void set_task_local(ANY value)

This value persists across verb calls and gets reset when the task is killed, making it suitable for securely passing sensitive intermediate data between verbs. The value can then later be retrieved using the `task_local` function.

```
set_task_local("arbitrary data")
set_task_local({"list", "of", "arbitrary", "data"})
```

**Function: `task_local`**

task_local -- Returns the value associated with the current task. The value is set with the `set_task_local` function.

mixed `task_local` ()

**Function: `threads`**

threads -- When one or more MOO processes are suspended and working in a separate thread, this function will return a LIST of handlers to those threads. These handlers can then be passed to `thread_info' for more information.

list `threads`()

**Function: `set_thread_mode`**

int `set_thread_mode`([INT mode])

With no arguments specified, set_thread_mode will return the current thread mode for the verb. A value of 1 indicates that threading is enabled for functions that support it. A value of 0 indicates that threading is disabled and all functions will execute in the main MOO thread, as functions have done in default LambdaMOO since version 1.

If you specify an argument, you can control the thread mode of the current verb. A mode of 1 will enable threading and a mode of 0 will disable it. You can invoke this function multiple times if you want to disable threading for a single function call and enable it for the rest.

When should you disable threading? In general, threading should be disabled in verbs where it would be undesirable to suspend(). Each threaded function will immediately suspend the verb while the thread carries out its work. This can have a negative effect when you want to use these functions in verbs that cannot or should not suspend, like $sysobj:do_command or $sysobj:do_login_command.

Note that the threading mode affects the current verb only and does NOT affect verbs called from within that verb.

**Function: `thread_info`**

thread_info -- If a MOO task is running in another thread, its thread handler will give you information about that thread. 

list `thread_info`(INT thread handler)

The information returned in a LIST will be:

English Name: This is the name the programmer of the builtin function has given to the task being executed.

Active: 1 or 0 depending upon whether or not the MOO task has been killed. Not all threads cleanup immediately after the MOO task dies.

**Function: `thread_pool`**

void `thread_pool`(STR function, STR pool [, INT value])

This function allows you to control any thread pools that the server created at startup. It should be used with care, as it has the potential to create disasterous consequences if used incorrectly.

The function parameter is the function you wish to perform on the thread pool. The functions available are:

INIT: Control initialization of a thread pool.

The pool parameter controls which thread pool you wish to apply the designated function to. At the time of writing, the server creates the following thread pool:

MAIN: The main thread pool where threaded built-in function work takes place.

Finally, value is the value you want to pass to the function of pool. The following functions accept the following values:

INIT: The number of threads to spawn. NOTE: When executing this function, the existing pool will be destroyed and a new one created in its place.

Examples:

```
thread_pool("INIT", "MAIN", 1)     => Replace the existing main thread pool with a new pool consisting of a single thread.
```

**Function: `ticks_left`**

ticks_left -- return the number of ticks left to the current task before it will be forcibly terminated

int `ticks_left` () **Function: `seconds_left`**

seconds_left -- return the number of seconds left to the current task before it will be forcibly terminated

int `seconds_left` ()

These are useful, for example, in deciding when to call `suspend()` to continue a long-lived computation.

**Function: `task_id`**

task_id -- returns the non-zero, non-negative integer identifier for the currently-executing task

int `task_id` ()

Such integers are randomly selected for each task and can therefore safely be used in circumstances where unpredictability is required.

**Function: `suspend`**

suspend -- suspends the current task, and resumes it after at least seconds seconds

value `suspend` ([int|float seconds])

Sub-second suspend (IE: 0.1) is possible. If seconds is not provided, the task is suspended indefinitely; such a task can only be resumed by use of the `resume()` function.

When the task is resumed, it will have a full quota of ticks and seconds. This function is useful for programs that run for a long time or require a lot of ticks. If seconds is negative, then `E_INVARG` is raised. `Suspend()` returns zero unless it was resumed via `resume()`, in which case it returns the second argument given to that function.

In some sense, this function forks the 'rest' of the executing task. However, there is a major difference between the use of `suspend(seconds)` and the use of the `fork (seconds)`. The `fork` statement creates a new task (a _forked task_) while the currently-running task still goes on to completion, but a `suspend()` suspends the currently-running task (thus making it into a _suspended task_). This difference may be best explained by the following examples, in which one verb calls another:

```
.program   #0:caller_A
#0.prop = 1;
#0:callee_A();
#0.prop = 2;
.

.program   #0:callee_A
fork(5)
  #0.prop = 3;
endfork
.

.program   #0:caller_B
#0.prop = 1;
#0:callee_B();
#0.prop = 2;
.

.program   #0:callee_B
suspend(5);
#0.prop = 3;
.
```

Consider `#0:caller_A`, which calls `#0:callee_A`. Such a task would assign 1 to `#0.prop`, call `#0:callee_A`, fork a new task, return to `#0:caller_A`, and assign 2 to `#0.prop`, ending this task. Five seconds later, if the forked task had not been killed, then it would begin to run; it would assign 3 to `#0.prop` and then stop. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 3.

Now consider `#0:caller_B`, which calls `#0:callee_B` instead of `#0:callee_A`. This task would assign 1 to `#0.prop`, call `#0:callee_B`, and suspend. Five seconds later, if the suspended task had not been killed, then it would resume; it would assign 3 to `#0.prop`, return to `#0:caller_B`, and assign 2 to `#0.prop`, ending the task. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 2.

A suspended task, like a forked task, can be described by the `queued_tasks()` function and killed by the `kill_task()` function. Suspending a task does not change its task id. A task can be suspended again and again by successive calls to `suspend()`.

By default, there is no limit to the number of tasks any player may suspend, but such a limit can be imposed from within the database. See the chapter on server assumptions about the database for details.

**Function: `resume`**

resume -- immediately ends the suspension of the suspended task with the given task-id; that task's call to `suspend()` will return value, which defaults to zero

none `resume` (int task-id [, value])

If value is of type `ERR`, it will be raised, rather than returned, in the suspended task. `Resume()` raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

**Function: `yin`**

yin -- Suspend the current task if it's running out of ticks or seconds.

int `yin`([INT time, INT minimum ticks, INT minimum seconds] )

`yin` stands for yield if needed.

This is meant to provide similar functionality to the LambdaCore-based suspend_if_needed verb or manually specifying something like: ticks_left() < 2000 && suspend(0)

Time: How long to suspend the task. Default: 0

Minimum ticks: The minimum number of ticks the task has left before suspending.

Minimum seconds: The minimum number of seconds the task has left before suspending.

**Function: `queue_info`**

queue_info -- if player is omitted, returns a list of object numbers naming all players that currently have active task queues inside the server

list `queue_info` ([obj player])
map `queue_info` ([obj player])

If player is provided, returns the number of background tasks currently queued for that user. It is guaranteed that `queue_info(X)` will return zero for any X not in the result of `queue_info()`.

If the caller is a wizard a map of debug information about task queues will be returned.

**Function: `queued_tasks`**

queued_tasks -- returns information on each of the background tasks (i.e., forked, suspended or reading) owned by the programmer (or, if the programmer is a wizard, all queued tasks)

list `queued_tasks` ([INT show-runtime [, INT count-only])

The returned value is a list of lists, each of which encodes certain information about a particular queued task in the following format:

```
{task-id, start-time, x, y, programmer, verb-loc, verb-name, line, this, task-size}
```

where task-id is an integer identifier for this queued task, start-time is the time after which this task will begin execution (in time() format), x and y are obsolete values that are no longer interesting, programmer is the permissions with which this task will begin execution (and also the player who owns this task), verb-loc is the object on which the verb that forked this task was defined at the time, verb-name is that name of that verb, line is the number of the first line of the code in that verb that this task will execute, this is the value of the variable `this` in that verb, and task-size is the size of the task in bytes. For reading tasks, start-time is -1. 

The x and y fields are now obsolete and are retained only for backward-compatibility reasons. They may be reused for new purposes in some future version of the server.

If `show-runtime` is true, all variables present in the task are presented in a map with the variable name as the key and its value as the value.     

If `count-only` is true, then only the number of tasks is returned. This is significantly more performant than length(queued_tasks()).

> Warning: If you are upgrading to ToastStunt from a version of LambdaMOO prior to 1.8.1 you will need to dump your database, reboot into LambdaMOO emergency mode, and kill all your queued_tasks() before dumping the DB again. Otherwise, your DB will not boot into ToastStunt.

**Function: `kill_task`**

kill_task -- removes the task with the given task-id from the queue of waiting tasks

none `kill_task` (int task-id)

If the programmer is not the owner of that task and not a wizard, then `E_PERM` is raised. If there is no task on the queue with the given task-id, then `E_INVARG` is raised.

**Function: `finished_tasks()`**

finished_tasks -- returns a list of the last X tasks to finish executing, including their total execution time

list `finished_tasks`()

When enabled (via SAVE_FINISHED_TASKS in options.h), the server will keep track of the execution time of every task that passes through the interpreter. This data is then made available to the database in two ways.

The first is the finished_tasks() function. This function will return a list of maps of the last several finished tasks (configurable via $server_options.finished_tasks_limit) with the following information:

| Value      | Description                                                                           |
| ---------- | ------------------------------------------------------------------------------------- |
| foreground | 1 if the task was a foreground task, 0 if it was a background task                    |
| fullverb   | the full name of the verb, including aliases                                          |
| object     | the object that defines the verb                                                      |
| player     | the player that initiated the task                                                    |
| programmer | the programmer who owns the verb                                                      |
| receiver   | typically the same as 'this' but could be the handler in the case of primitive values |
| suspended  | whether the task was suspended or not                                                 |
| this       | the actual object the verb was called on                                              |
| time | the total time it took the verb to run), and verb (the name of the verb call or command typed |

The second is via the $handle_lagging_task verb. When the execution threshold defined in $server_options.task_lag_threshold is exceeded, the server will write an entry to the log file and call the $handle_lagging_task verb with the call stack of the task as well as the execution time.

> Note: This builtin must be enabled in options.h to be used.

**Function: `callers`**

callers -- returns information on each of the verbs and built-in functions currently waiting to resume execution in the current task

list `callers` ([include-line-numbers])

When one verb or function calls another verb or function, execution of the caller is temporarily suspended, pending the called verb or function returning a value. At any given time, there could be several such pending verbs and functions: the one that called the currently executing verb, the verb or function that called that one, and so on. The result of `callers()` is a list, each element of which gives information about one pending verb or function in the following format:

```
{this, verb-name, programmer, verb-loc, player, line-number}
```

For verbs, this is the initial value of the variable `this` in that verb, verb-name is the name used to invoke that verb, programmer is the player with whose permissions that verb is running, verb-loc is the object on which that verb is defined, player is the initial value of the variable `player` in that verb, and line-number indicates which line of the verb's code is executing. The line-number element is included only if the include-line-numbers argument was provided and true.

For functions, this, programmer, and verb-loc are all `#-1`, verb-name is the name of the function, and line-number is an index used internally to determine the current state of the built-in function. The simplest correct test for a built-in function entry is

```
(VERB-LOC == #-1  &&  PROGRAMMER == #-1  &&  VERB-name != "")
```

The first element of the list returned by `callers()` gives information on the verb that called the currently-executing verb, the second element describes the verb that called that one, and so on. The last element of the list describes the first verb called in this task.

**Function: `task_stack`**

task_stack -- returns information like that returned by the `callers()` function, but for the suspended task with the given task-id; the include-line-numbers argument has the same meaning as in `callers()`

list `task_stack` (int task-id [, INT include-line-numbers [, INT include-variables])

Raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

If include-line-numbers is passed and true, line numbers will be included.

If include-variables is passed and true, variables will be included with each frame of the provided task.

#### Administrative Operations

**Function: `server_version`**

server_version -- returns a string giving the version number of the running MOO server

str `server_version` ([int with-details])

If with-details is provided and true, returns a detailed list including version number as well as compilation options.

**Function `load_server_options`**
load_server_options -- This causes the server to consult the current values of properties on $server_options, updating the corresponding serveroption settings

none `load_server_options` ()

For more information see section Server Options Set in the Database.. If the programmer is not a wizard, then E_PERM is raised.

**Function: `server_log`**

server_log -- The text in message is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages)

none server_log (str message [, int level])

If the programmer is not a wizard, then E_PERM is raised. 

If level is provided and is an integer between 0 and 7 inclusive, then message is marked in the server log as one of eight predefined types, from simple log message to error message. Otherwise, if level is provided and true, then message is marked in the server log as an error.

**Function: `renumber`**

renumber -- the object number of the object currently numbered object is changed to be the least nonnegative object number not currently in use and the new object number is returned

obj `renumber` (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised. If there are no unused nonnegative object numbers less than object, then object is returned and no changes take place.

The references to object in the parent/children and location/contents hierarchies are updated to use the new object number, and any verbs, properties and/or objects owned by object are also changed to be owned by the new object number. The latter operation can be quite time consuming if the database is large. No other changes to the database are performed; in particular, no object references in property values or verb code are updated.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

**Function: `reset_max_object`**

reset_max_object -- the server's idea of the highest object number ever used is changed to be the highest object number of a currently-existing object, thus allowing reuse of any higher numbers that refer to now-recycled objects

none `reset_max_object` ()

If the programmer is not a wizard, then `E_PERM` is raised.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

**Function: `memory_usage`**

memory_usage -- Return statistics concerning the server's consumption of system memory.

list `memory_usage` ()

The result is a list in the following format:

{total memory used, resident set size, shared pages, text, data + stack}

**Function: `usage`**

usage -- Return statistics concerning the server the MOO is running on.

list `usage` ()

The result is a list in the following format:

```
{{load averages}, user time, system time, page reclaims, page faults, block input ops, block output ops, voluntary context switches, involuntary context switches, signals received}
```

**Function: `dump_database`**

dump_database -- requests that the server checkpoint the database at its next opportunity

none `dump_database` ()

It is not normally necessary to call this function; the server automatically checkpoints the database at regular intervals; see the chapter on server assumptions about the database for details. If the programmer is not a wizard, then `E_PERM` is raised.

**Function: `panic`**

panic -- Unceremoniously shut down the server, mimicking the behavior of a fatal error.

void panic([STR message])

The database will NOT be dumped to the file specified when starting the server. A new file will be created with the name of your database appended with .PANIC.

> Warning: Don't run this unless you really want to panic your server.

**Function: `db_disk_size`**

db_disk_size -- returns the total size, in bytes, of the most recent full representation of the database as one or more disk files

int `db_disk_size` ()

Raises `E_QUOTA` if, for some reason, no such on-disk representation is currently available.

**Function: `exec`**

exec -- Asynchronously executes the specified external executable, optionally sending input. 

list `exec` (LIST command[, STR input][, LIST environment variables])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

Additionally, you can provide a list of environment variables to set in the shell.

When the process exits, it returns a list of the form:

```
{code, output, error}
```

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                      {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                     {0, "foo", ""}
exec({"echo", "one", "two"})                             {0, "one two~0A", ""}
```

**Function: `shutdown`**

shutdown -- requests that the server shut itself down at its next opportunity

none `shutdown` ([str message])

Before doing so, a notice (incorporating message, if provided) is printed to all connected players. If the programmer is not a wizard, then `E_PERM` is raised.

**Function: `verb_cache_stats`**

**Function: `log_cache_stats`**

list verb_cache_stats ()

none log_cache_stats ()

The server caches verbname-to-program lookups to improve performance. These functions respectively return or write to the server log file the current cache statistics. For verb_cache_stats the return value will be a list of the form

```
{hits, negative_hits, misses, table_clears, histogram},
```

though this may change in future server releases. The cache is invalidated by any builtin function call that may have an effect on verb lookups (e.g., delete_verb()). 


