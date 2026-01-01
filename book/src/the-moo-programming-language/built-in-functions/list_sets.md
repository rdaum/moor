## List Membership Functions

### `is_member`

Returns true if there is an element of list that is completely indistinguishable from value.

```
int is_member(ANY value, LIST list [, INT case-sensitive])
```

This is much the same operation as "`value in list`" except that, unlike `in`, the `is_member()` function does not
treat upper- and lower-case characters in strings as equal. This treatment of strings can be controlled with the
`case-sensitive` argument; setting `case-sensitive` to false will effectively disable this behavior.

Raises E_ARGS if two values are given or if more than three arguments are given. Raises E_TYPE if the second argument is
not a list. Otherwise returns the index of `value` in `list`, or 0 if it's not in there.

```
is_member(3, {3, 10, 11})                  => 1
is_member("a", {"A", "B", "C"})            => 0
is_member("XyZ", {"XYZ", "xyz", "XyZ"})    => 3
is_member("def", {"ABC", "DEF", "GHI"}, 0) => 2
```

### `all_members`

Returns the indices of every instance of `value` in `alist`.

```
LIST all_members(ANY value, LIST alist)
```

Example:

```
all_members("a", {"a", "b", "a", "c", "a", "d"}) => {1, 3, 5}
```

### `all`

Returns true if every argument is truthy. Returns true when no arguments are supplied.

```
bool all(any value1 [, any value2, ...])
```

Examples:

```
all(1, "ok", {1}) => true
all(1, 0)         => false
all()             => true
```

### `none`

Returns true if no argument is truthy. Returns true when no arguments are supplied.

```
bool none(any value1 [, any value2, ...])
```

Examples:

```
none(0, "", {false}) => true
none(0, 1)           => false
none()               => true
```

## List Modification Functions

### `listinsert`

Returns a copy of list with value added as a new element.

```
list listinsert(list list, value [, int index])
```

`listinsert()` adds value before the existing element with the given index, if provided.

If index is not provided, then `listinsert()` adds it at the beginning; this usage is discouraged, however, since the
same intent can be more clearly expressed using the list-construction expression, as shown in the examples below.

```
x = {1, 2, 3};
listinsert(x, 4, 2)   =>   {1, 4, 2, 3}
listinsert(x, 4)      =>   {4, 1, 2, 3}
{4, @x}               =>   {4, 1, 2, 3}
```

### `listappend`

Returns a copy of list with value added as a new element.

```
list listappend(list list, value [, int index])
```

`listappend()` adds value after the existing element with the given index, if provided.

The following three expressions always have the same value:

```
listinsert(list, element, index)
listappend(list, element, index - 1)
{@list[1..index - 1], element, @list[index..length(list)]}
```

If index is not provided, then `listappend()` adds the value at the end of the list.

```
x = {1, 2, 3};
listappend(x, 4, 2)   =>   {1, 2, 4, 3}
listappend(x, 4)      =>   {1, 2, 3, 4}
{@x, 4}               =>   {1, 2, 3, 4}
```

### `listdelete`

Returns a copy of list with the indexth element removed.

```
list listdelete(list list, int index)
```

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listdelete(x, 2)   =>   {"foo", "baz"}
```

### `listset`

Returns a copy of list with the indexth element replaced by value.

```
list listset(list list, value, int index)
```

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listset(x, "mumble", 2)   =>   {"foo", "mumble", "baz"}
```

This function exists primarily for historical reasons; it was used heavily before the server supported indexed
assignments like `x[i] = v`. New code should always use indexed assignment instead of `listset()` wherever possible.

## Set Operations

### `setadd`

Returns a copy of list with the given value added.

```
list setadd(list list, value)
```

`setadd()` only adds value if it is not already an element of list; list is thus treated as a mathematical set. value is
added at the end of the resulting list, if at all.

```
setadd({1, 2, 3}, 3)         =>   {1, 2, 3}
setadd({1, 2, 3}, 4)         =>   {1, 2, 3, 4}
```

### `setremove`

Returns a copy of list with the given value removed.

```
list setremove(list list, value)
```

`setremove()` returns a list identical to list if value is not an element. If value appears more than once in list, only
the first occurrence is removed in the returned copy.

```
setremove({1, 2, 3}, 3)      =>   {1, 2}
setremove({1, 2, 3}, 4)      =>   {1, 2, 3}
setremove({1, 2, 3, 2}, 2)   =>   {1, 3, 2}
```

### `reverse`

Return a reversed list or string.

```
str | list reverse(LIST alist)
```

Examples:

