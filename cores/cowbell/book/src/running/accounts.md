# Accounts, roles, and community administration

A person connects through a player account. That account owns their creations and carries the
permissions used when they act in the world. The server operator decides how people obtain accounts
and which additional privileges they receive.

Start with a separate ordinary account for everyday use. Keep wizard access for work that requires
it. Testing only as a wizard can hide permission problems that visitors and builders will encounter.

## Create an account

Fresh Cowbell allows account creation. Meadow provides a registration interface. At a terminal's
login prompt, the equivalent form is:

```text
create Ada CHOOSE-A-PASSWORD
```

Replace the name and password with your own choices. This is a login command, before connecting as
an existing player. Inside the world, `@create` means creating an object and has a different syntax.

The current registration command requires player names without spaces. That restriction concerns
account names. Ordinary objects can have names such as `pet rock` and aliases such as `rock`.

Cowbell offers a profile setup step for a new player. Pronouns and a description help other people
recognize the character. An account's login password and its Meadow history encryption password
serve different purposes. Changing the login password does not recover a forgotten history key.

## Understand the kinds of authority

| Access                  | What it means                                                                                     |
| ----------------------- | ------------------------------------------------------------------------------------------------- |
| Ordinary player         | Uses the world's social and object interactions.                                                  |
| Builder                 | Uses building commands, still subject to ownership and area permissions.                          |
| Programmer              | Can write MOO code. The grant command also enables builder abilities.                             |
| Wizard                  | Has broad authority over objects and protected server operations.                                 |
| Delegated administrator | Can use explicitly allowed administrative commands through another account's delegated authority. |

Ownership and grants matter within these roles. Builder status does not give someone permission to
connect passages into any room. A programmer does not automatically own another person's objects.

The operating-system account that runs mooR is another identity. Wizard status inside the MOO does
not itself provide a shell login to the server computer.

## Give a player building or programming access

As a wizard, use the player's name:

```text
@builder Ada
```

This grants building commands and creates an Architect's Compass for the player. If the player lacks
a description, the command asks whether to continue.

When Ada needs to write verbs:

```text
@programmer Ada
```

This enables programming and building, and supplies the relevant tools. Their language-model
features still need configuration. The presence of a Compass or Visor does not establish model
access.

`@builder Ada` also serves as a downgrade from programmer to builder. It asks for confirmation,
removes the programmer flag, and removes a Data Visor. It is not a command for revoking every kind
of privilege. In particular, do not use it as a substitute for reviewing wizard or delegated access.

`@wizard Ada` grants wizard access. Use that only when the person needs authority over the whole
world, rather than permission for one building task.

After changing privileges, let the person try the intended task from their own account. Check a
denied operation as well, so the boundary is visible.

## Delegate a particular administrative job

Cowbell has a separate `@sudo` mechanism. It combines a delegated wizard identity with a list of
commands the recipient can use. It does not make that player a wizard.

The related commands include `@sudo-grant`, `@sudo-allow`, `@sudo-show`, `@sudo-revoke`, and
`@sudo-log`. Start with their in-world help. A complete grant/use/revoke tutorial still needs to be
written and tested with two accounts.

Do not assume delegated access covers every action inside a command. For example, Git fetching and
new object creation through `@changes` require an actual wizard. The
[capability guide](https://github.com/timbran-project/moor/blob/main/cores/cowbell/CAPABILITIES.md)
describes these checks for programmers implementing administrative tools.

## Set the world's welcome and registration policy

The login object stores the public-facing settings. These include:

| Property on `$login`                                 | Purpose                                          |
| ---------------------------------------------------- | ------------------------------------------------ |
| `moo_title`                                          | The world's title.                               |
| `welcome_message`                                    | Text shown before login.                         |
| `player_creation_enabled`                            | Whether people can create accounts.              |
| `registration_string`                                | The response when creation is disabled.          |
| `default_home`                                       | The starting location for new players.           |
| `new_player_welcome_message` and `new_player_letter` | The greeting and letter for new accounts.        |
| `privacy_policy`                                     | Policy content presented through login features. |

For example, a wizard can use [the object shortcut command](../programming/object-shortcuts.md) to
change the title or close registration:

```text
#$login.moo_title = "The Lantern House"
#$login.player_creation_enabled = false
```

These assignments change properties on the existing login object. They do not replace its programs.
To reopen registration, assign `true` instead of `false`.

Before public use, revise the supplied welcome message that advertises the initial wizard password.
Review the other initial accounts too. Policy text exists both on the login object and in help
topics. Some supplied help refers to the Timbran Hotel; it is not a policy written for your
community.

Explain what your installation actually does with mail, event history, administrator access, and
model requests. An encrypted history store does not make every world property or every external
model request private.

Account recovery, full privilege revocation, and delegated administration still need complete
operating procedures. Those are recorded as documentation gaps rather than assumed to be covered by
the grant commands above.
