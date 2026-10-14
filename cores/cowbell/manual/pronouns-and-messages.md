# Write messages that fit the reader

When Alex picks up a pebble, Alex sees “You pick up the pebble.” Other people see “Alex picks up the
pebble.” You can write one message that produces both versions.

Cowbell calls this a **message template**. A template contains ordinary text and tokens in braces,
such as `{nc}`. Each token supplies a name, a pronoun, or a choice of words.

The [template reference](message-template-reference.md) lists the tokens and explains the
programming interface. In the world, use `help pronouns` to choose your pronouns or `help messages`
for a short authoring tutorial.

## Choose your own pronouns

Enter this command to see your current pronouns and the available presets:

```text
@pronouns
```

To choose a preset, include its name:

```text
@pronouns they/them
```

The supplied presets are `he/him`, `she/her`, `they/them`, `it/its`, and `e/em`. Meadow also
provides a pronoun selector in the account settings.

These choices affect messages that use pronoun tokens. They do not rewrite ordinary speech or
replace words that an author wrote directly into a message.

To enter your own words, use `@pronouns custom`. You do not need builder or programmer privileges.
The prompts ask you to fill five blanks:

| Sentence          | Example answer |
| ----------------- | -------------- |
| ___ is ready.     | ze             |
| I met ___.        | zir            |
| This is ___ coat. | zir            |
| The coat is ___.  | zirs           |
| Ze did it ___.    | zirself        |

Then choose the wording that fits: “ze is ready” or “ze are ready.” This also selects “has” or
“have.” The command shows complete sentences before asking you to apply the choice. Answer `no`, or
enter `@abort` at a word prompt, to keep your existing pronouns.

In Meadow, choose **Enter custom pronouns** in account settings or profile setup. Fill the blanks,
choose the verb form, then select **Preview sentences**. If the sentences look right, select **Use
these pronouns**. In profile setup, finish the profile to save the choice. If you change a word,
preview the sentences again before applying it.

## Choose pronouns for an NPC

If you have builder privileges, you can set pronouns for an actor you own. An actor is an object
that represents a character, such as Henri the cat. Programmer privileges are not required.

```text
@set-pronouns Henri to he/him
```

Use the actor's name, alias, or object identifier. The command confirms which actor changed and
shows the new pronouns. It also works on actor prototypes, so their children can inherit a choice.

For custom words, use:

```text
@set-pronouns Henri to custom
```

This uses the same five blanks, verb agreement, and preview as your personal pronoun editor. Enter
`@abort` at a word prompt, or answer `no` at confirmation, to keep the current pronouns. Permission
is checked again when you save.

`@pronouns` always refers to your own pronouns. For the builder command's reference, type
`help @set-pronouns`.

## Write a message for your own command

Give the message a name ending in `_msg`:

```text
@set-message "pet rock".pet_msg {nc} {pet|pets} {d}.
```

The command creates the message if it is missing. You must own the object to add one. For an
ordinary object message, `{nc}` is the person acting and `{d}` is the object. You do not need to
write a `message_bindings` verb for these roles.

A programmer can send this message from a command with:

```moo
player.location:announce(this:message_event(player, "pet_msg"));
```

`message_event` supplies the person and object, while `announce` chooses who receives it. Use
`this:message_event(player, "pet_denied_msg", 'error)` to prepare an error message. Send it to one
person with `player:inform_current(...)`.

Custom participants, such as a named recipient or destination, need a declaration and a supplied
value. Those are advanced extensions. The [reference](message-template-reference.md) describes them.

## Give an object a different pickup message

You need builder privileges for this example. The pebble belongs to you, so you can change its
messages. Enter each command separately.

Create the pebble:

```text
@create $thing named pebble
```

The new pebble starts in your inventory. Preview a pickup message:

```text
@preview-message pebble.get_msg {nc} {pick|picks} up {the d}.
```

The preview shows example readers named Alex, Sam, and Robin. It saves nothing and sends no pickup
event. Alex sees “You pick up the pebble.” The other readers see “Alex picks up the pebble.”

To save this message, use:

```text
@set-message pebble.get_msg {nc} {pick|picks} up {the d}.
```

The command stores the template. It does not send the pickup message yet.

Drop the pebble, then pick it up:

```text
drop pebble
get pebble
```

For a player named Alex, the pickup produces these messages:

| Reader | Message                   |
| ------ | ------------------------- |
| Alex   | You pick up the pebble.   |
| Sam    | Alex picks up the pebble. |

Here is what each piece means:

| Piece            | Meaning                                                          |
| ---------------- | ---------------------------------------------------------------- |
| `pebble.get_msg` | The pebble's pickup message.                                     |
| `{nc}`           | The person doing the action, or “You” when that person reads it. |
| `{pick\|picks}`  | “pick” for that person, “picks” for everyone else.               |
| `{the d}`        | “the” and the name of the item being picked up.                  |

The rest of the text appears as written. The token `{the d}` already includes the object's name.
Adding another `{d}` after it repeats the name.

To inspect the stored message, use:

```text
@get-message pebble.get_msg
```

To change it again, repeat `@set-message` with the new text.

To find out which participants this message can name, use:

```text
@message-bindings pebble.get_msg
```

For this message, `actor` means the person moving the pebble, and `dobj` means the pebble. These
meanings depend on the interaction. The editor checks them before saving. For example,
`Hello {dobbj}.` reports a mistake at character 7 and suggests `{dobj}`.

## Refer to people without guessing their pronouns

The **actor** is the person or object doing the action described by a message. In the pickup
example, Alex is the actor and the pebble is the **direct object**.

These tokens refer to the actor:

