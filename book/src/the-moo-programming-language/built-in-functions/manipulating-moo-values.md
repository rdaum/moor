# Manipulating MOO Values

There are several functions for performing primitive operations on MOO values, and they can be cleanly split into two
kinds: those that do various very general operations that apply to all types of values, and those that are specific to
one particular type. There are so many operations concerned with objects that we do not list them in this section but
rather give them their own section following this one.

## General Operations Applicable to All Values

### `typeof`

```
int typeof(value)
```

Takes any MOO value and returns an integer representing the type of value.

The result is the same as the initial value of one of these built-in variables: `INT`, `FLOAT`, `STR`, `LIST`, `OBJ`, or
`ERR`, `BOOL`, `MAP`, `WAIF`, `ANON`. Thus, one usually writes code like this:

```
if (typeof(x) == LIST) ...
```

and not like this:

```
if (typeof(x) == 3) ...
```

because the former is much more readable than the latter.

### `tostr`

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

Warning `tostr()` does not do a good job of converting lists and maps into strings; all lists, including the empty list,
are converted into the string `"{list}"` and all maps are converted into the string `"[map]"`. The function
`toliteral()`, below, is better for this purpose.

### `toliteral`

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

### `toint`

```
int toint(value)
```

Converts the given MOO value into an integer and returns that integer.

Floating-point numbers are rounded toward zero, truncating their fractional parts. Object numbers are converted into the
equivalent integers. Strings are parsed as the decimal encoding of a real number which is then converted to an integer.
Errors are converted into integers obeying the same ordering (with respect to `<=` as the errors themselves. `toint()`
raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number,
then `toint()` returns 0.

```
toint(34.7)        =>   34
toint(-34.7)       =>   -34
toint(#34)         =>   34
toint("34")        =>   34
toint("34.7")      =>   34
toint(" - 34  ")   =>   -34
toint(E_TYPE)      =>   1
```

### `toobj`

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

### `tofloat`

```
float tofloat(value)
```

Converts the given MOO value into a floating-point number and returns that number.

Integers and object numbers are converted into the corresponding integral floating-point numbers. Strings are parsed as
the decimal encoding of a real number which is then represented as closely as possible as a floating-point number.
Errors are first converted to integers as in `toint()` and then converted as integers are. `tofloat()` raises `E_TYPE`
if value is a list. If value is a string but the string does not contain a syntactically-correct number, then
`tofloat()` returns 0.

```
tofloat(34)          =>   34.0
tofloat(#34)         =>   34.0
tofloat("34")        =>   34.0
tofloat("34.7")      =>   34.7
tofloat(E_TYPE)      =>   1.0
```

### `equal`

```
int equal(value, value2)
```

Returns true if value1 is completely indistinguishable from value2.

This is much the same operation as `value1 == value2` except that, unlike `==`, the `equal()` function does not treat
upper- and lower-case characters in strings as equal and thus, is case-sensitive.

```
"Foo" == "foo"         =>   1
equal("Foo", "foo")    =>   0
equal("Foo", "Foo")    =>   1
```

### `value_bytes`

```
int value_bytes(value)
```

Returns the number of bytes of the server's memory required to store the given value.

### `value_hash`

```
str value_hash(value, [, str algo] [, binary])
```

Returns the same string as `string_hash(toliteral(value))`.

See the description of `string_hash()` for details.

### `value_hmac`

```
str value_hmac(value, STR key [, STR algo [, binary]])
```

Returns the same string as string_hmac(toliteral(value), key)

See the description of string_hmac() for details.










