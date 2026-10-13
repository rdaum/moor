# Message template reference

This reference describes Cowbell's current message templates. For a worked example, start with
[Write messages that fit the reader](pronouns-and-messages.md).

## Names and roles

| Token | Event role      | Result                                            |
| ----- | --------------- | ------------------------------------------------- |
| `{n}` | actor           | Actor's name, or “you” for the actor.             |
| `{d}` | direct object   | Direct object's name, or “you” for that object.   |
| `{i}` | indirect object | Indirect object's name, or “you” for that object. |
| `{t}` | hosting object  | Name of the object supplied through `with_this`.  |
| `{l}` | location        | Name of the captured event location.              |

Each name token substitutes “you” if its object is the viewer. Add `c` for initial capitalization:
`{nc}`, `{dc}`, `{ic}`, `{tc}`, or `{lc}`. Capitalization uses Unicode case conversion on the first
character and leaves the rest of the name unchanged.

The long bindings `{actor}`, `{dobj}`, `{iobj}`, `{this_obj}`, and `{location}` also identify these
roles. `{this}` is an alias for `{this_obj}`.

`{t}` is an explicit event role. It is not necessarily MOO's current `this` or the direct object. A
missing standard participant produces a placeholder such as `<no-dobj>`.

## Pronouns

| Actor token | Direct object token | Indirect object token | Meaning                |
| ----------- | ------------------- | --------------------- | ---------------------- |
| `{s}`       | `{s_dobj}`          | `{s_iobj}`            | Subject: they, she.    |
| `{o}`       | `{o_dobj}`          | `{o_iobj}`            | Object: them, her.     |
| `{p}`       | `{p_dobj}`          | `{p_iobj}`            | Before noun: their.    |
| `{q}`       | `{q_dobj}`          | `{q_iobj}`            | Without noun: theirs.  |
| `{r}`       | `{r_dobj}`          | `{r_iobj}`            | Reflexive: themselves. |

Add `c` before a role suffix: `{sc_dobj}`, `{pc_iobj}`. When the referenced object is the viewer,
the forms are `you`, `you`, `your`, `yours`, and `yourself`.

There are no corresponding pronoun tokens for `this_obj`, `location`, or arbitrary named bindings.
There is no general first-person “I/me” mode.

## Verbs and word choices

| Actor token | Other roles                  | Pronoun agreement                                                           |
| ----------- | ---------------------------- | --------------------------------------------------------------------------- |
| `{be}`      | `{be_dobj}`, `{be_iobj}`     | `are` for the viewer, otherwise `verb_be`.                                  |
| `{have}`    | `{have_dobj}`, `{have_iobj}` | `have` for the viewer, otherwise `verb_have`.                               |
| `{look}`    | `{look_dobj}`, `{look_iobj}` | `look` for the viewer or a plural-agreement pronoun set, otherwise `looks`. |

These shortcuts use the pronoun set, not the object's noun number. Pair them with `{s}` or its role
variants. Use explicit agreement when the sentence names its subject or needs another verb:

```text
{nc} {agree actor name:are|is} ready.
{sc} {agree actor pronoun:wave|waves}.
{The d} {agree dobj name:are|is} here.
```

The syntax is `{agree BINDING FORM:PLURAL|SINGULAR}`:

- `BINDING` identifies the subject: `actor`, `dobj`, `iobj`, `this`, `location`, or a named object
  binding.
- `FORM` is `name` or `pronoun`, matching how the sentence refers to its subject.
- `PLURAL` and `SINGULAR` are literal words or phrases. They cannot contain nested substitutions.

Name agreement reads the object's `is_plural()` result. Pronoun agreement reads
`pronouns.is_plural`. The subject's own view always uses the plural form, as in “you are.” Thus
singular Alex using `they/them` gives “Alex is” or “They are,” depending on the chosen subject.
Missing object roles produce placeholders. Text bindings cannot supply grammar.

`{left|right}` selects `left` for the event actor and `right` for every other viewer. It selects
perspective without checking grammar. Branches preserve whitespace and can contain other tokens:

```text
{You see {d}|{nc} sees {d}}
```

