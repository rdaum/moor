# Authoring and Programming Tools

The web client includes built-in tools for building and editing MOO content without leaving the browser. These tools use
`moor-web-host` endpoints to read and write objects, verbs, and properties. Availability can be restricted based on
permissions and server configuration.

## Access and Permissions

The web client uses the programmer flag from the login/session payload to decide whether to show authoring features in
the UI. If the current player does not have the programmer flag, the object browser and eval panel controls are hidden and
server-driven object browser presentations are dismissed.

All requests still pass through `moor-web-host`, and the daemon enforces permissions on the underlying operations, so
clients cannot bypass server-side access checks.

## Object Browser

The object browser lets builders inspect objects, browse properties and verbs, and navigate object hierarchies. It is
shown when the logged-in player has the programmer flag. The browser can be opened from the top navigation bar or
triggered by a `present()` event targeting `object-browser`.

## Verb Editor

The verb editor provides syntax highlighting and autocompletion for MOO code. Verbs can be opened from the object browser
or presented by the server, and changes are saved back to the database via `moor-web-host`.

## Property Editor

The property editor supports creating and updating properties and their permissions. The property value editor provides a
structured editor for values, including lists and maps, and can be launched from the object browser or via `present()`.

## Text Editor

For long-form text (descriptions, help, or custom content), the text editor provides a larger editing surface and supports
`text/plain` and `text/djot` content. It can be opened from the object browser or presented by the server.

## Eval Panel

The eval panel lets programmers evaluate MOO expressions or statements in-place and see the result immediately. It runs
through the server eval endpoint (the same underlying capability as the `eval()` builtin). The UI is shown only when the
player has the programmer flag.

Related docs:

- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Object Properties](../the-database/object-properties.md)
- [Object Verbs](../the-database/object-verbs.md)