```
reverse({1,2,3,4}) => {4,3,2,1}
reverse("asdf") => "fdsa"
```

### `slice`

Return the index-th elements of alist. By default, index will be 1. If index is a list of integers, the returned list
will have those elements from alist. This is the built-in equivalent of LambdaCore's $list_utils:slice verb.

```
list slice(LIST alist [, INT | LIST | STR index, ANY default map value])
```

If alist is a list of maps, index can be a string indicating a key to return from each map in alist.

If default map value is specified, any maps not containing the key index will have default map value returned in their
place. This is useful in situations where you need to maintain consistency with a list index and can't have gaps in your
return list.

Examples:

```
slice({{"z", 1}, {"y", 2}, {"x",5}}, 2)                                 => {1, 2, 5}
slice({{"z", 1, 3}, {"y", 2, 4}}, {2, 1})                               => {{1, "z"}, {2, "y"}}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6]}, "a")                => {1, 5}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6], ["b" -> 8]}, "a", 0) => {1, 5, 0}
```

### `sort`

Sorts list either by keys or using the list itself.

```
list sort(LIST list [, LIST keys, INT natural sort order?, INT reverse])
```

When sorting list by itself, you can use an empty list ({}) for keys to specify additional optional arguments.

If natural sort order is true, strings containing multi-digit numbers will consider those numbers to be a single
character. So, for instance, this means that 'x2' would come before 'x11' when sorted naturally because 2 is less than
11. This argument defaults to 0.

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

## Additional List Functions

### `length`

Returns the number of elements in list.

```
int length(list list)
```

It is also permissible to pass a string to `length()`; see the description in the string functions section.

```
length({1, 2, 3})   =>   3
length({})          =>   0
```

### `complex_match`

Performs sophisticated string matching with ordinal support and object auto-detection.

```
str | obj complex_match(STR token, LIST targets [, LIST keys] [, NUM fuzzy_threshold])
```