Only the selected branch expands. Empty branches insert nothing. Further alternatives can be nested.
The explicit spelling is `{self:left|right}`. The decompiler uses this spelling to preserve literal
branches that start with the reserved `agree` or `self:` prefix.

`{cap CONTENT}` capitalizes the first resolved character. For example, `{cap {direction}}`
capitalizes a named text binding. `{cap {you|they}}` capitalizes the selected alternative.

## Articles

`{a d}` or `{an d}` inserts an indefinite article and the direct object's name. `{the d}` inserts
the definite article and the name. Both return “you” for the object itself and omit articles for
proper names. Indefinite articles are also omitted for plural and uncountable nouns.

These forms accept other object roles and named object bindings, such as `{the i}` or `{a gift}`.
They do not accept text bindings. For a text binding `direction`, use `{direction}` with any
required surrounding words: `to the {direction}`.

`{A d}` and `{The d}` capitalize the whole phrase. `{the dc}` capitalizes only the object's name.
Only `nc`, `dc`, `ic`, `tc`, and `lc` have the article-specific capitalization suffix. Names such as
`relic` remain intact.

The object supplies `is_proper_noun()`, `is_plural()`, and `is_countable()`. On things, these read
`is_proper_noun_name`, `is_plural_noun`, and `is_countable_noun`.

Automatic `a`/`an` selection uses spelling and common pronunciation exceptions, such as “hour” and
“user.” Set a thing's `indefinite_article` to `"a"` or `"an"` to override it. An empty string uses
the automatic choice. This override covers acronyms and dialect-dependent words such as “herb.” It
does not force an article onto a proper name, plural noun, or uncountable noun.

The MOO constructors are `$sub:a('d)`, `$sub:ac('d)`, `$sub:the('d)`, and `$sub:thec('d)`. They
insert the name with its article, exactly like the template tokens.

## Named bindings and literal text

`{direction}` reads a binding called `direction`. The event producer must supply it. Unknown token
names compile as bindings, so a typo can survive compilation and fail during delivery.

There are three binding kinds:

| Kind    | Event method                        | How to insert it                                 |
| ------- | ----------------------------------- | ------------------------------------------------ |
| Text    | `with_text_binding(name, string)`   | `{name}` or `$sub:binding(name)`                 |
| Object  | `with_object_binding(name, object)` | `{name}` or `$sub:binding(name)`                 |
| Content | `with_content_binding(name, parts)` | `{content name}` or `$sub:content_binding(name)` |

Binding names are MOO symbols. Standard role names and their aliases are reserved. Token names such
as `s` and `be` take precedence over custom binding names. Use `{bind s}` to read a binding
literally named `s`. Template identifiers start with an ASCII letter or underscore. Later characters
can also be digits or hyphens.

Text values stay literal. A value containing `{n}`, HTML, or Djot does not become another template
or markup. Object bindings become object references with viewer-specific labels. Content bindings
explicitly insert structured inline content through `{content NAME}`.

Backslash quotes literal delimiters: `\{`, `\}`, `\|`, and `\\`. A pipe outside braces can also
appear literally. Other backslashes remain unchanged. Literal strings supplied directly to events do
not need template escaping.

Compilation rejects empty tokens, unclosed braces, and stray closing braces. Its limits are 65536
characters, 2048 tokens, 16 brace levels, and 8192 text or delimiter pieces. Resolution has separate
document limits and can reject content that expands beyond them.

## Which roles does a message supply?

The producer determines the roles. The following table describes current callers:

| Message producer              | Actor            | Direct object    | Indirect object | Hosting object |
| ----------------------------- | ---------------- | ---------------- | --------------- | -------------- |
| Thing pickup/drop in the room | Person acting    | Item             | Person acting   | Unset          |
| Furniture sit/stand           | Person acting    | Unset            | Unset           | Furniture      |
| Reaction `announce`           | Triggering actor | Reacting object  | Unset           | Unset          |
| Reaction `emote`              | Reacting object  | Triggering actor | Unset           | Unset          |
| Reaction `tell`               | Triggering actor | Unset            | Reacting object | Unset          |

