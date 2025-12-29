# Presentations

The web client supports server-driven UI panels and windows via the `present()` builtin. Presentations are how MOO code
asks the client to display structured UI elements beyond plain text output, delivered through `moor-web-host`.

## The `present()` Builtin

```moo
present(player, id, content_type, target, content, attributes)
present(player, id)  // Dismiss presentation
```

| Parameter      | Description                                                         |
|----------------|---------------------------------------------------------------------|
| `player`       | The player object to show the presentation to                       |
| `id`           | Unique identifier for this presentation (used to update or dismiss) |
| `content_type` | MIME type for content (usually `"text/plain"` or `"text/html"`)     |
| `target`       | Where to show the presentation (see Targets below)                  |
| `content`      | Optional content string                                             |
| `attributes`   | List of `{key, value}` pairs for additional options                 |

To dismiss a presentation, call `present()` with only `player` and `id`.

## Targets and Placement

The client maps semantic targets to dock positions based on screen size:

### Dock Targets

| Target          | Desktop Position | Mobile Position | Use Case                     |
|-----------------|------------------|-----------------|------------------------------|
| `navigation`    | Left dock        | Top             | Maps, compass, location info |
| `communication` | Left dock        | Top             | Chat panels, who lists       |
| `inventory`     | Right dock       | Bottom          | Player inventory             |
| `status`        | Right dock       | Top             | Health, stats, timers        |
| `tools`         | Right dock       | Bottom          | Quick actions, utilities     |
| `help`          | Right dock       | Bottom          | Help panels, hints           |

### Floating Targets

| Target           | Behavior                   | Use Case               |
|------------------|----------------------------|------------------------|
| `window`         | Floating window, draggable | General-purpose popups |
| `object-browser` | Floating or docked         | Object inspection      |

### Editor Targets

| Target                  | Opens                   |
|-------------------------|-------------------------|
| `verb-editor`           | Verb code editor        |
| `property-editor`       | Property text editor    |
| `property-value-editor` | Structured value editor |
| `text-editor`           | Long-form text editor   |

### Dialog Targets

| Target          | Purpose                      |
|-----------------|------------------------------|
| `profile-setup` | Player profile configuration |

## Common Attributes

Attributes are passed as a list of `{key, value}` pairs:

| Attribute  | Used By          | Description                       |
|------------|------------------|-----------------------------------|
| `title`    | All              | Window/panel title                |
| `name`     | All              | Alternative to title              |
| `object`   | Editors, browser | Object reference (e.g., `"#123"`) |
| `verb`     | `verb-editor`    | Verb name to edit                 |
| `property` | Property editors | Property name to edit             |
| `content`  | Editors          | Initial content                   |
| `fields`   | `profile-setup`  | Comma-separated field names       |

## Examples

### Open a Verb Editor

```moo
present(player, "edit-look", "text/plain", "verb-editor", "",
        {{"object", tostr(this)}, {"verb", "look"}, {"title", "Edit look verb"}});
```

### Open a Property Editor

```moo
present(player, "edit-desc", "text/plain", "property-editor", "",
        {{"object", tostr(this)}, {"property", "description"}});
```

### Show Object Browser Focused on an Object

```moo
present(player, "inspect", "text/plain", "object-browser", "",
        {{"object", tostr(target_obj)}});
```

### Show a Status Panel

```moo
content = $format.block:mk(
    "Health: " + tostr(player.health) + "/" + tostr(player.max_health),
    "Mana: " + tostr(player.mana) + "/" + tostr(player.max_mana)
);
present(player, "status-panel", "text/html", "status",
        content:compose(player, 'text_html), {{"title", "Status"}});
```

### Show an Inventory Panel

```moo
items = {};
for item in (player.contents)
    items = {@items, item.name};
endfor
content = $format.list:mk(items):compose(player, 'text_html);
present(player, "inv-panel", "text/html", "inventory", content,
        {{"title", "Inventory"}});
```

### Update an Existing Presentation

Calling `present()` with the same `id` updates the content:

```moo
// Initial presentation
present(player, "timer", "text/plain", "status", "Time: 60", {{"title", "Countdown"}});

// Update it later
present(player, "timer", "text/plain", "status", "Time: 30", {{"title", "Countdown"}});
```

### Dismiss a Presentation

```moo
present(player, "timer");  // Removes the "timer" presentation
```

### Profile Setup Dialog

```moo
present(player, "profile-setup", "text/plain", "profile-setup", "",
        {{"title", "Set up your profile"}, {"fields", "pronouns,description,picture"}});
```

The `fields` attribute controls which profile fields are shown:

- `pronouns` - Pronoun selection
- `description` - Character description
- `picture` - Profile picture upload

## Responsive Behavior

The client automatically adjusts presentation placement based on screen size:

- **Desktop (>768px)**: Dock panels appear in sidebars; floating windows can be dragged
- **Mobile (≤768px)**: Dock panels stack vertically; editors open in full-screen modal mode

Programmers should design presentations to work well at both sizes. Use semantic targets (`inventory`, `status`, etc.)
rather than assuming specific screen positions.

## Related Docs

- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Client Output and Presentations](./client-output-and-presentations.md)
- [Authoring Tools](./authoring-tools.md)
- [Networking](../the-moo-programming-language/networking.md)
