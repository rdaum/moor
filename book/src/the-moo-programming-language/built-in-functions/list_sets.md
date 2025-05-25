## List Membership Functions

### `is_member`

**Description:** Checks if a value is an element of a list.
**Arguments:**

- : The value to search for `value`
- : The list to search in `list`

**Returns:** A boolean value (1 if the value is in the list, 0 otherwise)
**Example:**

```
is_member("apple", {"apple", "banana", "orange"}) => 1
is_member(5, {1, 2, 3}) => 0
```

## List Modification Functions

### `listinsert`

**Description:** Inserts a value at a specific position in a list.
**Arguments:**

- : The list to modify `list`
- : The value to insert `value`
- : The position where the value should be inserted (1-based index) `position`

**Returns:** The modified list
**Notes:**

- If is less than or equal to 1, the value is inserted at the beginning `position`
- If is greater than the length of the list, the value is appended to the end `position`

**Example:**

```
listinsert({1, 2, 3}, 99, 2) => {1, 99, 2, 3}
```

### `listappend`

**Description:** Appends a value to the end of a list.
**Arguments:**

- : The list to modify `list`
- : The value to append `value`

**Returns:** The modified list
**Example:**

```
listappend({1, 2, 3}, 4) => {1, 2, 3, 4}
```

### `listdelete`

**Description:** Removes an element at a specific position from a list.
**Arguments:**

- : The list to modify `list`
- : The position of the element to remove (1-based index) `position`

**Returns:** The modified list
**Notes:**

- If is outside the valid range (1 to length of list), E_RANGE is raised `position`

**Example:**

```
listdelete({1, 2, 3, 4}, 3) => {1, 2, 4}
```

### `listset`

**Description:** Replaces the value at a specific position in a list.
**Arguments:**

- : The list to modify `list`
- : The new value `value`
- : The position to replace (1-based index) `position`

**Returns:** The modified list
**Notes:**

- If is outside the valid range (1 to length of list), E_RANGE is raised `position`

**Example:**

```
listset({1, 2, 3}, 99, 2) => {1, 99, 3}
```

## Set Operations

### `setadd`

**Description:** Adds a value to a list if it's not already present, treating the list as a set.
**Arguments:**

- : The list to modify `list`
- : The value to add `value`

**Returns:** The modified list
**Example:**

```
setadd({1, 2, 3}, 4) => {1, 2, 3, 4}
setadd({1, 2, 3}, 2) => {1, 2, 3}  // No change as 2 is already in the list
```

### `setremove`

**Description:** Removes all occurrences of a value from a list.
**Arguments:**

- : The list to modify `list`
- : The value to remove `value`

**Returns:** The modified list
**Example:**

```
setremove({1, 2, 3, 2}, 2) => {1, 3}
```

## Regular Expression Functions

### `match`

**Description:** Searches for a pattern in a string using MOO-style regular expressions.
**Arguments:**

- : The regular expression pattern `pattern`
- : The string to search in `subject`
- : Optional boolean (default: 0) indicating if case sensitivity should be used `case-matters`

**Returns:** If a match is found, a list containing:

1. The starting position of the match (1-based)
2. The length of the match
3. For each capturing group: a list containing its starting position and length

If no match is found, returns 0.
**Example:**

```
match("a(.*)c", "abcdef") => {1, 3, {2, 1}}  // Matches "abc", with group capturing "b"
```

### `rmatch`

**Description:** Similar to `match`, but searches from the end of the string.
**Arguments:**

- : The regular expression pattern `pattern`
- : The string to search in `subject`
- : Optional boolean (default: 0) indicating if case sensitivity should be used `case-matters`

**Returns:** Same as `match`, but finds the rightmost occurrence of the pattern.

### `pcre_match`

**Description:** Searches for a pattern in a string using PCRE-compatible regular expressions.
**Arguments:**

- : The regular expression pattern `pattern`
- : The string to search in `subject`
- : Optional boolean (default: 0) indicating if case sensitivity should be used `case-matters`
- : Optional boolean (default: 0) indicating if all matches should be found `repeat`

**Returns:**

- If is 0: returns a list with the first match and all capture groups `repeat`
- If is 1: returns a list of all matches, with each match as a sublist containing the match and its capture groups
  `repeat`

**Example:**

```
pcre_match("a(.)c", "abc adc") => {"abc", "b"}
pcre_match("a(.)c", "abc adc", 0, 1) => {{"abc", "b"}, {"adc", "d"}}
```

### `pcre_replace`

**Description:** Replaces text in a string using PCRE regular expressions.
**Arguments:**

- : The regular expression pattern `pattern`
- : The replacement string `replacement`
- : The string to modify `subject`
- : Optional boolean (default: 0) indicating if case sensitivity should be used `case-matters`

**Returns:** The modified string with replacements applied
**Example:**

```
pcre_replace("a(.)c", "A$1C", "abc adc") => "AbC AdC"
```

### `substitute`

**Description:** Substitutes captures from a regular expression match into a template string.
**Arguments:**

- : The template string with placeholders like %1, %2, etc. `template`
- : The match result from a previous call to `match` or `matches`rmatch``
- : The original string that was matched against `subject`

**Returns:** The template with placeholders replaced by the captured text
**Example:**

```
substitute("The %2 is %1.", match("(\\w+) (\\w+)", "red apple"), "red apple") => "The apple is red."
```

## List Manipulation Functions

### `slice`

**Description:** Extracts a portion of a list.
**Arguments:**

- : The list to extract from `list`
- : The starting position (1-based index) `from`
- `to`: The ending position (1-based index)

**Returns:** A new list containing the elements from position to position `to` (inclusive) `from`
**Notes:**

- If is negative, it counts from the end of the list `from`
- If `to` is negative, it counts from the end of the list
- If `to` is greater than the length of the list, it is treated as the list length

**Example:**

```
slice({1, 2, 3, 4, 5}, 2, 4) => {2, 3, 4}
slice({1, 2, 3, 4, 5}, 2, -2) => {2, 3, 4}
```

## Regular Expression Syntax

### MOO-Style Regular Expressions

The `match` and functions use a simplified regular expression syntax: `rmatch`

- `.` - Matches any single character
- `*` - Matches zero or more of the preceding character or group
- `+` - Matches one or more of the preceding character or group
- `?` - Matches zero or one of the preceding character or group
- `[abc]` - Matches any character in the brackets
- `[^abc]` - Matches any character not in the brackets
- `()` - Creates a capturing group
- `|` - Alternation (OR)
- `^` - Matches the start of a string
- `$` - Matches the end of a string

### PCRE Regular Expressions

The and functions use the more powerful PCRE syntax, which includes: `pcre_match`pcre_replace``

- All MOO-style features
- `\d` - Matches any digit
-
  - Matches any word character (letter, digit, underscore) `\w`
- `\s` - Matches any whitespace character
- `{n}` - Matches exactly n of the preceding character or group
- `{n,m}` - Matches between n and m of the preceding character or group
- `(?:...)` - Non-capturing group
- Look-ahead and look-behind assertions
- And many more advanced features

## Replacement String Syntax

In , the replacement string can include: `pcre_replace`

- `$n` or - Refers to the nth capture group `\n`
- `$0` - Refers to the entire match
- `\\` - Represents a literal backslash
- `\$` - Represents a literal dollar sign

In , the template string can include: `substitute`

- `%n` - Refers to the nth capture group
- `%%` - Represents a literal percent sign
