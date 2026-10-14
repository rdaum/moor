# Message template reference

This reference describes Cowbell's current message templates. For a worked example, start with
[Write messages that fit the reader](pronouns-and-messages.md). The in-world `help messages` topic
has a shorter tutorial. Use `help pronouns` for personal pronoun settings.

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

`{pronoun BINDING FORM}` selects a pronoun for any object binding, including `this`, `location`, or
a named participant such as `recipient`. The producer must supply the participant.

| Form        | Example                                    | Result for someone using they/them |
| ----------- | ------------------------------------------ | ---------------------------------- |
| `subject`   | `{pronoun recipient subject}`              | they                               |
| `object`    | `{pronoun recipient object}`               | them                               |
| `pos_adj`   | `{pronoun recipient pos_adj} coat`         | their coat                         |
| `pos_noun`  | `the coat is {pronoun recipient pos_noun}` | the coat is theirs                 |
| `reflexive` | `{pronoun recipient reflexive}`            | themselves                         |

Use `{cap {pronoun recipient subject}}` at the start of a sentence. Pair it with
`{agree recipient pronoun:are|is}` for “They are” or “You are.” The MOO constructor is
`$sub:pronoun('recipient, 'subject)`. Text and content bindings cannot supply pronouns. There is no
general first-person “I/me” mode.

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

Name agreement reads the object's `noun_grammar(viewer)['number]` result. Pronoun agreement reads
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

`{the d}` asks the direct object for its definite phrase, such as “the box.” `{a d}` and `{an d}`
ask for its indefinite phrase: “a box,” “an apple,” “some boots,” or “some water.” `{listing d}`
uses the object's preferred form for room and contents lists. Complete names such as Henri and Ada's
mailbox acquire no article. For the object itself, prose references become “you.”

These operations accept object roles and named object bindings, such as `{the i}` or `{a gift}`.
They reject text bindings. A direction is text, so `{direction}` inserts its authored label
unchanged.

`{A d}`, `{The d}`, and `{Listing d}` capitalize the whole phrase. `{the dc}` capitalizes the name
after its article. Only the short forms `nc`, `dc`, `ic`, `tc`, and `lc` have that suffix; a binding
called `relic` remains intact.

Builders use `@grammar` to configure ordinary objects. See the
[naming tutorial](../docs/names-and-grammar.md). The spelling helper selects “an hour” and “a user.”
`article an` or `article a` overrides the choice for a singular count noun. `article auto` restores
automatic selection. It cannot force an article onto a complete name, mass noun, or plural noun.

The constructors are `$sub:a('d)`, `$sub:ac('d)`, `$sub:the('d)`, `$sub:thec('d)`,
`$sub:listing('d)`, and `$sub:listingc('d)`. `$sub:reference(object, form)` refers directly to an
object when building a list. It resolves for the actual reader and preserves the object link. Its
default form is `'definite`. `$sub:attribution(object)` uses a third-person definite phrase for an
emote prefix, including for the actor. This preserves authored continuations such as “waves,” which
would not agree with “You.” It asks for the phrase with a neutral viewer and still resolves when the
document does.

### Object naming verbs

All objects inherit the basic interface from `$root`. Settings configure these verbs; callers use
the verbs rather than reading settings themselves.

| Verb                        | Result                                                                                                                                         |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `noun_grammar(viewer)`      | `['kind -> 'count, 'number -> 'singular, 'article -> ""]`, for example. Kind is count, mass, or name; number is singular or plural.            |
| `noun_phrase(form, viewer)` | A string for label, bare, definite, indefinite, or listing. Label preserves the literal name even for the viewer.                              |
| `listing_form(viewer)`      | `'indefinite` or `'definite`. World prototypes expose the inherited `listing_form` preference. Utilities need no such property.                |
| `match_names(context)`      | A list of accepted names. The default returns the main name and aliases. Explicit scope keys still replace this list.                          |
| `set_grammar(changes)`      | Validates fields kind, number, article, and listing. The value `'inherit` clears a local setting. Requires owner, wizard, or bearer authority. |

For example, a box prototype can override one phrase for its keeper:

```moo
const {form, viewer} = args;
if (form == 'definite && viewer == this.keeper)
  return "the box you left here";
endif
return pass(@args);
```

Listing calls the object's selected phrase through ordinary verb dispatch, so this override also
works with definite listing. A grammar override can call `pass(@args)` and change the returned map.
If the phrase describes a plural subject, report plural number too.