The `complex_match()` function provides advanced pattern matching with support for ordinal selectors (e.g., "first", "
second", "1st", "2nd", "twenty-first") and four-tier matching precedence:

1. **Exact matches** - Complete string equality (case-insensitive)
2. **Prefix matches** - Strings that start with the search token
3. **Substring matches** - Strings that contain the search token anywhere
4. **Fuzzy matches** - Strings with small edit distances (typo tolerance)

#### Arguments

- `token` (STR): The search string to match against
- `targets` (LIST): List of strings or objects to search through
- `keys` (LIST, optional): List of key lists for object matching. Pass `false` to disable key-based matching.
- `fuzzy_threshold` (NUM, optional): Controls fuzzy matching sensitivity:
    - `0.0`: Disable fuzzy matching (exact/prefix/substring only)
    - `0.5`: Reasonable default (1-2 character differences allowed depending on word length)
    - `1.0`: Very permissive fuzzy matching (allows more character differences)
    - Boolean values supported for backward compatibility: `false` = `0.0`, `true` = `0.5`

#### String matching (2-argument form)

The two-argument form matches against a list of strings directly:

```
complex_match("foo", {"foobar", "food", "foot"})        => "foobar"
complex_match("second foo", {"foobar", "food", "foot"}) => "food"
complex_match("1st bar", {"foobar", "barfoo"})         => "foobar"
```

#### Object auto-detection (2-argument form with objects)

When the targets list contains objects, `complex_match` automatically extracts their names:

```
players = {#123, #456, #789, #890};  // Objects with names "Alice", "Bob", "Charlie", "Alice"
complex_match("alice", players)          => #123  // Case-insensitive name matching
complex_match("second alice", players)   => #890  // Ordinal selection by name

// Same functionality with explicit keys
complex_match("alice", {1, 2, 3, 4}, {"Alice", "Bob", "Charlie", "Alice"})        => 1
complex_match("second alice", {1, 2, 3, 4}, {"Alice", "Bob", "Charlie", "Alice"}) => 4
```

#### Object matching with keys (3-4 argument form)

The three/four-argument form matches against explicit object keys:

```
objs = {#123, #456, #789};
keys = {{"lamp", "light"}, {"bottle", "container"}, {"book", "tome"}};
complex_match("lamp", objs, keys)        => #123
complex_match("second b", objs, keys)    => #789  // matches "book"
complex_match("lamp", objs, keys, 0.0)   => #123  // fuzzy disabled
complex_match("lammp", objs, keys, 0.8)  => #123  // high fuzzy tolerance for typos
```

To disable key-based matching and force object auto-detection:

```
complex_match("alice", players, false)   => #123  // Use object names, not keys
```

#### Ordinal support

The function supports various ordinal formats:

- **Word ordinals**: "first", "second", "third", ..., "twentieth", "thirtieth", etc.
- **Numeric ordinals**: "1st", "2nd", "3rd", "4th", ..., "21st", "22nd", etc.
- **Dot notation with space**: "1. foo", "2. bar", "10. lamp", etc.
- **Dot notation without space**: "1.foo", "2.bar", "10.lamp", etc.
- **Compound ordinals**: "twenty-first", "thirty-second", etc.

Ordinals count across all match tiers combined. For example, with `{"foo", "foobar", "foobaz"}` and token "foo":
- Position 1: "foo" (exact match)
- Position 2: "foobar" (prefix match)
- Position 3: "foobaz" (prefix match)

So `complex_match("2.foo", {"foo", "foobar", "foobaz"})` returns "foobar".

#### Return values

- Returns the matched string/object for single matches
- Returns the first match when multiple matches exist at the same precedence level
- Returns `#-3` (FAILED_MATCH) when no matches are found
- For key-based matching, returns `#-2` (AMBIGUOUS) when multiple exact matches exist

#### Examples

```
// Basic string matching
complex_match("foo", {"foobar", "food"})             => "foobar"  // exact wins
complex_match("bar", {"foobar", "barbaz"})           => "foobar"  // first prefix match

// Ordinal selection (within same tier)
complex_match("2nd foo", {"foobar", "food", "foot"}) => "food"
complex_match("third lamp", {"lamp1", "lamp2", "lamp3"}) => "lamp3"

// Ordinal selection (across tiers)
complex_match("2.foo", {"foo", "foobar", "foobaz"})     => "foobar"  // 2nd overall (1 exact + 1st prefix)
complex_match("second foo", {"foo", "foobar", "foobaz"}) => "foobar"
complex_match("2nd foo", {"foo", "foobar", "foobaz"})   => "foobar"
complex_match("3.foo", {"foo", "foobar", "foobaz"})     => "foobaz"  // 3rd overall
complex_match("2.foo", {"foo", "bar", "baz"})           => #-3       // only 1 match exists

// Four-tier precedence
complex_match("test", {"testing", "test", "contest"}) => "test"  // exact beats prefix/substring

// Object auto-detection
players = players();  // Returns list of player objects
complex_match("alice", players)                       => #123  // Finds player named Alice
complex_match("archwizard", players)                  => #2    // Case-insensitive matching

// Disable keys, force object auto-detection
complex_match("alice", players, false)                => #123  // Use object names
complex_match("alice", players, false, 0.0)           => #123  // No fuzzy matching
complex_match("alise", players, false, 0.3)           => #123  // Low fuzzy tolerance for typos

// No matches
complex_match("xyz", {"abc", "def"})                  => #-3
```

This function is particularly useful for implementing sophisticated object matching in MOO commands.

### `complex_matches`

Returns all matches from the best (highest priority) non-empty tier as a list.

```
list complex_matches(STR token, LIST targets [, NUM fuzzy_threshold])
```

Unlike `complex_match()` which returns a single match (or uses ordinals to select one), `complex_matches()` returns all matching strings from the highest-priority non-empty tier.

#### Arguments

- `token` (STR): The search string to match against (ordinals are ignored)
- `targets` (LIST): List of strings to search through
- `fuzzy_threshold` (NUM, optional): Controls fuzzy matching sensitivity (default: 0.5)

#### Return values

- Returns a list of all matches from the best non-empty tier
- Returns an empty list `{}` when no matches are found
- Tier priority: exact > prefix > substring > fuzzy

#### Examples

```
// Returns all exact matches (best tier)
complex_matches("foo", {"foo", "bar", "foo"})       => {"foo", "foo"}

// Returns all prefix matches when no exact match
complex_matches("foo", {"foobar", "food", "bar"})   => {"foobar", "food"}

// Returns all substring matches when no exact/prefix
complex_matches("oo", {"foobar", "boo", "bar"})     => {"foobar", "boo"}

// Ordinals in token are stripped (returns all matches, not Nth)
complex_matches("2nd foo", {"foo", "foobar"})       => {"foo"}

// No matches returns empty list
complex_matches("xyz", {"abc", "def"})              => {}

// Fuzzy matching with typos
complex_matches("lammp", {"lamp", "table"}, 0.5)    => {"lamp"}
complex_matches("lammp", {"lamp", "table"}, 0.0)    => {}  // fuzzy disabled
```
