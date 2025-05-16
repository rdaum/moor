# Manipulating MOO Values

There are several functions for performing primitive operations on MOO values, and they can be cleanly split into two kinds: those that do various very general operations that apply to all types of values, and those that are specific to one particular type. There are so many operations concerned with objects that we do not list them in this section but rather give them their own section following this one.

## General Operations Applicable to All Values

### Function: `typeof`

```
int typeof(value)
```

Takes any MOO value and returns an integer representing the type of value.

The result is the same as the initial value of one of these built-in variables: `INT`, `FLOAT`, `STR`, `LIST`, `OBJ`, or `ERR`, `BOOL`, `MAP`, `WAIF`, `ANON`. Thus, one usually writes code like this:

```
if (typeof(x) == LIST) ...
```

and not like this:

```
if (typeof(x) == 3) ...
```

because the former is much more readable than the latter.

### Function: `tostr`

```
str tostr(value, ...)
```

Converts all of the given MOO values into strings and returns the concatenation of the results.

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

Warning `tostr()` does not do a good job of converting lists and maps into strings; all lists, including the empty list, are converted into the string `"{list}"` and all maps are converted into the string `"[map]"`. The function `toliteral()`, below, is better for this purpose.

### Function: `toliteral`

```
str toliteral(value)
```

Returns a string containing a MOO literal expression that, when evaluated, would be equal to value.

```
toliteral(17)         =>   "17"
toliteral(1.0/3.0)    =>   "0.333333333333333"
toliteral(#17)        =>   "#17"
toliteral("foo")      =>   "\"foo\""
toliteral({1, 2})     =>   "{1, 2}"
toliteral([1 -> 2]    =>   "[1 -> 2]"
toliteral(E_PERM)     =>   "E_PERM"
```

### Function: `toint`

```
int toint(value)
```

Converts the given MOO value into an integer and returns that integer.

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

### Function: `toobj`

```
obj toobj(value)
```

Converts the given MOO value into an object number and returns that object number.

The conversions are very similar to those for `toint()` except that for strings, the number _may_ be preceded by `#`.

```
toobj("34")       =>   #34
toobj("#34")      =>   #34
toobj("foo")      =>   #0
toobj({1, 2})     =>   E_TYPE (error)
```

### Function: `tofloat`

```
float tofloat(value)
```

Converts the given MOO value into a floating-point number and returns that number.

Integers and object numbers are converted into the corresponding integral floating-point numbers. Strings are parsed as the decimal encoding of a real number which is then represented as closely as possible as a floating-point number. Errors are first converted to integers as in `toint()` and then converted as integers are. `tofloat()` raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number, then `tofloat()` returns 0.

```
tofloat(34)          =>   34.0
tofloat(#34)         =>   34.0
tofloat("34")        =>   34.0
tofloat("34.7")      =>   34.7
tofloat(E_TYPE)      =>   1.0
```

### Function: `equal`

```
int equal(value, value2)
```

Returns true if value1 is completely indistinguishable from value2.

This is much the same operation as `value1 == value2` except that, unlike `==`, the `equal()` function does not treat upper- and lower-case characters in strings as equal and thus, is case-sensitive.

```
"Foo" == "foo"         =>   1
equal("Foo", "foo")    =>   0
equal("Foo", "Foo")    =>   1
```

### Function: `value_bytes`

```
int value_bytes(value)
```

Returns the number of bytes of the server's memory required to store the given value.

### Function: `value_hash`

```
str value_hash(value, [, str algo] [, binary])
```

Returns the same string as `string_hash(toliteral(value))`.

See the description of `string_hash()` for details.

### Function: `value_hmac`

```
str value_hmac(value, STR key [, STR algo [, binary]])
```

Returns the same string as string_hmac(toliteral(value), key)

See the description of string_hmac() for details.

### Function: `generate_json`

```
str generate_json(value [, str mode])
```

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

### Function: `parse_json`

```
value parse_json(str json [, str mode])
```

Returns the MOO value representation of the JSON string.

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

## Operations on Numbers

### Function: `random`

```
int random([int mod, [int range]])
```

random -- Return a random integer

mod must be a positive integer; otherwise, `E_INVARG` is raised. If mod is not provided, it defaults to the largest MOO integer, which will depend on if you are running 32 or 64-bit.

if range is provided then an integer in the range of mod to range (inclusive) is returned.