These queries must not mutate or suspend. The resolver captures answers locally for one output,
shared across bindings to the same object. Formatting that document does not call the object again.
A later resolution can see a rename. No utility keeps a naming registry or persistent cache. The
viewer controls perspective, not permissions. Malformed return values cause an authoring error.
Message previews query the actual host, including its overrides.

Matching consumes at most one leading the, a, an, or some before the normal matcher runs. Exact
authored names win first. Articles do not widen scope, expose omitted aliases, or select one of
several matching objects. `the second box` uses the existing ordinal rule; an object literally named
`the second box` takes precedence.

## Default participants for object messages

An ordinary custom message supplies `actor`, `location`, and `dobj`. `dobj` refers to the object
that owns the message. `$root:message_event(actor, property[, kind])` creates that event. The
optional kind is `info` or `error`. The caller chooses where to deliver it.

Inherited operations and reaction steps keep their own participant meanings. For example, an emote
step uses its reacting object as actor. Use `@message-bindings OBJECT.PROPERTY` to inspect an object
message, or `@reaction ... preview-message` to preview a reaction message.

Override `message_bindings(property)` only when a custom producer supplies different or additional
values. The declaration supports validation and previews; it does not supply runtime values. The
producer must put those values on the event. Call `pass(@args)` for other message properties.

## Named bindings and literal text

`{direction}` reads a binding called `direction`. The event producer must supply it. `@set-message`,
`@add-message`, message tools, and reaction message steps check the producer's declarations before
saving. Errors identify the token's character position and suggest similar names. The check covers
both branches of an alternative, even when a preview only displays one branch.

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

Use `@message-bindings OBJECT.PROPERTY` to see each binding's meaning for that message. The producer
determines the roles. The following table describes current callers:

| Message producer              | Actor            | Direct object    | Indirect object | Hosting object |
| ----------------------------- | ---------------- | ---------------- | --------------- | -------------- |
| Thing pickup/drop in the room | Person acting    | Item             | Person acting   | Unset          |
| Furniture sit/stand           | Person acting    | Unset            | Unset           | Furniture      |
| Reaction `announce`           | Triggering actor | Reacting object  | Unset           | Unset          |
| Reaction `emote`              | Reacting object  | Triggering actor | Unset           | Unset          |
| Reaction `tell`               | Triggering actor | Unset            | Reacting object | Unset          |

A reaction's private-message recipient is separate from the event's indirect object. Reaction
`emote` also prefixes the reacting object's name before the template. Reaction `tell` supplies a
named `recipient` binding for the actual recipient.

Passage movement templates supply `actor`, `location`, and `this_obj`. They also supply object
bindings `from_room` and `to_room`, plus text bindings `direction`, `from_direction`,
`to_direction`, and `passage`. The arrival message uses the destination-side direction and location.

Rule bindings, reaction state, and reaction inputs do not automatically become message bindings.

### Preview a draft

`@preview-message OBJECT.PROPERTY TEMPLATE` checks a draft and shows three example views without
saving it. The examples use Alex with they/them, Sam with she/her, and Robin with he/him. Object
examples preserve the host's name, pronouns, and noun properties. Other object values come from the
producer's declared examples. These are examples, not a prediction of the next event.

For reactions, use `@reaction OBJECT NAME preview-message announce "TEMPLATE"`. Replace `announce`
with `emote` or `tell actor` as needed. This leaves the saved steps unchanged.
`@reaction OBJECT NAME preview` also shows wording for saved message steps, including scheduled
messages. It does not run those steps or start their timers.

Previews use the document resolver. Reactions share the same event constructor between preview and
delivery. A private message preview includes other views for comparison; delivery goes only to its
recipient. Programmed content that cannot be expressed as a template is reported as unavailable for
preview.

### Declare bindings for a new message

Programmers declare a message's bindings on the prototype that produces its event. Override
`message_bindings(property)` and call `pass(@args)` for other properties. For example, a new
`welcome_msg` with a named recipient can declare:

```moo
const {property} = args;
if (property == "welcome_msg")
  let bindings = $sub_utils:standard_bindings("The person giving the welcome.");
  bindings['recipient] = ['kind -> 'object, 'description -> "The person being welcomed.",
    'example -> 'recipient];
  return bindings;
endif
return pass(@args);
```

Each declaration gives the binding's `kind`, a plain-language `description`, and an `example` for
previews. Object examples select `actor`, `recipient`, `host`, `item`, `location`, `from_room`, or
`to_room`. Text and content examples hold literal values. The declaration supplies editing guidance;
the event producer still supplies actual values with methods such as `with_object_binding`.

