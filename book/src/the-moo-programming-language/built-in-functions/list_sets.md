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

## List Modification Functions

### `listinsert`

Returns a copy of list with value added as a new element.

```
list listinsert(list list, value [, int index])
```

`listinsert()` adds value before the existing element with the given index, if provided.

If index is not provided, then `listinsert()` adds it at the beginning; this usage is discouraged, however, since the same intent can be more clearly expressed using the list-construction expression, as shown in the examples below.

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

`setremove()` returns a list identical to list if value is not an element. If value appears more than once in list, only the first occurrence is removed in the returned copy.

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
character. So, for instance, this means that 'x2' would come before 'x11' when sorted naturally because 2 is less than 11. This argument defaults to 0.

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

Performs sophisticated string matching with ordinal support and three-tier matching precedence.

```
str | obj complex_match(STR token, LIST strings)
obj complex_match(STR token, LIST objects, LIST keys)
```

The `complex_match()` function provides advanced pattern matching with support for ordinal selectors (e.g., "first", "second", "1st", "2nd", "twenty-first") and three-tier matching precedence:

1. **Exact matches** - Complete string equality (case-insensitive)
2. **Prefix matches** - Strings that start with the search token
3. **Substring matches** - Strings that contain the search token anywhere

#### Two-argument form

The two-argument form matches against a list of strings directly:

```
complex_match("foo", {"foobar", "food", "foot"})        => "foobar"
complex_match("second foo", {"foobar", "food", "foot"}) => "food"
complex_match("1st bar", {"foobar", "barfoo"})         => "foobar"
```

#### Three-argument form

The three-argument form matches against object keys and returns the corresponding objects:

```
objs = {#123, #456, #789};
keys = {{"lamp", "light"}, {"bottle", "container"}, {"book", "tome"}};
complex_match("lamp", objs, keys)       => #123
complex_match("second b", objs, keys)   => #789  // matches "book"
```

#### Ordinal support

The function supports various ordinal formats:

- **Word ordinals**: "first", "second", "third", ..., "twentieth", "thirtieth", etc.
- **Numeric ordinals**: "1st", "2nd", "3rd", "4th", ..., "21st", "22nd", etc.
- **Dot notation**: "1.", "2.", "10.", etc.
- **Compound ordinals**: "twenty-first", "thirty-second", etc.

#### Return values

- Returns the matched string/object for single matches
- Returns the first match when multiple matches exist at the same precedence level
- Returns `#-1` (FAILED_MATCH) when no matches are found
- For the three-argument form, returns `#-2` (AMBIGUOUS) when multiple exact matches exist

#### Examples

```
// Basic matching
complex_match("foo", {"foobar", "food"})           => "foobar"  // exact wins
complex_match("bar", {"foobar", "barbaz"})         => "foobar"  // first prefix match

// Ordinal selection
complex_match("2nd foo", {"foobar", "food", "foot"}) => "food"
complex_match("third lamp", {"lamp1", "lamp2", "lamp3"}) => "lamp3"

// Three-tier precedence
complex_match("test", {"testing", "test", "contest"}) => "test"  // exact beats prefix/substring

// No matches
complex_match("xyz", {"abc", "def"})               => #-1
```

This function is particularly useful for implementing sophisticated object matching in MOO commands and user interfaces.
