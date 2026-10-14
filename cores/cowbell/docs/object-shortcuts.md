# Find it, inspect it, change it: a tour of `#`

Sometimes you just want to find your lamp, check who owns it, or change one of its settings. `#` is
Cowbell's Swiss army knife for jobs like these. Start with an object's name, then add what you want
to do with it.

This short lesson makes a lamp to experiment with. You'll read its properties, change a setting, and
call a verb. You can get the command reference at any time by typing `help #` or `help hash` inside
the world.

## Before you begin

You need builder privileges to look up objects with `#`. Changing properties and running code also
require programmer privileges. Ask the person who runs your world if you need those privileges.

In the examples, `%` marks a command to type. `=` marks the response; `~~` shows part of a response.
Leave these markers out when you type. For example, `==> "lamp"` means you should see `=> "lamp"`.
The collapsible sections are instructions for the test runner, not commands for you to enter.

<details>
<summary>Test setup</summary>

```moot
@wizard
; #90101.name = "Ada"; #90101.is_builder = true; #90101.authoring_features = $prog_features; return true;
true
@programmer
; return player.programmer && !player.wizard;
true
```

</details>

## Find your lamp

Make an object called `lamp`:

```moot
% @create $thing named lamp
~~Created
% @describe lamp as A small brass lamp.
~~Set description
```

Your new lamp starts in your inventory. Try this command:

```moot
% #lamp
~~Object
```

It shows the lamp's identifier, name, owner, and location. An identifier looks like `#123` or a
longer UUID identifier. You can use that identifier instead of the name in later commands.

<details>
<summary>Check the object summary</summary>

```moot
=
~~Name
=
~~Owner
=
~~Location
```

</details>

Keep the name attached to `#`, with no space between them. Names containing spaces need a short
alias or an identifier here. For example, a lamp called “small brass lamp” could have the alias
`lamp`.

Ordinary lookup searches your inventory and room first, then players, then passages. A name must
identify one object. If you have two lamps, use an identifier or a more specific name.

## Read its properties

An object stores information in **properties**. A dot selects one:

```moot
% #lamp.name
==> "lamp"
% #lamp.description
==> "A small brass lamp."
```

A property can refer to another object. Add another dot to follow that reference:

```moot
% #lamp.owner.name
==> "Ada"
```

You'll see your own name here. The command finds the lamp, follows its `owner` property, then reads
that owner's `name`.

There is also a `.parent` shortcut. It finds the prototype the object inherits from:

```moot
% #lamp.parent.name
==> "Generic Thing"
```

Your lamp inherits its ordinary object behavior from `$thing`.

## Choose where to look

Some names can refer to several things. A **scope** limits the search:

| Command           | Where it looks                                     |
| ----------------- | -------------------------------------------------- |
| `#lamp inventory` | Objects you carry                                  |
| `#Ada player`     | Players                                            |
| `#north exit`     | Passages from your current room, by label or alias |

Put the scope after any properties:

```moot
% #lamp.name inventory
==> "lamp"
% #Ada.name player
==> "Ada"
```

Replace Ada with your own player name. A scope does not move anything or change your permissions.

Some references do not need a name search:

| Reference | What it selects                        |
| --------- | -------------------------------------- |
| `#me`     | You                                    |
| `#here`   | Your current room                      |
| `#123`    | Object number 123, wherever it is      |
| `#$thing` | The system's ordinary object prototype |
| `#@Ada`   | The player named Ada                   |

The leading `#` starts the command. In `#123`, it also forms the object identifier; use one `#`.

## Change a value

First, add a setting to the lamp:

```moot
% @property lamp.brightness 0 rc
~~Added
```

`@property` adds a property that does not exist yet. `0` is its starting value. The `rc` flags let
others read it and let child objects own their copy.

Now use `=` to assign a different value:

```moot
% #lamp.brightness = 3
~~.brightness = 3
% #lamp.brightness
==> 3
```

You can also calculate the value. Here, the right-hand side is a MOO **expression**: code that
produces a value.

```moot
% #lamp.brightness = 2 * 4
~~.brightness = 8
```

For text, use double quotes:

```moot
% #lamp.description = "A brass lamp with a green shade."
~~.description = "A brass lamp with a green shade."
```

These assignments require programmer privileges and permission to write the property. They do not
create missing properties. The brightness value also does not make the lamp emit light: that needs
behavior which uses the setting.

## Do something with the result

After `for`, you can use the selected object or value in an expression. `%#` stands for that result.

```moot
% #lamp.brightness for %# + 1
==> 9
% #lamp.brightness
==> 8
```

The first command reads `brightness` and adds one. It displays the answer without changing the
stored value. An assignment changes it:

```moot
% #lamp for %#.brightness = %#.brightness + 1
==> 9
% #lamp.brightness
==> 9
```

Here, `%#` means the lamp itself because we selected `#lamp`, without a property suffix.

You can combine a scope with `for`:

```moot
% #lamp.brightness inventory for %# * 2
==> 18
```

Use one expression after `for`, without `return`. `%#` is a text substitution; it is replaced even
inside quoted strings. Ordinary `=` assignments do not perform that substitution.

## Call a verb

A **verb** is a piece of code attached to an object. Use a colon and parentheses to call one. Let's
borrow a cat for this example:

```moot
% @create $actor named Mabel
~~Created
% #Mabel.pronouns = $pronouns:lookup("she/her")
~~.pronouns =
% #Mabel for %#:pronouns_display()
==> "she/her"
```

The assignment asks `$pronouns` for the pronoun set called `she/her`, then stores it on Mabel. The
final command finds Mabel and calls her `pronouns_display` verb. The empty parentheses mean that
this call takes no arguments.

`#Mabel:pronouns_display()` is not a command shortcut in Cowbell. Use the `for` form above.

## When to use another command

Use `#` to inspect objects and work with individual properties or verbs. Building commands often
provide a more convenient way to perform a larger operation:

- `@describe` edits descriptions and supports formatted text.
- `@move` moves an object; assigning its `location` property does not.
- `@parent` changes an object's prototype.
- `@passage` and `@set-passage` inspect and edit passages.

Passages are stored values rather than independent objects. `#north exit` can inspect one, but
editing a temporary copy does not save it back into the world.

All ordinary permissions still apply. If you get a permission error, check whether you own the
object and whether you have the required privileges. `#` gives you a shorter way to ask; it does not
give you another person's authority.

For more practice, [make a pet rock](programming-with-rules-and-reactions.md) or
[make room for Henri](henri-and-the-cupboard.md). Both lessons use `#` to work with their objects.
For a reminder of the syntax while building, type `help #`.

## Where this command came from

Cowbell's `#` follows the longstanding LambdaMOO/LambdaCore utility.