Unknown message properties have no declared bindings. Authors can save literal text in them. Declare
the participants before authors use substitutions in a new message property. No declaration grants
permission to edit a property or read another object's contents.

For direct MOO compilation, pass the declarations as the second argument:
`$sub_utils:compile(text, object:message_bindings(property))`. The one-argument form checks syntax
only, for callers that supply their own event context.

## MOO programming interface

Compile authored template text before making the event:

```moo
const parts = $sub_utils:compile("{The actor} {agree actor name pick|picks} up {the d}.");
const event = $event:mk_info(player, @parts):with_dobj(item);
player.location:announce(event);
```

Here, `item` is an existing object supplied by your command. The `@` expands the list into the event
constructor's content arguments. The constructor does not compile strings automatically.

The equivalent programmatic content is:

```moo
const event = $event:mk_info(player,
  $sub:thec('actor), " ", $sub:agree('actor, 'name, "pick", "picks"), " up ", $sub:the('d), ".")
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
An object's noun number is separate: `noun_grammar(viewer)['number]` reports whether its name is
singular or plural.

Create a custom set with:

```moo
const pronouns = $pronouns:mk("ze", "zir", "zir", "zirs", "zirself", false);
```

The last argument selects plural verb agreement and defaults to `false`. Supply nonempty strings and
a boolean. Construction validates these requirements. Stored sets are also checked when read.

Assign the result to an actor or thing's `pronouns` property using the property owner's permissions.
`player:set_pronouns(input)` accepts a preset label or a map of custom fields. Both commands and
Meadow call this setter. It checks owner, wizard, or delegated `set_pronouns` authority.

```moo
const input = ["subject" -> "ze", "object" -> "zir", "possessive_adjective" -> "zir",
  "possessive_pronoun" -> "zirs", "reflexive" -> "zirself", "agreement" -> "is"];
const sentences = $pronouns:examples(input);
player:set_pronouns(input);
```

Custom input requires all six fields and rejects unknown fields. Each word must occupy one line,
with 1–64 characters after trimming. `agreement` is `"is"` or `"are"`. `$pronouns:from_input(input)`
validates and constructs the set; `examples(input)` returns the sentences without saving.
`player:pronouns_settings()` returns the current fields for editing. `@pronouns custom` collects
these fields, previews the sentences, and rechecks permission on save.

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

## Upgrade an existing world

This is a core API change. Fresh cores use the new settings directly. An existing world must
preserve its old settings before the update removes their properties.

The conversion helpers are in `$obj_utils`, in `src/utils/obj_utils.moo`. They require wizard
authority. Install `capture_legacy_naming` and `apply_legacy_naming` from that file before applying
the rest of the core update. The capture helper uses the old properties; it does not need the new
naming API.

Capture the objects you will upgrade and retain the returned map. For a small world, a wizard can
make a checkpoint object:

```moo
const checkpoint = create($root, player);
const snapshot = $obj_utils:capture_legacy_naming({$root, @descendants($root)});
add_property(checkpoint, "snapshot", snapshot, {player, ""});
return checkpoint;
```

Keep the returned object identifier. Read its `snapshot['review]` list before proceeding. For a
larger world, capture batches of objects and retain every batch; do not increase task limits to scan
the whole database. The helpers store no checkpoint on a utility singleton.

The records preserve local noun settings and pronunciation overrides. Children that inherited old
settings continue to inherit the corresponding new ones. Complete-name flags take precedence over
countability. Mailbox records select complete-name grammar. Letter records preserve the old name as
their subject.

Review names beginning with articles; the conversion does not strip articles or lowercase names.
Convert custom `is_plural`, `is_countable`, `is_proper_noun`, and article-bearing `display_name`
overrides into the new object verbs. A record with custom verbs refuses application until you review
it and clear its `custom` list. Previews and room lists must call the same naming verbs; do not keep
a separate `display_name` article rule.

Apply the core update, including removal of the obsolete flags and methods. Then pass the saved
`records` list to `$obj_utils:apply_legacy_naming(records)`. It checks the batch before writing and
refuses targets whose owner, parent, or name changed. It sets each letter's subject before changing
its physical name to `letter`. Reapplying the same batch is supported. Retain the checkpoint until
you verify the results.

Remove any site-defined copies of the three obsolete flags after converting their descendants. Keep
only the new writable settings. Check custom templates for bare references and singular verbs: use
`{the d}` for an identified object and `agree` for its subject.