```
random(10)                  => integer between 1-10
random()                    => integer between 1 and maximum integer supported
random(1, 5000)             => integer between 1 and 5000
```

### Function: `frandom`

```
float frandom(FLOAT mod1 [, FLOAT mod2)
```

If only one argument is given, a floating point number is chosen randomly from the range `[1.0..mod1]` and returned. If two arguments are given, a floating point number is randomly chosen from the range `[mod1..mod2]`.

### Function: `random_bytes`

```
int random_bytes(int count)
```

Returns a binary string composed of between one and 10000 random bytes. count specifies the number of bytes and must be a positive integer; otherwise, E_INVARG is raised.

### Function: `reseed_random`

```
void reseed_random()
```

Provide a new seed to the pseudo random number generator.

### Function: `min`

```
int min(int x, ...)
```

Return the smallest of it's arguments.

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

### Function: `max`

```
int max(int x, ...)
```

Return the largest of it's arguments.

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

### Function: `abs`

```
int abs(int x)
```

Returns the absolute value of x.

If x is negative, then the result is `-x`; otherwise, the result is x. The number x can be either integer or floating-point; the result is of the same kind.

### Function: `exp`

```
float exp(FLOAT x)
```

Returns E (Eulers number) raised to the power of x.

### Function: `floatstr`

```
str floatstr(float x, int precision [, scientific])
```

Converts x into a string with more control than provided by either `tostr()` or `toliteral()`.

Precision is the number of digits to appear to the right of the decimal point, capped at 4 more than the maximum available precision, a total of 19 on most machines; this makes it possible to avoid rounding errors if the resulting string is subsequently read back as a floating-point value. If scientific is false or not provided, the result is a string in the form `"MMMMMMM.DDDDDD"`, preceded by a minus sign if and only if x is negative. If scientific is provided and true, the result is a string in the form `"M.DDDDDDe+EEE"`, again preceded by a minus sign if and only if x is negative.

### Function: `sqrt`

```
float sqrt(float x)
```

Returns the square root of x.

Raises `E_INVARG` if x is negative.

### Function: `sin`

```
float sin(float x)
```

Returns the sine of x.

### Function: `cos`

```
float cos(float x)
```

Returns the cosine of x.

### Function: `tangent`

```
float tan(float x)
```

Returns the tangent of x.

### Function: `asin`

```
float asin(float x)
```

Returns the arc-sine (inverse sine) of x, in the range `[-pi/2..pi/2]`

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

### Function: `acos`

```
float acos(float x)
```

Returns the arc-cosine (inverse cosine) of x, in the range `[0..pi]`

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

### Function: `atan`

```
float atan(float y [, float x])
```

Returns the arc-tangent (inverse tangent) of y in the range `[-pi/2..pi/2]`.

if x is not provided, or of `y/x` in the range `[-pi..pi]` if x is provided.

### Function: `sinh`

```
float sinh(float x)
```

Returns the hyperbolic sine of x.

### Function: `cosh`

```
float cosh(float x)
```

Returns the hyperbolic cosine of x.

### Function: `tanh`

```
float tanh(float x)
```

Returns the hyperbolic tangent of x.

### Function: `exp`

```
float exp(float x)
```

Returns e raised to the power of x.

### Function: `log`

```
float log(float x)
```

Returns the natural logarithm of x.

Raises `E_INVARG` if x is not positive.

### Function: `log10`

```
float log10(float x)
```

Returns the base 10 logarithm of x.

Raises `E_INVARG` if x is not positive.

### Function: `ceil`

```
float ceil(float x)
```

Returns the smallest integer not less than x, as a floating-point number.

### Function: `floor`

```
float floor(float x)
```

Returns the largest integer not greater than x, as a floating-point number.

### Function: `trunc`

```
float trunc(float x)
```

Returns the integer obtained by truncating x at the decimal point, as a floating-point number.

For negative x, this is equivalent to `ceil()`; otherwise it is equivalent to `floor()`.

## Operations on Strings

### Function: `length`

```
int length(str string)
```

Returns the number of characters in string.

It is also permissible to pass a list to `length()`; see the description in the next section.

```
length("foo")   =>   3
length("")      =>   0
```

### Function: `strsub`

```
str strsub(str subject, str what, str with [, int case-matters])
```

Replaces all occurrences of what in subject with with, performing string substitution.

