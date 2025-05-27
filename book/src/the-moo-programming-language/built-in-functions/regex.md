# Regular Expression Functions

## Perl Compatible Regular Expressions

mooR has two methods of operating on regular expressions. The classic style (outdated, more difficult to use, detailed
in the next section) and the preferred Perl Compatible Regular Expression library. It is beyond the scope of this
document to teach regular expressions, but an internet search should provide all the information you need to get started
on what will surely become a lifelong journey of either love or frustration.

MOO offers two primary methods of interacting with regular expressions.

### `pcre_match`

```
list pcre_match(str subject, str pattern [, int case_matters] [, int repeat_until_no_matches])
```

The function `pcre_match()` searches `subject` for `pattern` using the Perl Compatible Regular Expressions library.

The return value is a list of maps containing each match. Each returned map will have a key which corresponds to either
a named capture group or the number of the capture group being matched. The full match is always found in the key "0".
The value of each key will be another map containing the keys 'match' and 'position'. Match corresponds to the text that
was matched and position will return the indices of the substring within `subject`.

If `repeat_until_no_matches` is 1, the expression will continue to be evaluated until no further matches can be found or
it exhausts the iteration limit. This defaults to 1.

Additionally, wizards can control how many iterations of the loop are possible by adding a property
to $server_options. $server_options.pcre_match_max_iterations is the maximum number of loops allowed before giving up
and allowing other tasks to proceed. CAUTION: It's recommended to keep this value fairly low. The default value is 1000.
The minimum value is 100.

**Examples:**

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

### `pcre_replace`

```
str pcre_replace(str subject, str pattern)
```

The function `pcre_replace()` replaces `subject` with replacements found in `pattern` using the Perl Compatible Regular
Expressions library.

The pattern string has a specific format that must be followed, which should be familiar if you have used the likes of
Vim, Perl, or sed. The string is composed of four elements, each separated by a delimiter (typically a slash (/) or an
exclamation mark (!)), that tell PCRE how to parse your replacement. We'll break the string down and mention relevant
options below:

1. Type of search to perform. In MOO, only 's' is valid. This parameter is kept for the sake of consistency.

2. The text you want to search for a replacement.

3. The regular expression you want to use for your replacement text.

4. Optional modifiers:
    - Global. This will replace all occurrences in your string rather than stopping at the first.
    - Case-insensitive. Uppercase, lowercase, it doesn't matter. All will be replaced.

**Examples:**

Replace one word with another:

```
pcre_replace("I like banana pie. Do you like banana pie?", "s/banana/apple/g")

=> "I like apple pie. Do you like apple pie?"
```

If you find yourself wanting to replace a string that contains slashes, it can be useful to change your delimiter to an
exclamation mark:

```
pcre_replace("Unix, wow! /bin/bash is a thing.", "s!/bin/bash!/bin/fish!g")

=> "Unix, wow! /bin/fish is a thing."
```

## Legacy MOO Regular Expressions

_Regular expression_ matching allows you to test whether a string fits into a specific syntactic shape. You can also
search a string for a substring that fits a pattern.

A regular expression describes a set of strings. The simplest case is one that describes a particular string; for
example, the string `foo` when regarded as a regular expression matches `foo` and nothing else. Nontrivial regular
expressions use certain special constructs so that they can match more than one string. For example, the regular
expression `foo%|bar` matches either the string `foo` or the string `bar`; the regular expression `c[ad]*r` matches any
of the strings `cr`, `car`, `cdr`, `caar`, `cadddar` and all other such strings with any number of `a`'s and `d`'s.

Regular expressions have a syntax in which a few characters are special constructs and the rest are _ordinary_. An
ordinary character is a simple regular expression that matches that character and nothing else. The special characters
are `$`, `^`, `.`, `*`, `+`, `?`, `[`, `]` and `%`. Any other character appearing in a regular expression is ordinary,
unless a `%` precedes it.

For example, `f` is not a special character, so it is ordinary, and therefore `f` is a regular expression that matches
the string `f` and no other string. (It does _not_, for example, match the string `ff`.) Likewise, `o` is a regular
expression that matches only `o`.

Any two regular expressions a and b can be concatenated. The result is a regular expression which matches a string if a
matches some amount of the beginning of that string and b matches the rest of the string.

As a simple example, we can concatenate the regular expressions `f` and `o` to get the regular expression `fo`, which
matches only the string `fo`. Still trivial.

### Regular Expression Syntax