A reaction's private-message recipient is separate from the event's indirect object. Reaction
`emote` also prefixes the reacting object's name before the template.

Passage movement templates supply `actor`, `location`, and `this_obj`. They also supply object
bindings `from_room` and `to_room`, plus text bindings `direction`, `from_direction`,
`to_direction`, and `passage`. The arrival message uses the destination-side direction and location.

Rule bindings, reaction state, and reaction inputs do not automatically become message bindings.

## MOO programming interface

Compile authored template text before making the event:

```moo
const parts = $sub_utils:compile("{nc} {pick|picks} up {the d}.");
const event = $event:mk_info(player, @parts):with_dobj(item);
player.location:announce(event);
```

Here, `item` is an existing object supplied by your command. The `@` expands the list into the event
constructor's content arguments. The constructor does not compile strings automatically.

The equivalent programmatic content is:

```moo
const event = $event:mk_info(player,
  $sub:nc(), " ", $sub:self_alt("pick", "picks"), " up ", $sub:the('d), ".")
  :with_dobj(item);
```

Programmatic `self_alt` branches can contain lists of text and substitutions, or structured inline
content:

```moo
const content = $sub:self_alt(
  {"You notice ", $sub:d()},
  {$sub:nc(), " notices ", $sub:d()});
const event = $event:mk_info(player, content):with_dobj(item);
```

Only the selected branch expands. `self_altc` capitalizes the first resolved text in that branch.

To inspect the result without sending it:

```moo
const document = event:resolve(viewer);
const text = $document:render(document, 'text_plain)['body];
```

`viewer` is an existing object. Resolution uses the caller's authority, not the viewer's
permissions. Names and pronouns are read during resolution. An already resolved document contains
fixed text and references. Renderers do not rerun substitutions.

`$sub_utils:decompile(parts)` preserves strings, `$sub` values, and nested lists for editing. It
escapes literal punctuation and preserves capitalization. It rejects other content instead of
converting it to text. Use a content binding for formatted nodes or computed content.

The general agreement constructor is:

```moo
const verb = $sub:agree('actor, 'name, "wave", "waves");
```

Its arguments match the template's binding, subject form, plural wording, and singular wording.
`$sub:capitalize(content)` is the programmatic equivalent of `{cap CONTENT}`.

## Pronoun sets for programmers

The presets are `$pronouns.he_him`, `.she_her`, `.they_them`, `.it_its`, and `.spivak`. Each is a
flyweight: an immutable value containing a delegate and named fields.

| Field       | Meaning                  | they/them preset |
| ----------- | ------------------------ | ---------------- |
| `ps`        | Subject                  | `they`           |
| `po`        | Object                   | `them`           |
| `pp`        | Possessive before noun   | `their`          |
| `pq`        | Possessive without noun  | `theirs`         |
| `pr`        | Reflexive                | `themselves`     |
| `verb_be`   | Third-person “be” form   | `are`            |
| `verb_have` | Third-person “have” form | `have`           |
| `is_plural` | Plural verb agreement    | `true`           |
| `display`   | Selector label           | `they/them`      |

`is_plural` here describes agreement. It does not mean that the person is more than one individual.
An object's noun number is separate: `$thing.is_plural_noun` controls its `is_plural()` result.

Create a custom set with:

```moo
const pronouns = $pronouns:mk("ze", "zir", "zir", "zirs", "zirself", false);
```

The last argument selects plural verb agreement and defaults to `false`. Supply nonempty strings and
a boolean. Construction validates these requirements. Stored sets are also checked when read.

Assign the result to an actor or thing's `pronouns` property using the property owner's permissions.
`player:set_pronouns(string)` accepts preset labels only and checks owner, wizard, or delegated
`set_pronouns` authority. `@pronouns` changes the current player's preset.

`object:pronouns()` returns the validated set. `actor:pronouns_display()` returns its display label.
Actors and things share these convenience methods:

```moo
object:pronoun_subject();
object:pronoun_object();
object:pronoun_possessive('adj);
object:pronoun_possessive('noun);
object:pronoun_reflexive();
```

For `she/her`, the two possessive calls return `her` and `hers`. Each takes exactly one selector.
