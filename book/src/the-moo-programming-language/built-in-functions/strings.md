## String Manipulation Functions

### `strsub`

Replaces all occurrences of `what` in `subject` with `with`, performing string substitution.

The occurrences are found from left to right and all substitutions happen simultaneously. By default, occurrences of
`what` are searched for while ignoring the upper/lower case distinction. If `case-matters` is provided and true, then
case
is treated as significant in all comparisons.

```
strsub("%n is a fink.", "%n", "Fred")   =>   "Fred is a fink."
strsub("foobar", "OB", "b")             =>   "fobar"
strsub("foobar", "OB", "b", 1)          =>   "foobar"
```

### `index`

Returns the index of the first character of the first occurrence of `str2` in `str1`.

```
int index(str str1, str str2 [, int case-matters [, int skip]])
```

These functions will return zero if `str2` does not occur in `str1` at all.

By default the search for an occurrence of `str2` is done while ignoring the upper/lower case distinction. If
`case-matters`
is provided and true, then case is treated as significant in all comparisons.

By default the search starts at the beginning of `str1`. If `skip` is provided, the search skips the first `skip`
characters and starts at an offset from the beginning of `str1`. The skip must be a positive integer for `index()`. The
default value of skip is 0 (skip no characters).

```
index("foobar", "o")            ⇒   2
index("foobar", "o", 0, 0)      ⇒   2
index("foobar", "o", 0, 2)      ⇒   1
index("foobar", "x")            ⇒   0
index("foobar", "oba")          ⇒   3
index("Foobar", "foo", 1)       ⇒   0
```

### `rindex`

Returns the index of the first character of the last occurrence of `str2` in `str1`.

```
int rindex(str str1, str str2 [, int case-matters [, int skip]])
```

By default the search starts at the end of `str1`. If `skip` is provided, the search skips the last `skip`
characters and starts at an offset from the end of `str1`. The skip must be a negative integer for `rindex()`. The
default value of skip is 0 (skip no characters).

```
rindex("foobar", "o")           ⇒   3
rindex("foobar", "o", 0, 0)     ⇒   3
rindex("foobar", "o", 0, -4)    ⇒   2
```

### `strcmp`

Performs a case-sensitive comparison of the two argument strings.

If `str1` is lexicographically less than `str2`, the `strcmp()` returns a negative integer. If the two strings are
identical, `strcmp()` returns zero. Otherwise, `strcmp()` returns a positive integer. The ASCII character ordering is
used for the comparison.

### `explode`

Returns a list of substrings of `subject` that are separated by `break`. `break` defaults to a space.

Only the first character of `break` is considered:

```
explode("slither%is%wiz", "%")      => {"slither", "is", "wiz"}
explode("slither%is%%wiz", "%%")    => {"slither", "is", "wiz"}
```

You can use `include-sequential-occurrences` to get back an empty string as part of your list if `break` appears
multiple
times with nothing between it, or there is a leading/trailing `break` in your string:

```
explode("slither%is%%wiz", "%%", 1)  => {"slither", "is", "", "wiz"}
explode("slither%is%%wiz%", "%", 1)  => {"slither", "is", "", "wiz", ""}
explode("%slither%is%%wiz%", "%", 1) => {"", "slither", "is", "", "wiz", ""}
```

> Note: This can be used as a replacement for `$string_utils:explode`.

### `strtr`

Transforms the string `source` by replacing the characters specified by `str1` with the corresponding characters
specified
by `str2`.

All other characters are not transformed. If `str2` has fewer characters than `str1` the unmatched characters are simply
removed from `source`. By default the transformation is done on both upper and lower case characters no matter the case.
If `case-matters` is provided and true, then case is treated as significant.

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

### `decode_binary`

Returns a list of strings and/or integers representing the bytes in the binary string `bin_string` in order.

If `fully` is false or omitted, the list contains an integer only for each non-printing, non-space byte; all other
characters are grouped into the longest possible contiguous substrings. If `fully` is provided and true, the list
contains
only integers, one for each byte represented in `bin_string`. Raises `E_INVARG` if `bin_string` is not a properly-formed
binary string.

```
decode_binary("foo")               =>   {"foo"}
decode_binary("~~foo")             =>   {"~foo"}
decode_binary("foo~0D~0A")         =>   {"foo", 13, 10}
decode_binary("foo~0Abar~0Abaz")   =>   {"foo", 10, "bar", 10, "baz"}
decode_binary("foo~0D~0A", 1)      =>   {102, 111, 111, 13, 10}
```

### `encode_binary`

Translates each integer and string in turn into its binary string equivalent, returning the concatenation of all these
substrings into a single binary string.

Each argument must be an integer between 0 and 255, a string, or a list containing only legal arguments for this
function.

```
encode_binary("~foo")                     =>   "~7Efoo"
encode_binary({"foo", 10}, {"bar", 13})   =>   "foo~0Abar~0D"
encode_binary("foo", 10, "bar", 13)       =>   "foo~0Abar~0D"
```

### `decode_base64`

`decode_base64(base64 [, safe])`

Returns the binary string representation of the supplied Base64 encoded string argument.

Raises E_INVARG if base64 is not a properly-formed Base64 string. If `safe` is provided and is true, a URL-safe version
of
Base64 is used (see RFC4648). The default is to use the URL-safe version.

```
decode_base64("AAEC")      ⇒    b"AAEC"
```

### `encode_base64`

`encode_base64(binary [, safe])`

Returns the Base64 encoded string representation of the supplied binary string argument.

If `safe` is provided and is true, a URL-safe version of Base64 is used (see RFC4648). The default is to use the
URL-safe version.

```
encode_base64(b"AAEC")      ⇒    "AAEC"
```

