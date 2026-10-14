# Start programming

Builder commands let you configure existing objects. Programming lets you add an operation that
those objects do not yet provide. For example, the pet rock lesson starts by adding a `pet` command.
Later changes use reactions, messages, and relations to choose the rock's response.

An object's **properties** hold information, such as its description or a visitor count. Its
**verbs** contain instructions. A child object can use verbs inherited from its prototype, so you
can make several rocks without copying the same program into each one.

You need programmer privileges to write verbs or evaluate MOO code. The tutorials also use builder
commands and state when builder privileges are needed.

## Your first projects

[The pet rock](pet-rock.md) is a first programming lesson. You do not need to understand its code in
advance. It introduces each part before using it to make the rock do something new.

[The tour of `#`](object-shortcuts.md) teaches a useful companion tool. It finds an object, shows
its properties, changes values, and calls verbs. Try the first few sections before the rock, or
return to them when you need to inspect your work.

After the rock, [Henri](../actors/henri-and-the-cupboard.md) brings several objects together in one
scene. [The portal lesson](custom-exits.md) shows how a new kind of exit participates in room
descriptions and travel commands.

## Beyond the tutorials

The book still needs a tour of the editing and debugging tools, followed by a guide to designing
prototypes other people can customize. It also needs a worked example of events and documents: how
an action becomes a message for each observer, and how that message reaches Meadow or a terminal.

For the language itself, use the
[mooR programming manual](https://timbran.org/book/2.0-dev/html/the-moo-programming-language.html).
This book concentrates on writing objects that work with Cowbell's commands and conventions.