The occurrences are found from left to right and all substitutions happen simultaneously. By default, occurrences of what are searched for while ignoring the upper/lower case distinction. If case-matters is provided and true, then case is treated as significant in all comparisons.

```
strsub("%n is a fink.", "%n", "Fred")   =>   "Fred is a fink."
strsub("foobar", "OB", "b")             =>   "fobar"
strsub("foobar", "OB", "b", 1)          =>   "foobar"
```

### Functions: `index`, `rindex`

index -- Returns the index of the first character of the first occurrence of str2 in str1.

rindex -- Returns the index of the first character of the last occurrence of str2 in str1.

```
int index(str str1, str str2, [, int case-matters [, int skip])
int rindex(str str1, str str2, [, int case-matters [, int skip])
```

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

### Function: `strtr`

```
int strtr(str source, str str1, str str2 [, case-matters])
```

Transforms the string source by replacing the characters specified by str1 with the corresponding characters specified by str2.

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

### Function: `strcmp`

```
int strcmp(str str1, str str2)
```

Performs a case-sensitive comparison of the two argument strings.

If str1 is [lexicographically](https://en.wikipedia.org/wiki/Lexicographical_order) less than str2, the `strcmp()` returns a negative integer. If the two strings are identical, `strcmp()` returns zero. Otherwise, `strcmp()` returns a positive integer. The ASCII character ordering is used for the comparison.

### Function: `explode`

```
list explodeSTR subject [, STR break [, INT include-sequential-occurrences])
```

Returns a list of substrings of subject that are separated by break. break defaults to a space.

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

### Function: `decode_binary`

```
list decode_binary(str bin-string [, int fully])
```

Returns a list of strings and/or integers representing the bytes in the binary string bin_string in order.

If fully is false or omitted, the list contains an integer only for each non-printing, non-space byte; all other characters are grouped into the longest possible contiguous substrings. If fully is provided and true, the list contains only integers, one for each byte represented in bin_string. Raises `E_INVARG` if bin_string is not a properly-formed binary string. (See the early section on MOO value types for a full description of binary strings.)

```
decode_binary("foo")               =>   {"foo"}
decode_binary("~~foo")             =>   {"~foo"}
decode_binary("foo~0D~0A")         =>   {"foo", 13, 10}
decode_binary("foo~0Abar~0Abaz")   =>   {"foo", 10, "bar", 10, "baz"}
decode_binary("foo~0D~0A", 1)      =>   {102, 111, 111, 13, 10}
```

### Function: `encode_binary`

```
str encode_binary(arg, ...)
```

Translates each integer and string in turn into its binary string equivalent, returning the concatenation of all these substrings into a single binary string.

Each argument must be an integer between 0 and 255, a string, or a list containing only legal arguments for this function. This function (See the early section on MOO value types for a full description of binary strings.)

```
encode_binary("~foo")                     =>   "~7Efoo"
encode_binary({"foo", 10}, {"bar", 13})   =>   "foo~0Abar~0D"
encode_binary("foo", 10, "bar", 13)       =>   "foo~0Abar~0D"
```

### Function: `decode_base64`

```
str decode_base64(str base64 [, int safe])
```

Returns the binary string representation of the supplied Base64 encoded string argument.

Raises E_INVARG if base64 is not a properly-formed Base64 string. If safe is provide and is true, a URL-safe version of Base64 is used (see RFC4648).

```
decode_base64("AAEC")      ⇒    "~00~01~02"
decode_base64("AAE", 1)    ⇒    "~00~01"
```

### Function: `encode_base64`

```
str encode_base64(str binary [, int safe])
```

Returns the Base64 encoded string representation of the supplied binary string argument.

Raises E_INVARG if binary is not a properly-formed binary string. If safe is provide and is true, a URL-safe version of Base64 is used (see [RFC4648](https://datatracker.ietf.org/doc/html/rfc4648)).

```
encode_base64("~00~01~02")    ⇒    "AAEC"
encode_base64("~00~01", 1)    ⇒    "AAE"
```

### Function: `spellcheck`

```
int | list spellcheck(STR word)
```

This function checks the English spelling of word.

If the spelling is correct, the function will return a 1. If the spelling is incorrect, a LIST of suggestions for correct spellings will be returned instead. If the spelling is incorrect and no suggestions can be found, an empty LIST is returned.

### Function: `chr`

```
int chr(INT arg, ...)
```

This function translates integers into ASCII characters. Each argument must be an integer between 0 and 255.

If the programmer is not a wizard, and integers less than 32 are provided, E_INVARG is raised. This prevents control characters or newlines from being written to the database file by non-trusted individuals.

### Function: `match`

```
list match(str subject, str pattern [, int case-matters])
```

Searches for the first occurrence of the regular expression pattern in the string subject

If pattern is syntactically malformed, then `E_INVARG` is raised. The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

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

### Function: `rmatch`

```
list rmatch(str subject, str pattern [, int case-matters])
```

Searches for the last occurrence of the regular expression pattern in the string subject

If pattern is syntactically malformed, then `E_INVARG` is raised. The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

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

## Perl Compatible Regular Expressions

ToastStunt has two methods of operating on regular expressions. The classic style (outdated, more difficult to use, detailed in the next section) and the preferred Perl Compatible Regular Expression library. It is beyond the scope of this document to teach regular expressions, but an internet search should provide all the information you need to get started on what will surely become a lifelong journey of either love or frustration.

ToastCore offers two primary methods of interacting with regular expressions.

### Function: `pcre_match`

```
LIST pcre_match(STR subject, STR pattern [, ?case matters=0] [, ?repeat until no matches=1])
```

The function `pcre_match()` searches `subject` for `pattern` using the Perl Compatible Regular Expressions library.

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

### Function: `pcre_replace`

```
STR pcre_replace(STR `subject`, STR `pattern`)
```

The function `pcre_replace()` replaces `subject` with replacements found in `pattern` using the Perl Compatible Regular Expressions library.

The pattern string has a specific format that must be followed, which should be familiar if you have used the likes of Vim, Perl, or sed. The string is composed of four elements, each separated by a delimiter (typically a slash (/) or an exclamation mark (!)), that tell PCRE how to parse your replacement. We'll break the string down and mention relevant options below:

1. Type of search to perform. In MOO, only 's' is valid. This parameter is kept for the sake of consistency.

2. The text you want to search for a replacement.

3. The regular expression you want to use for your replacement text.

4. Optional modifiers:
   - Global. This will replace all occurrences in your string rather than stopping at the first.
   - Case-insensitive. Uppercase, lowercase, it doesn't matter. All will be replaced.

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

## Legacy MOO Regular Expressions

_Regular expression_ matching allows you to test whether a string fits into a specific syntactic shape. You can also search a string for a substring that fits a pattern.

A regular expression describes a set of strings. The simplest case is one that describes a particular string; for example, the string `foo` when regarded as a regular expression matches `foo` and nothing else. Nontrivial regular expressions use certain special constructs so that they can match more than one string. For example, the regular expression `foo%|bar` matches either the string `foo` or the string `bar`; the regular expression `c[ad]*r` matches any of the strings `cr`, `car`, `cdr`, `caar`, `cadddar` and all other such strings with any number of `a`'s and `d`'s.

Regular expressions have a syntax in which a few characters are special constructs and the rest are _ordinary_. An ordinary character is a simple regular expression that matches that character and nothing else. The special characters are `$`, `^`, `.`, `*`, `+`, `?`, `[`, `]` and `%`. Any other character appearing in a regular expression is ordinary, unless a `%` precedes it.

For example, `f` is not a special character, so it is ordinary, and therefore `f` is a regular expression that matches the string `f` and no other string. (It does _not_, for example, match the string `ff`.) Likewise, `o` is a regular expression that matches only `o`.

Any two regular expressions a and b can be concatenated. The result is a regular expression which matches a string if a matches some amount of the beginning of that string and b matches the rest of the string.

As a simple example, we can concatenate the regular expressions `f` and `o` to get the regular expression `fo`, which matches only the string `fo`. Still trivial.

The following are the characters and character sequences that have special meaning within regular expressions. Any character not mentioned here is not special; it stands for exactly itself for the purposes of searching and matching.

| Character Sequences    | Special Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                                                                                                               |                                                                                     |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| <code>.</code>         | is a special character that matches any single character. Using concatenation, we can make regular expressions like <code>a.b</code>, which matches any three-character string that begins with <code>a</code> and ends with <code>b</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |                                                                                                                                                                                                               |                                                                                     |
| <code>*</code>         | is not a construct by itself; it is a suffix that means that the preceding regular expression is to be repeated as many times as possible. In <code>fo*</code>, the <code> _</code> applies to the <code>o</code>, so <code>fo_</code> matches <code>f</code> followed by any number of <code>o</code>&apos;s. The case of zero <code>o</code>&apos;s is allowed: <code>fo*</code> does match <code>f</code>. <code> _</code> always applies to the <em>smallest</em> possible preceding expression. Thus, <code>fo_</code> has a repeating <code>o</code>, not a repeating <code>fo</code>. The matcher processes a <code> _</code> construct by matching, immediately, as many repetitions as can be found. Then it continues with the rest of the pattern. If that fails, it backtracks, discarding some of the matches of the <code>_</code>&apos;d construct in case that makes it possible to match the rest of the pattern. For example, matching <code>c[ad]_ar</code> against the string <code>caddaar</code>, the <code>[ad]_</code> first matches <code>addaa</code>, but this does not allow the next <code>a</code> in the pattern to match. So the last of the matches of <code>[ad]</code> is undone and the following <code>a</code> is tried again. Now it succeeds.                                                                                                     |                                                                                                                                                                                                               |                                                                                     |
| <code>+</code>         | <code>+</code> is like <code>*</code> except that at least one match for the preceding pattern is required for <code>+</code>. Thus, <code>c[ad]+r</code> does not match <code>cr</code> but does match anything else that <code>c[ad]*r</code> would match.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |                                                                                                                                                                                                               |                                                                                     |
| <code>?</code>         | <code>?</code> is like <code>*</code> except that it allows either zero or one match for the preceding pattern. Thus, <code>c[ad]?r</code> matches <code>cr</code> or <code>car</code> or <code>cdr</code>, and nothing else.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |                                                                                                                                                                                                               |                                                                                     |
| <code>[ ... ]</code>   | <code>[</code> begins a <em>character set</em>, which is terminated by a <code>]</code>. In the simplest case, the characters between the two brackets form the set. Thus, <code>[ad]</code> matches either <code>a</code> or <code>d</code>, and <code>[ad]*</code> matches any string of <code>a</code>&apos;s and <code>d</code>&apos;s (including the empty string), from which it follows that <code>c[ad]*r</code> matches <code>car</code>, etc.<br>Character ranges can also be included in a character set, by writing two characters with a <code>-</code> between them. Thus, <code>[a-z]</code> matches any lower-case letter. Ranges may be intermixed freely with individual characters, as in <code>[a-z$%.]</code>, which matches any lower case letter or <code>$</code>, <code>%</code> or period.<br> Note that the usual special characters are not special any more inside a character set. A completely different set of special characters exists inside character sets: <code>]</code>, <code>-</code> and <code>^</code>.<br> To include a <code>]</code> in a character set, you must make it the first character. For example, <code>[]a]</code> matches <code>]</code> or <code>a</code>. To include a <code>-</code>, you must use it in a context where it cannot possibly indicate a range: that is, as the first character, or immediately after a range. |                                                                                                                                                                                                               |                                                                                     |
| <code>[^...]</code>    | <code>[^</code> begins a <em>complement character set</em>, which matches any character except the ones specified. Thus, <code>[^a-z0-9A-Z]</code> matches all characters <em>except</em> letters and digits.<br><code>^</code> is not special in a character set unless it is the first character. The character following the <code>^</code> is treated as if it were first (it may be a <code>-</code> or a <code>]</code>).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                                                                                                               |                                                                                     |
| <code>^</code>         | is a special character that matches the empty string -- but only if at the beginning of the string being matched. Otherwise it fails to match anything. Thus, <code>^foo</code> matches a <code>foo</code> which occurs at the beginning of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                                                                                                               |                                                                                     |
| <code>$</code>         | is similar to <code>^</code> but matches only at the <em>end</em> of the string. Thus, <code>xx*$</code> matches a string of one or more <code>x</code>&apos;s at the end of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |                                                                                                                                                                                                               |                                                                                     |
| <code>%</code>         | has two functions: it quotes the above special characters (including <code>%</code>), and it introduces additional special constructs.<br> Because <code>%</code> quotes special characters, <code>%$</code> is a regular expression that matches only <code>$</code>, and <code>%[</code> is a regular expression that matches only <code>[</code>, and so on.<br> For the most part, <code>%</code> followed by any character matches only that character. However, there are several exceptions: characters that, when preceded by <code>%</code>, are special constructs. Such characters are always ordinary when encountered on their own.<br> No new special characters will ever be defined. All extensions to the regular expression syntax are made by defining new two-character constructs that begin with <code>%</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |                                                                                                                                                                                                               |                                                                                     |
| <code>%\|</code>       | specifies an alternative. Two regular expressions a and b with <code>%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | </code> in between form an expression that matches anything that either a or b will match.<br> Thus, <code>foo%                                                                                               | bar</code> matches either <code>foo</code> or <code>bar</code> but no other string. |
| <code>%                | </code> applies to the largest possible surrounding expressions. Only a surrounding <code>%( ... %)</code> grouping can limit the grouping power of <code>%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | </code>.<br> Full backtracking capability exists for when multiple <code>%                                                                                                                                    | </code>&apos;s are used.                                                            |
| <code>%( ... %)</code> | is a grouping construct that serves three purposes:<br> * To enclose a set of <code>%\|</code> alternatives for other operations. Thus, <code>%(foo%\|bar%)x</code> matches either <code>foox</code> or <code>barx</code>.<br> * To enclose a complicated expression for a following <code> _</code>, <code>+</code>, or <code>?</code> to operate on. Thus, <code>ba%(na%)_</code> matches <code>bananana</code>, etc., with any number of <code>na</code>&apos;s, including none.<br> * To mark a matched substring for future reference.<br> This last application is not a consequence of the idea of a parenthetical grouping; it is a separate feature that happens to be assigned as a second meaning to the same <code>%( ... %)</code> construct because there is no conflict in practice between the two meanings. Here is an explanation of this feature:                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |                                                                                                                                                                                                               |                                                                                     |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                                                                                                                                                                         | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |                                                                                     |
| <code>%digit</code>    | After the end of a <code>%( ... %)</code> construct, the matcher remembers the beginning and end of the text matched by that construct. Then, later on in the regular expression, you can use <code>%</code> followed by digit to mean &quot;match the same text matched by the digit&apos;th <code>%( ... %)</code> construct in the pattern.&quot; The <code>%( ... %)</code> constructs are numbered in the order that their <code>%(</code>&apos;s appear in the pattern.<br> The strings matching the first nine <code>%( ... %)</code> constructs appearing in a regular expression are assigned numbers 1 through 9 in order of their beginnings. <code>%1</code> through <code>%9</code> may be used to refer to the text matched by the corresponding <code>%( ... %)</code> construct.<br> For example, <code>%(._%)%1</code> matches any string that is composed of two identical halves. The <code>%(._%)</code> matches the first half, which may be anything, but the <code>%1</code> that follows must match the same exact text.                                                                                                                                                                                                                                                                                                                                          |                                                                                                                                                                                                               |                                                                                     |
| <code>%b</code>        | matches the empty string, but only if it is at the beginning or end of a word. Thus, <code>%bfoo%b</code> matches any occurrence of <code>foo</code> as a separate word. <code>%bball%(s%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | %)%b</code> matches <code>ball</code> or <code>balls</code> as a separate word.<br> For the purposes of this construct and the five that follow, a word is defined to be a sequence of letters and/or digits. |                                                                                     |
| <code>%B</code>        | matches the empty string, provided it is <em>not</em> at the beginning or end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |                                                                                                                                                                                                               |                                                                                     |
| <code>%&lt;</code>     | matches the empty string, but only if it is at the beginning of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                                                                                                               |                                                                                     |
| <code>%&gt;</code>     | matches the empty string, but only if it is at the end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |                                                                                                                                                                                                               |                                                                                     |
| <code>%w</code>        | matches any word-constituent character (i.e., any letter or digit).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |                                                                                                                                                                                                               |                                                                                     |
| <code>%W</code>        | matches any character that is not a word constituent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |                                                                                                                                                                                                               |                                                                                     |

### Function: `substitute`

```
str substitute(str template, list subs)
```

Performs a standard set of substitutions on the string template, using the information contained in subs, returning the resulting, transformed template.

Subs should be a list like those returned by `match()` or `rmatch()` when the match succeeds; otherwise, `E_INVARG` is raised.

In template, the strings `%1` through `%9` will be replaced by the text matched by the first through ninth parenthesized sub-patterns when `match()` or `rmatch()` was called. The string `%0` in template will be replaced by the text matched by the pattern as a whole when `match()` or `rmatch()` was called. The string `%%` will be replaced by a single `%` sign. If `%` appears in template followed by any other character, `E_INVARG` will be raised.

```
subs = match("*** Welcome to ToastStunt!!!", "%(%w*%) to %(%w*%)");
substitute("I thank you for your %1 here in %2.", subs)
        =>   "I thank you for your Welcome here in ToastStunt."
```

### Function: `salt`

```
str salt(str format, str input)
```

Generate a crypt() compatible salt string for the specified salt format using the specified binary random input.

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

### Function: `crypt`

```
str crypt(str text [, str salt])
```

Encrypts the given text using the standard UNIX encryption method.

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

### Function: `argon2`

```
str argon2(STR password, STR salt [, iterations = 3] [, memory usage in KB = 4096] [, CPU threads = 1])
```

The function `argon2()' hashes a password using the Argon2id password hashing algorithm. It is parametrized by three optional arguments:

- Time: This is the number of times the hash will get run. This defines the amount of computation required and, as a result, how long the function will take to complete.
- Memory: This is how much RAM is reserved for hashing.
- Parallelism: This is the number of CPU threads that will run in parallel.

The salt for the password should, at minimum, be 16 bytes for password hashing. It is recommended to use the random_bytes() function.

```
salt = random_bytes(20);
return argon2(password, salt, 3, 4096, 1);
```

> Warning: The MOO is single threaded in most cases, and this function can take significant time depending on how you call it. While it is working, nothing else is going to be happening on your MOO. It is possible to build the server with the `THREAD_ARGON2` option which will mitigate lag. This has major caveats however, see the section below on `argon2_verify` for more information.

### Function: `argon2_verify`

```
int argon2_verify(STR hash, STR password)
```

Compares password to the previously hashed hash.

Returns 1 if the two match or 0 if they don't.

This is a more secure way to hash passwords than the `crypt()` builtin.

> Note: ToastCore defines some sane defaults for how to utilize `argon2` and `argon2_verify`. You can `@grep argon2` from within ToastCore to find these.

> Warning: It is possible to build the server with the `THREAD_ARGON2` option. This will enable this built-in to run in a background thread and mitigate lag that these functions can cause. However, this comes with some major caveats. `do_login_command` (where you will typically be verifying passwords) cannot be suspended. Since threading implicitly suspends the MOO task, you won't be able to directly use Argon2 in do_login_command. Instead, you'll have to devise a new solution for logins that doesn't directly involve calling Argon2 in do_login_command.

> Note: More information on Argon2 can be found in the [Argon2 Github](https://github.com/P-H-C/phc-winner-argon2).

### Functions: `string_hash`, `binary_hash`

string_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

binary_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

```
str `string_hash`(str string, [, algo [, binary]])
str `binary_hash`(str bin-string, [, algo [, binary])
```

If algo is provided, it specifies the hashing algorithm to use. "MD5", "SHA1", "SHA224", "SHA256", "SHA384", "SHA512" and "RIPEMD160" are all supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

Note that the MD5 hash algorithm is broken from a cryptographic standpoint, as is SHA1. Both are included for interoperability with existing applications (both are still popular).

All supported hash functions have the property that, if

`string_hash(x) == string_hash(y)`

then, almost certainly,

`equal(x, y)`

This can be useful, for example, in certain networking applications: after sending a large piece of text across a connection, also send the result of applying string_hash() to the text; if the destination site also applies string_hash() to the text and gets the same result, you can be quite confident that the large text has arrived unchanged.

### Functions: `string_hmac`, `binary_hmac`

```
str string_hmac(str text, str key [, str algo [, binary]])
str binary_hmac(str bin-string, str key [, str algo [, binary]])
```

Returns a string encoding the result of applying the HMAC-SHA256 cryptographically secure HMAC function to the contents of the string text or the binary string bin-string with the specified secret key. If algo is provided, it specifies the hashing algorithm to use. Currently, only "SHA1" and "SHA256" are supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

All cryptographically secure HMACs have the property that, if

`string_hmac(x, a) == string_hmac(y, b)`

then, almost certainly,

`equal(x, y)`

and furthermore,

`equal(a, b)`

This can be useful, for example, in applications that need to verify both the integrity of the message (the text) and the authenticity of the sender (as demonstrated by the possession of the secret key).

## Operations on Lists

### Function: `length`

```
int length(list list)
```

Returns the number of elements in list.

It is also permissible to pass a string to `length()`; see the description in the previous section.

```
length({1, 2, 3})   =>   3
length({})          =>   0
```

### Function: `is_member`

```
int is_member(ANY value, LIST list [, INT case-sensitive])
```

Returns true if there is an element of list that is completely indistinguishable from value.

This is much the same operation as " `value in list`" except that, unlike `in`, the `is_member()` function does not treat upper- and lower-case characters in strings as equal. This treatment of strings can be controlled with the `case-sensitive` argument; setting `case-sensitive` to false will effectively disable this behavior.

Raises E_ARGS if two values are given or if more than three arguments are given. Raises E_TYPE if the second argument is not a list. Otherwise returns the index of `value` in `list`, or 0 if it's not in there.

```
is_member(3, {3, 10, 11})                  => 1
is_member("a", {"A", "B", "C"})            => 0
is_member("XyZ", {"XYZ", "xyz", "XyZ"})    => 3
is_member("def", {"ABC", "DEF", "GHI"}, 0) => 2
```

### Function: `all_members`

```
LIST all_members(ANY `value`, LIST `alist`)
```

Returns the indices of every instance of `value` in `alist`.

Example:

```
all_members("a", {"a", "b", "a", "c", "a", "d"}) => {1, 3, 5}
```

### Functions: `listinsert`, `listappend`

listinsert -- This functions return a copy of list with value added as a new element.

listappend -- This functions return a copy of list with value added as a new element.

```
list `listinsert`(list list, value [, int index])
list `listappend` (list list, value [, int index])
```

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

### Function: `listdelete`

```
list listdelete(list list, int index)
```

Returns a copy of list with the indexth element removed.

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listdelete(x, 2)   =>   {"foo", "baz"}
```

### Function: `listset`

```
list listset(list list, value, int index)
```

Returns a copy of list with the indexth element replaced by value.

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listset(x, "mumble", 2)   =>   {"foo", "mumble", "baz"}
```

This function exists primarily for historical reasons; it was used heavily before the server supported indexed assignments like `x[i] = v`. New code should always use indexed assignment instead of `listset()` wherever possible.

### Functions: `setadd`, `setremove`

setadd -- Returns a copy of list with the given value added.

setremove -- Returns a copy of list with the given value removed.

```
list setadd(list list, value)
list setremove(list list, value)
```

`setadd()` only adds value if it is not already an element of list; list is thus treated as a mathematical set. value is added at the end of the resulting list, if at all. Similarly, `setremove()` returns a list identical to list if value is not an element. If value appears more than once in list, only the first occurrence is removed in the returned copy.

```
setadd({1, 2, 3}, 3)         =>   {1, 2, 3}
setadd({1, 2, 3}, 4)         =>   {1, 2, 3, 4}
setremove({1, 2, 3}, 3)      =>   {1, 2}
setremove({1, 2, 3}, 4)      =>   {1, 2, 3}
setremove({1, 2, 3, 2}, 2)   =>   {1, 3, 2}
```

### Function: `reverse`

```
str | list reverse(LIST alist)
```

Return a reversed list or string

Examples:

```
reverse({1,2,3,4}) => {4,3,2,1}
reverse("asdf") => "fdsa"
```

### Function: `slice`

```
list slice(LIST alist [, INT | LIST | STR index, ANY default map value])
```

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

### Function: `sort`

```
list sort(LIST list [, LIST keys, INT natural sort order?, INT reverse])
```

Sorts list either by keys or using the list itself.

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

## Operations on Maps

When using the functions below, it's helpful to remember that maps are ordered.

### Function: `mapkeys`

```
list mapkeys(map map)
```

returns the keys of the elements of a map.

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapkeys(x)   =>  {"bar", "baz", "foo"}
```

### Function: `mapvalues`

```
list mapvalues(MAP `map` [, ... STR `key`])
```

returns the values of the elements of a map.

If you only want the values of specific keys in the map, you can specify them as optional arguments. See examples below.

Examples:

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapvalues(x)               =>  {2, 3, 1}
mapvalues(x, "foo", "baz") => {1, 3}
```

### Function: `mapdelete`

```
map mapdelete(map map, key)
```

Returns a copy of map with the value corresponding to key removed. If key is not a valid key, then E_RANGE is raised.

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapdelete(x, "bar")   ⇒   ["baz" -> 3, "foo" -> 1]
```

### Function: `maphaskey`

```
int maphaskey(MAP map, STR key)
```

Returns 1 if key exists in map. When not dealing with hundreds of keys, this function is faster (and easier to read) than something like: !(x in mapkeys(map))