| Token | Example use       | Alex reads | Sam reads, if Alex uses they/them |
| ----- | ----------------- | ---------- | --------------------------------- |
| `{s}` | subject           | you        | they                              |
| `{o}` | object            | you        | them                              |
| `{p}` | before a noun     | your       | their                             |
| `{q}` | without a noun    | yours      | theirs                            |
| `{r}` | referring to self | yourself   | themselves                        |

For example, `{p} pebble` becomes “your pebble” for Alex and “their pebble” for Sam. The message
`{sc} {have} a pebble.` becomes “You have a pebble” or “They have a pebble.” With `she/her`, the
observer sees “She has a pebble.”

The `c` in `{sc}` capitalizes the first character. The same option exists for the other pronouns:
`{oc}`, `{pc}`, `{qc}`, and `{rc}`.

Possessive words describe the sentence. They do not check who owns the object in the database.

## Match the verb to the subject

“Alex has a pebble” and “They have a pebble” describe the same person. Their subjects need different
verb forms. This distinction matters when you write a template.

When the sentence uses a name, tell the template to match that name:

```text
{nc} {agree actor name:have|has} a pebble.
```

`actor` selects the person doing the action. `name` says that the sentence uses their name. The two
words after the colon are the plural and singular forms, in that order. Alex reads “You have a
pebble.” Sam reads “Alex has a pebble.”

When the sentence uses a pronoun, select `pronoun` instead:

```text
{sc} {agree actor pronoun:have|has} a pebble.
```

With Alex's `they/them` pronouns, Sam reads “They have a pebble.” With `she/her`, Sam reads “She has
a pebble.” Alex still reads “You have a pebble.”

There are shortcuts for three common verbs. `{be}`, `{have}`, and `{look}` follow pronoun agreement.
For example, `{sc} {have} a pebble.` gives the same result as the second template. Use those
shortcuts after pronouns. Use name agreement after names.

The earlier `{pick|picks}` example makes a simpler choice: “pick” for the actor, “picks” for other
readers. That works for one named person. It does not check whether a subject is plural. Use `agree`
when the message could describe a plural name or needs pronoun agreement.

## Write a different sentence for each reader

A choice can contain names and other tokens:

```text
{You notice {the d}|{nc} notices {the d}}
```

For Alex holding a pebble, this becomes “You notice the pebble” for Alex and “Alex notices the
pebble” for Sam. Only the chosen side is expanded. Spaces on each side remain as written.

Use `{cap CONTENT}` to capitalize the start of a phrase, including names with accented initials. For
example, `{cap {direction}}` turns a supplied “north” into “North.” The interaction must supply that
`direction` binding.

To print braces as ordinary text, put a backslash before them: `\{` and `\}`. Use `\|` for a literal
pipe inside a choice, and `\\` for a literal backslash.

## Refer to another participant

A message can also identify a direct object, an **indirect object**, and the object hosting an
interaction. For a gift, these can be the gift, the recipient, and the object running the command.

The command or reaction supplies these roles. Tokens read the supplied roles. They do not search the
room or infer a recipient from the surrounding words.

For example, a give command can supply Alex as actor and Sam as indirect object. This template:

```text
{nc} {give|gives} {p} pebble to {i}.
```

produces:

| Reader | Message                         |
| ------ | ------------------------------- |
| Alex   | You give your pebble to Sam.    |
| Sam    | Alex gives their pebble to you. |

`{i}` becomes “you” when the indirect object reads the message. Actor pronouns still refer to Alex.
To use Sam's pronouns, use tokens with `_iobj`, such as `{p_iobj}`.

Some interactions supply a participant by name, such as `recipient`. For these messages,
`{pronoun recipient pos_adj} coat` means “their coat” to others and “your coat” to the recipient.
The name must come from the interaction; writing `recipient` does not select someone automatically.

The [reference](message-template-reference.md#which-roles-does-a-message-supply) lists the roles
supplied by common interactions and reaction effects.

## Use messages in reactions

An `announce` step can use the same templates. For example, a welcome reaction on a box can include:

```text
@reaction cedarbox welcome then announce "{nc} {open|opens} {the d}."
```

For this effect, `{nc}` refers to the person who triggered the reaction. `{d}` refers to the box.
This example assumes that you already created the reaction.

Before adding the step, you can preview its wording:

```text
@reaction cedarbox welcome preview-message announce "{nc} {open|opens} {the d}."
```

This command does not add or run a step. A normal reaction `preview` also shows the example
sentences for its saved message steps. A `tell actor` preview uses the same example person as actor
and recipient. Only the recipient gets the actual private message.

The [rules and reactions guide](reactions.md#make-a-box-announce-a-message) explains how to create
and enable it. A normal open command already announces opening the box, so choose a different
message when you want an additional response.

## When the output is wrong

| Symptom                        | What to check                                                      |
| ------------------------------ | ------------------------------------------------------------------ |
| `Alex have` or `Alex are`      | Select `name` agreement for a name.                                |
| `They waves`                   | Select `pronoun` agreement for a subject pronoun.                  |
| `<no-dobj>` or `<no-iobj>`     | The interaction did not supply that participant.                   |
| `Missing event binding`        | The template names a binding that the interaction does not supply. |
| An object's name appears twice | `{a d}` and `{the d}` already contain the name.                    |
| A brace error while saving     | Close every token and escape literal braces with a backslash.      |

For an item named “water,” set its `is_countable_noun` property to `false` so it omits “a.” For a
name with an unusual pronunciation, its `indefinite_article` property can specify “a” or “an.” These
properties require permission to edit the object. A programmer can help configure them.

Message templates and Djot descriptions are different authoring formats. A template does not
automatically interpret asterisks as emphasis. A room description does not automatically interpret
`{nc}` as a message token.
