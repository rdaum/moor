# Give objects names that fit a sentence

Make a pet rock. Give it the name `pet rock` and the shorter alias `rock`:

<details>
<summary>Test setup</summary>

```moot
@wizard
; const room = create($room, #90100); room.name = "Naming Workshop"; room.description = "A quiet room for trying object names."; move(#90102, room); #90102.is_builder = true; #90102.name = "Ada"; #90102.authoring_features = $builder_features; return true;
true
; add_property(#90102, "naming_output", "", {#90102, ""}); add_verb(#90102, {#90102, "rxd", "inform_current"}, {"this", "none", "this"}); return set_verb_code(#90102, "inform_current", {"this.naming_output = $document:render(args[1]:resolve(this), 'text_plain)['body];", "if (!index(this.naming_output, \"\\n\")) return pass(@args); endif"});
0
@nonprogrammer
```

</details>

```moot
% @create $thing named "pet rock",rock
~~Created
```

The name is what people see. An alias is another way to refer to the same object in a command. Both
`take rock` and `take pet rock` work. You can also type `take the pet rock`.

You need builder privileges for this lesson. You do not need programmer privileges. In each example,
`%` marks a command and `=` marks its response. Leave those markers out when typing. The collapsible
sections set up and check the lesson for the test runner.

## Let sentences supply the article

An ordinary thing already knows how to appear in a sentence. Try dropping the rock:

```moot
% drop rock
=You dropped the pet rock.
% look
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "You see a pet rock here.") > 0;
true
@nonprogrammer
```

</details>

The room introduces **a pet rock**. The action refers to **the pet rock** you selected. The object's
name remains `pet rock`; neither article is part of it.

You can check that with `#`, the object lookup command:

```moot
% #rock.name
==> "pet rock"
% #rock.aliases
==> {"rock"}
% get the pet rock
=You picked up the pet rock.
```

The `.name` property holds the main name. `.aliases` holds its additional command names. See
`help #` or the [object lookup lesson](object-shortcuts.md) for more uses of `#`.

## Choose how a room introduces something

A room normally introduces a portable object with `a`, `an`, or `some`. A fixture might read better
with `the`. Make a box and choose that wording:

```moot
% @create $container named "wooden box",box
~~Created
% @grammar box listing definite
=Grammar updated for wooden box.
% drop box
=You dropped the wooden box.
% look
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "You see the wooden box here.") > 0;
true
@nonprogrammer
```

</details>

This changes the box's listing preference. It does not rename the box or change its aliases. Inspect
the settings and preview sentences with:

```moot
% @grammar box
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "Room: You see the wooden box here.") > 0;
true
; return index(#90102.naming_output, "Action: You examine the wooden box.") > 0;
true
; return index(#90102.naming_output, "Subject: The wooden box is here.") > 0;
true
@nonprogrammer
```

</details>

The display says which settings are local and which come from a parent prototype. To restore the
parent's listing choice, use `@grammar box listing inherit`.

## Boots, water, and Henri

Some names need a different grammatical form. A pair of boots is one object, but its name is plural:

```moot
% @create $wearable named boots
~~Created
% @grammar boots number plural
=Grammar updated for boots.
% @grammar boots
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "Room: You see some boots here.") > 0;
true
; return index(#90102.naming_output, "Subject: The boots are here.") > 0;
true
@nonprogrammer
```

</details>

Water is a **mass noun**: we say “some water,” rather than “a water.” Its name still takes a
singular verb:

```moot
% @create $thing named water
~~Created
% @grammar water kind mass
=Grammar updated for water.
% @grammar water
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "Room: You see some water here.") > 0;
true
; return index(#90102.naming_output, "Subject: The water is here.") > 0;
true
@nonprogrammer
```

</details>

A character's personal name needs no added article. Actors inherit that choice:

```moot
% @create $actor named Henri
~~Created
% @grammar Henri
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "Room: You see Henri here.") > 0;
true
; return index(#90102.naming_output, "Subject: Henri is here.") > 0;
true
@nonprogrammer
```

</details>

For an actor called `guard`, use `@grammar guard kind count`. That produces “a guard” and “the
guard.” For a group called `guards`, also use `@grammar guards number plural`.

`kind name` preserves a complete name, including names such as `The Who` or `Ada's mailbox`. It does
not infer grammar from capital letters. Pronouns are a separate choice:
`@set-pronouns Henri to he/him` controls “he” and “his.” A person using “they” can still have a
singular name.

## Correct an unusual pronunciation

Cowbell chooses `a` or `an` from spelling, including common exceptions such as “an hour” and “a
user.” For an abbreviation, tell it how the name sounds:

```moot
% @create $thing named "MRI scanner",scanner
~~Created
% @grammar scanner article an
=Grammar updated for MRI scanner.
% @grammar scanner
```

<details>
<summary>Check the displayed wording</summary>

```moot
@wizard
; return index(#90102.naming_output, "Room: You see an MRI scanner here.") > 0;
true
@nonprogrammer
```

</details>

`article auto` returns to the automatic choice. `article inherit` removes your local setting and
uses the parent's choice. The article setting only affects singular count nouns.

## Write a sentence about the object

When you customize a message, choose the reference its sentence needs:

```text example
@set-message rock.get_msg {The actor} {agree actor name:pick|picks} up {the d}.
```

`{The actor}` supplies the person's name, or “You” for that person. `{the d}` refers to the rock.
`agree` selects “pick” or “picks” to fit the actor's name. `{a d}` introduces an object;
`{listing d}` uses its listing preference.

Start with [messages that fit the reader](../manual/pronouns-and-messages.md) for an explanation of
message templates. Type `help @grammar` for the settings reference.

## When a prototype needs custom wording

Programmers can override `noun_phrase(form, viewer)`, `noun_grammar(viewer)`,
`listing_form(viewer)`, and `match_names(context)` on a prototype. Children inherit those verbs in
the usual way. A box could call itself “the box you left here” for its keeper, without changing its
name for other people.

These verbs answer questions. They must not change the world or suspend. Messages and room lists
call them with the reader's perspective and retain their returned words for that output. `@grammar`
identifies custom verbs and previews their actual results. See the
[programming reference](../manual/message-template-reference.md#object-naming-verbs) for the return
values and an example.
