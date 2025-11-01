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

### `encode_base64`

```
str encode_base64(str|binary data [, bool url_safe] [, bool no_padding])
```

Encodes the given string or binary data using Base64 encoding.

**Parameters:**
- `data`: String or binary data to encode
- `url_safe`: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.
- `no_padding`: If true, omits trailing = padding characters. Defaults to false.

**Returns:** Base64-encoded string

**Examples:**

```
encode_base64("hello world")          ⇒    "aGVsbG8gd29ybGQ="
encode_base64("hello world", 1)       ⇒    "aGVsbG8gd29ybGQ="
encode_base64("hello world", 1, 1)    ⇒    "aGVsbG8gd29ybGQ"
encode_base64(b"AAEC")                ⇒    "QUFFQ0=="
```

### `decode_base64`

```
binary decode_base64(str encoded_text [, bool url_safe])
```

Decodes Base64-encoded string to binary data.

**Parameters:**
- `encoded_text`: Base64-encoded string to decode
- `url_safe`: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.

**Returns:** Decoded binary data

Raises E_INVARG if the input is not a properly-formed Base64 string.

**Examples:**

```
decode_base64("aGVsbG8gd29ybGQ=")     ⇒    b"hello world"
decode_base64("aGVsbG8gd29ybGQ", 1)   ⇒    b"hello world"
decode_base64("QUFFQ0==")             ⇒    b"AAEC"
```

### `binary_to_str`

`str binary_to_str(binary data [, bool allow_lossy])`

Converts binary data to a string.

By default (`allow_lossy` is false or not provided), the binary data must be valid UTF-8. If the data contains invalid UTF-8 sequences, `E_INVARG` is raised.

If `allow_lossy` is true, invalid UTF-8 sequences are replaced with the Unicode replacement character (U+FFFD, displayed as '?'), allowing the conversion to succeed.

```
binary_to_str(b"Hello")                    ⇒    "Hello"
binary_to_str(b"\xF0\x9F\xA6\x80")         ⇒    "🦀"
binary_to_str(b"\xFF\xFE\xFD", 0)          ⇒    E_INVARG
binary_to_str(b"\xFF\xFE\xFD", 1)          ⇒    "???"
```

### `binary_from_str`

`binary binary_from_str(str text)`

Converts a string to binary data.

This function encodes the string as UTF-8 bytes. All valid strings can be converted.

```
binary_from_str("Hello")        ⇒    b"Hello"
binary_from_str("🦀")           ⇒    b"\xF0\x9F\xA6\x80"
binary_from_str("")             ⇒    b""
```