The following are the characters and character sequences that have special meaning within regular expressions. Any
character not mentioned here is not special; it stands for exactly itself for the purposes of searching and matching.

| Character Sequences | Special Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
|---------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `.`                 | is a special character that matches any single character. Using concatenation, we can make regular expressions like `a.b`, which matches any three-character string that begins with `a` and ends with `b`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `*`                 | is not a construct by itself; it is a suffix that means that the preceding regular expression is to be repeated as many times as possible. In `fo*`, the `*` applies to the `o`, so `fo*` matches `f` followed by any number of `o`'s. The case of zero `o`'s is allowed: `fo*` does match `f`. `*` always applies to the _smallest_ possible preceding expression. Thus, `fo*` has a repeating `o`, not a repeating `fo`. The matcher processes a `*` construct by matching, immediately, as many repetitions as can be found. Then it continues with the rest of the pattern. If that fails, it backtracks, discarding some of the matches of the `*`'d construct in case that makes it possible to match the rest of the pattern. For example, matching `c[ad]*ar` against the string `caddaar`, the `[ad]*` first matches `addaa`, but this does not allow the next `a` in the pattern to match. So the last of the matches of `[ad]` is undone and the following `a` is tried again. Now it succeeds.                                                                                                  |
| `+`                 | `+` is like `*` except that at least one match for the preceding pattern is required for `+`. Thus, `c[ad]+r` does not match `cr` but does match anything else that `c[ad]*r` would match.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `?`                 | `?` is like `*` except that it allows either zero or one match for the preceding pattern. Thus, `c[ad]?r` matches `cr` or `car` or `cdr`, and nothing else.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `[ ... ]`           | `[` begins a _character set_, which is terminated by a `]`. In the simplest case, the characters between the two brackets form the set. Thus, `[ad]` matches either `a` or `d`, and `[ad]*` matches any string of `a`'s and `d`'s (including the empty string), from which it follows that `c[ad]*r` matches `car`, etc.<br>Character ranges can also be included in a character set, by writing two characters with a `-` between them. Thus, `[a-z]` matches any lower-case letter. Ranges may be intermixed freely with individual characters, as in `[a-z$%.]`, which matches any lower case letter or `$`, `%` or period.<br> Note that the usual special characters are not special any more inside a character set. A completely different set of special characters exists inside character sets: `]`, `-` and `^`.<br> To include a `]` in a character set, you must make it the first character. For example, `[]a]` matches `]` or `a`. To include a `-`, you must use it in a context where it cannot possibly indicate a range: that is, as the first character, or immediately after a range. |
| `[^...]`            | `[^` begins a _complement character set_, which matches any character except the ones specified. Thus, `[^a-z0-9A-Z]` matches all characters _except_ letters and digits.<br>`^` is not special in a character set unless it is the first character. The character following the `^` is treated as if it were first (it may be a `-` or a `]`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `^`                 | is a special character that matches the empty string -- but only if at the beginning of the string being matched. Otherwise it fails to match anything. Thus, `^foo` matches a `foo` which occurs at the beginning of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `$`                 | is similar to `^` but matches only at the _end_ of the string. Thus, `xx*$` matches a string of one or more `x`'s at the end of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `%`                 | has two functions: it quotes the above special characters (including `%`), and it introduces additional special constructs.<br> Because `%` quotes special characters, `%$` is a regular expression that matches only `$`, and `%[` is a regular expression that matches only `[`, and so on.<br> For the most part, `%` followed by any character matches only that character. However, there are several exceptions: characters that, when preceded by `%`, are special constructs. Such characters are always ordinary when encountered on their own.<br> No new special characters will ever be defined. All extensions to the regular expression syntax are made by defining new two-character constructs that begin with `%`.                                                                                                                                                                                                                                                                                                                                                                         |
| `%\|`               | specifies an alternative. Two regular expressions a and b with `%\|` in between form an expression that matches anything that either a or b will match.<br> Thus, `foo%\|bar` matches either `foo` or `bar` but no other string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `%\|`               | applies to the largest possible surrounding expressions. Only a surrounding `%( ... %)` grouping can limit the grouping power of `%\|`.<br> Full backtracking capability exists for when multiple `%\|`'s are used.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `%( ... %)`         | is a grouping construct that serves three purposes:<br> * To enclose a set of `%\|` alternatives for other operations. Thus, `%(foo%\|bar%)x` matches either `foox` or `barx`.<br> * To enclose a complicated expression for a following `*`, `+`, or `?` to operate on. Thus, `ba%(na%)*` matches `bananana`, etc., with any number of `na`'s, including none.<br> * To mark a matched substring for future reference.<br> This last application is not a consequence of the idea of a parenthetical grouping; it is a separate feature that happens to be assigned as a second meaning to the same `%( ... %)` construct because there is no conflict in practice between the two meanings. Here is an explanation of this feature:                                                                                                                                                                                                                                                                                                                                                                       |
| `%digit`            | After the end of a `%( ... %)` construct, the matcher remembers the beginning and end of the text matched by that construct. Then, later on in the regular expression, you can use `%` followed by digit to mean "match the same text matched by the digit'th `%( ... %)` construct in the pattern." The `%( ... %)` constructs are numbered in the order that their `%(`'s appear in the pattern.<br> The strings matching the first nine `%( ... %)` constructs appearing in a regular expression are assigned numbers 1 through 9 in order of their beginnings. `%1` through `%9` may be used to refer to the text matched by the corresponding `%( ... %)` construct.<br> For example, `%(.*%)%1` matches any string that is composed of two identical halves. The `%(.*%)` matches the first half, which may be anything, but the `%1` that follows must match the same exact text.                                                                                                                                                                                                                    |
| `%b`                | matches the empty string, but only if it is at the beginning or end of a word. Thus, `%bfoo%b` matches any occurrence of `foo` as a separate word. `%bball%(s%\|%)%b` matches `ball` or `balls` as a separate word.<br> For the purposes of this construct and the five that follow, a word is defined to be a sequence of letters and/or digits.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `%B`                | matches the empty string, provided it is _not_ at the beginning or end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `%<`                | matches the empty string, but only if it is at the beginning of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `%>`                | matches the empty string, but only if it is at the end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `%w`                | matches any word-constituent character (i.e., any letter or digit).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `%W`                | matches any character that is not a word constituent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

### `match`

```
list match(str subject, str pattern [, int case_matters])
```

Searches for the first occurrence of the regular expression pattern in the string subject.

If pattern is syntactically malformed, then `E_INVARG` is raised. The process of matching can in some cases consume a
great deal of memory in the server; should this memory consumption become excessive, then the matching process is
aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about
the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and
true, then case is treated as significant in all comparisons.

The list that `match()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match,
replacements is a list described below, and subject is the same string that was given as the first argument to
`match()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end
indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the
indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If
there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then
the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on
parenthesized sub-patterns.

**Examples:**

```
match("foo", "^fo*$")        =>  {1, 3, {{0, -1}, ...}, "foo"}
match("foobar", "o*b")       =>  {2, 4, {{0, -1}, ...}, "foobar"}
match("foobar", "f%(o*%)b")
        =>  {1, 4, {{2, 3}, {0, -1}, ...}, "foobar"}
```

### `rmatch`

```
list rmatch(str subject, str pattern [, int case_matters])
```

Searches for the last occurrence of the regular expression pattern in the string subject.

If pattern is syntactically malformed, then `E_INVARG` is raised. The process of matching can in some cases consume a
great deal of memory in the server; should this memory consumption become excessive, then the matching process is
aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about
the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and
true, then case is treated as significant in all comparisons.

The list that `rmatch()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match,
replacements is a list described below, and subject is the same string that was given as the first argument to
`rmatch()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end
indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the
indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If
there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then
the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on
parenthesized sub-patterns.

**Examples:**

```
rmatch("foobar", "o*b")      =>  {4, 4, {{0, -1}, ...}, "foobar"}
```

### `substitute`

```
str substitute(str template, list subs)
```

Performs a standard set of substitutions on the string template, using the information contained in subs, returning the
resulting, transformed template.

Subs should be a list like those returned by `match()` or `rmatch()` when the match succeeds; otherwise, `E_INVARG` is
raised.

In template, the strings `%1` through `%9` will be replaced by the text matched by the first through ninth parenthesized
sub-patterns when `match()` or `rmatch()` was called. The string `%0` in template will be replaced by the text matched
by the pattern as a whole when `match()` or `rmatch()` was called. The string `%%` will be replaced by a single `%`
sign. If `%` appears in template followed by any other character, `E_INVARG` will be raised.

**Examples:**

```
subs = match("*** Welcome to LambdaMOO!!!", "%(%w*%) to %(%w*%)");
substitute("I thank you for your %1 here in %2.", subs)
        =>   "I thank you for your Welcome here in LambdaMOO."
```


