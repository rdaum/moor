# $sub - Event Substitution System

## Overview

The `$sub` system provides template-based text substitution for events in the MOO. It allows you to
write narrative descriptions that automatically adapt based on perspective (first-person vs
third-person) and grammatical context.

Substitutions are created as lightweight flyweight objects that get evaluated when events are
rendered to players. This means the same event can produce different text for different viewers.

## Basic Concept

When you create an event, you build content using substitution flyweights:

```moo
content = {$sub:nc(), " picks up ", $sub:d(), "."};
```

When this event is rendered:

- **To the actor**: "You pick up the sword."
- **To others**: "Alice picks up the sword."

## Name Substitutions

These substitute names of objects and participants in the event.

### Actor Names

| Verb        | Output                 | Example (actor is Alice) |
| ----------- | ---------------------- | ------------------------ |
| `$sub:n()`  | Actor name             | "Alice" / "you"          |
| `$sub:nc()` | Capitalized actor name | "Alice" / "You"          |

### Object Names

| Verb        | Description          | Example               |
| ----------- | -------------------- | --------------------- |
| `$sub:d()`  | Direct object        | "the sword" / "you"   |
| `$sub:dc()` | Capitalized dobj     | "The sword" / "You"   |
| `$sub:i()`  | Indirect object      | "the chest" / "you"   |
| `$sub:ic()` | Capitalized iobj     | "The chest" / "You"   |
| `$sub:t()`  | This object          | "the door" / "you"    |
| `$sub:tc()` | Capitalized this     | "The door" / "You"    |
| `$sub:l()`  | Location             | "the tavern" / "here" |
| `$sub:lc()` | Capitalized location | "The tavern" / "Here" |

## Pronoun Substitutions

These substitute pronouns based on the actor's or object's gender settings.

### Actor Pronouns

| Verb       | Type                 | Example (he/him)       | Example (they/them)       |
| ---------- | -------------------- | ---------------------- | ------------------------- |
| `$sub:s()` | Subject              | "he" / "you"           | "they" / "you"            |
| `$sub:o()` | Object               | "him" / "you"          | "them" / "you"            |
| `$sub:p()` | Possessive adjective | "his" / "your"         | "their" / "your"          |
| `$sub:q()` | Possessive noun      | "his" / "yours"        | "theirs" / "yours"        |
| `$sub:r()` | Reflexive            | "himself" / "yourself" | "themselves" / "yourself" |

Add `c` for capitalized versions: `$sub:sc()`, `$sub:oc()`, etc.

### Direct Object Pronouns

| Verb            | Type            | Example              |
| --------------- | --------------- | -------------------- |
| `$sub:s_dobj()` | Subject         | "he" / "it"          |
| `$sub:o_dobj()` | Object          | "him" / "it"         |
| `$sub:p_dobj()` | Possessive adj  | "his" / "its"        |
| `$sub:q_dobj()` | Possessive noun | "his" / "its"        |
| `$sub:r_dobj()` | Reflexive       | "himself" / "itself" |

Also available with capitalization: `$sub:sc_dobj()`, etc.

### Indirect Object Pronouns

Same pattern as dobj, but using `_iobj` suffix:

- `$sub:s_iobj()`, `$sub:o_iobj()`, `$sub:p_iobj()`, etc.
- Capitalized: `$sub:sc_iobj()`, `$sub:oc_iobj()`, etc.

## Verb Conjugation

These conjugate verbs based on person and number.

### Actor Verb Conjugation

| Verb               | 2nd person (you) | 3rd person (he/she/it) |
| ------------------ | ---------------- | ---------------------- |
| `$sub:verb_be()`   | "are"            | "is"                   |
| `$sub:verb_have()` | "have"           | "has"                  |
| `$sub:verb_look()` | "look"           | "looks"                |

### Object Verb Conjugation

Same verbs available with `_dobj` or `_iobj` suffix:

- `$sub:verb_be_dobj()`, `$sub:verb_have_iobj()`, etc.

## Self-Alternation

The `self_alt()` method chooses between two alternatives based on whether the viewer is the actor.

```moo
$sub:self_alt(for_self, for_others)
```

### Basic Usage

```moo
content = {$sub:nc(), " ", $sub:self_alt("feel", "feels"), " tired."};
```

- **To actor**: "You feel tired."
- **To others**: "Alice feels tired."

### Nested Substitutions

You can nest substitutions inside `self_alt()`:

```moo
content = {
    $sub:nc(), " ", $sub:self_alt("try", "tries"),
    " to pet the cat, but it swats ",
    $sub:self_alt("your", $sub:p()), " hand away."
};
```

- **To actor**: "You try to pet the cat, but it swats **your** hand away."
- **To others**: "Alice tries to pet the cat, but it swats **her** hand away."

The nested `$sub:p()` is only evaluated when the viewer is not the actor.

### Capitalization

Use `$sub:self_altc()` to capitalize the result:

```moo
$sub:self_altc("you're", "they're")
```

## Complete Example

Here's a full example of building event content:

```moo
verb pet (this none this)
    dobj = args[1];

    if (!dobj:allows_petting())
        "Build content for failed petting attempt";
        content = {
            $sub:nc(), " ", $sub:self_alt("try", "tries"),
            " to pet ", $sub:d(), ", but ", $sub:d(),
            " hisses and swats ", $sub:self_alt("your", $sub:p()),
            " hand away."
        };

        event = $event:mk(player, #-1, this, dobj, #-1, content, {});
        event:send_to_location();

        "Tell actor they failed";
        notify(player, "The cat doesn't want to be petted right now.");
        return;
    endif

    "Build content for successful petting";
    content = {
        $sub:nc(), " ", $sub:self_alt("pet", "pets"), " ", $sub:d(),
        " gently, and ", $sub:d(), " purrs contentedly."
    };

    event = $event:mk(player, #-1, this, dobj, #-1, content, {});
    event:send_to_location();
endverb
```

When Alice pets the friendly cat:

- **Alice sees**: "You pet the friendly cat gently, and the friendly cat purrs contentedly."
- **Bob sees**: "Alice pets the friendly cat gently, and the friendly cat purrs contentedly."

When Alice tries to pet the grumpy cat:

- **Alice sees**: "You try to pet the grumpy cat, but the grumpy cat hisses and swats your hand
  away."
- **Bob sees**: "Alice tries to pet the grumpy cat, but the grumpy cat hisses and swats her hand
  away."

## Implementation Notes

- All substitution verbs return flyweight objects with a `.type` property
- The actual substitution happens when the event is rendered via `:render_as()` or `:compose()`
- Substitutions check if `event.actor == render_for` to determine perspective
- The `name_sub()` method handles the "you" vs object name logic
- Nested substitutions are evaluated recursively during rendering
- If a dobj/iobj is missing, placeholder text like `"<no-dobj>"` is returned

## Phrase Utilities

The `$sub:phrase()` verb provides text manipulation:

```moo
$sub:phrase(text, options)
```

Options:

- `'strip_period` - Remove trailing period
- `'initial_lowercase` - Lowercase first character

Example:

```moo
result = $sub:phrase("Hello world.", {'strip_period, 'initial_lowercase});
"Result: hello world";
```

## Technical Details

- All substitution verbs use wildcard matching (e.g., `"n* nc*"`) to handle variations
- Capitalization is controlled by checking for `"c"` in the verb name
- The flyweight's `.capitalize` property is checked during rendering
- Events are composed of lists that may contain strings, flyweights, or other content
- The system recurses through content, evaluating each substitution flyweight it encounters
