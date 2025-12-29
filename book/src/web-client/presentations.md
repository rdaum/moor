# Presentations

The web client supports server-driven UI panels and windows via the `present()` builtin. Presentations are how MOO code
asks the client to display structured UI elements beyond plain text output, delivered through `moor-web-host`.

## Targets and Placement

Supported presentation targets include:

- `window`: Floating window
- `navigation`, `communication`: Left dock on desktop, top on mobile
- `inventory`, `status`, `tools`, `help`: Right dock on desktop, top/bottom on mobile
- `verb-editor`, `property-editor`, `property-value-editor`, `text-editor`: Editing tools
- `object-browser`: Object browser panel/window
- `profile-setup`: Profile setup dialog

The client maps these semantic targets to dock placements based on screen size.

## Driving Presentations from MOO

Use `present()` to create or update a presentation, and call it again with only `player` and `id` to dismiss it. You can
attach attributes such as `title` and tool-specific fields (for example `object`/`verb` for editors).

Related docs:

- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Networking](../the-moo-programming-language/networking.md)
