# Client Output and Presentations

This page describes how the web client interprets `notify()` and `present()` output, including content types and
metadata that affect rendering. These events flow through `moor-web-host` over FlatBuffers and are rendered with the
web client's richer UI.

## Rich Content Formats

The web client supports rich output formats and applies strict safety rules to prevent spoofing or unsafe rendering.

### Djot

Djot is a modern, structured markup format designed for predictable rendering with fewer edge cases than Markdown. We use
it because it supports rich formatting while keeping tighter restrictions that reduce spoofing risks.

Learn more:

- https://github.com/jgm/djot

### HTML

HTML output is supported, but it is heavily sanitized (via DOMPurify) before rendering and presented with a very
restricted subset of elements. This allows basic formatting while preventing untrusted content from injecting scripts or
spoofing the UI.

## Login and Welcome Screen Customization

When a browser connects, the web client invokes the welcome flow through `moor-web-host` and renders the narrative output
from the login command. You can customize this welcome message using rich content types:

- `text/html`: sanitized HTML for branded layouts
- `text/djot`: djot markup for structured content
- `text/x-uri`: a URL rendered in a sandboxed iframe

```moo
// Example: in $do_login_command with no args
notify(connection, "= Welcome to the Observatory\n*Please log in to continue.*", false, false, "text/djot");
```

For iframe welcome content, return a URL as `text/x-uri`:

```moo
notify(connection, "https://example.com/welcome.html", false, false, "text/x-uri");
```

The iframe is sandboxed and intended for trusted, static content.

## notify() Content Types

The `notify()` builtin can specify a `content_type` in rich mode. The web client understands:

- `text/plain`: Plain text (default)
- `text/html`: HTML-formatted output (sanitized)
- `text/djot`: Djot-formatted output
- `text/x-uri`: Rendered in an iframe (used for welcome screens)

```moo
// Plain text
notify(player, "You see a lantern.");

// HTML
notify(player, "<strong>Warning:</strong> Low power.", false, false, "text/html");

// Djot
notify(player, "= Status\n*All systems nominal*", false, false, "text/djot");
```

### Inline Links

When sending `text/html` or `text/djot`, the web client turns anchors into interactive elements. It recognizes `moo://`
links for client-side actions:

- `moo://cmd/<command>`: Run the URL-decoded command as if typed by the player.
- `moo://inspect/<ref>`: Reserved for object inspection.
- `moo://help/<topic>`: Reserved for contextual help.

```moo
notify(player, "[look](moo://cmd/look)", false, false, "text/djot");
```

## notify() Metadata

The web client also reads metadata attached to narrative events. Common keys include:

- `presentation_hint`: Styling hint (e.g. inset, processing, expired)
- `group_id`: Group related lines together
- `tts_text`: Alternate text for screen readers
- `thumbnail`: `[content_type, binary_data]` image for previews

If your core uses metadata, you can shape how the web client presents or groups output without changing the visible
message body. Most cores pair `notify()` with `event_log()` so the same metadata applies to live output and history.

### Presentation Hints

`presentation_hint` guides visual treatment for a line or group. Common values include:

- `inset`: Render in an inset card (useful for look output or summaries)
- `processing`: Render as in-progress or transient output
- `expired`: Render as faded or stale output

```moo
metadata = ["presentation_hint" -> "processing"];
event_log(player, "Calibrating sensors...", 'text_plain, metadata);
```

### Grouping with group_id

When consecutive messages share the same `presentation_hint` and `group_id` (and the same actor, if provided), the web
client visually groups them together. This is useful for look descriptions, multi-line notices, or composite messages, and
it also enables collapse/expand behavior for grouped look output.

```moo
// Log a grouped look description to history
metadata = ["presentation_hint" -> "inset", "group_id" -> "look:#123"];
event_log(player, "You are in the Observatory.", 'text_plain, metadata);
event_log(player, "A brass telescope points skyward.", 'text_plain, metadata);
```

## present() Targets and Attributes

The `present()` builtin is used to open or update panels and windows. The target selects the UI surface, and attributes
provide additional context.

Common targets:

- `window`: Floating window
- `navigation`, `communication`: Left dock on desktop, top on mobile
- `inventory`, `status`, `tools`, `help`: Right dock on desktop, top/bottom on mobile
- `verb-editor`, `property-editor`, `property-value-editor`, `text-editor`: Editing tools
- `object-browser`: Object browser panel/window
- `profile-setup`: Profile setup dialog

Common attributes:

- `title`/`name`: UI title text
- `object`, `verb`: Used by editor tools
- `property`: Used by property editors
- `fields`: Comma-separated profile fields (`pronouns`, `description`, `picture`)

```moo
// Open a verb editor
present(player, "edit-look", "text/plain", "verb-editor", "",
        {{"object", "#123"}, {"verb", "look"}, {"title", "Edit look"}});

// Show profile setup dialog
present(player, "profile-setup", "text/plain", "profile-setup", "",
        {{"title", "Set up your profile"}, {"fields", "pronouns,description"}});

// Dismiss a presentation
present(player, "edit-look");
```

Related docs:

- [Networking](../the-moo-programming-language/networking.md)
- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Presentations](./presentations.md)
