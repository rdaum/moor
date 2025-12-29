# Client Output and Presentations

This page describes how the web client interprets `notify()` and `present()` output, including content types and
metadata that affect rendering. These events flow through `moor-web-host` over FlatBuffers and are rendered with the web
client's richer UI.

## Rich Content Formats

The web client supports rich output formats and applies strict safety rules to prevent spoofing or unsafe rendering.

### Djot

Djot is a modern, structured markup format designed for predictable rendering with fewer edge cases than Markdown. We
use it because it supports rich formatting while keeping tighter restrictions that reduce spoofing risks.

Learn more: <https://github.com/jgm/djot>

### HTML

HTML output is supported, but it is heavily sanitized (via DOMPurify) before rendering and presented with a very
restricted subset of elements. This allows basic formatting while preventing untrusted content from injecting scripts or
spoofing the UI.

## Login and Welcome Screen Customization

When a browser connects, the web client invokes the welcome flow through `moor-web-host` and renders the narrative
output from the login command. You can customize this welcome message using rich content types:

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

| Content Type     | Description                       |
|------------------|-----------------------------------|
| `text/plain`     | Plain text (default)              |
| `text/html`      | HTML-formatted output (sanitized) |
| `text/djot`      | Djot-formatted output             |
| `text/x-uri`     | URL rendered in an iframe         |
| `text/traceback` | Stack trace formatting            |

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

| Link Pattern          | Action                                                |
|-----------------------|-------------------------------------------------------|
| `moo://cmd/<command>` | Run the URL-decoded command as if typed by the player |
| `moo://inspect/<ref>` | Open object inspector (if programmer)                 |
| `moo://help/<topic>`  | Show help for topic                                   |

```moo
notify(player, "[look](moo://cmd/look)", false, false, "text/djot");
notify(player, "[examine sword](moo://cmd/examine%20sword)", false, false, "text/djot");
```

## notify() Metadata

The web client reads metadata attached to narrative events. Common keys include:

| Key                 | Type   | Description                                      |
|---------------------|--------|--------------------------------------------------|
| `presentation_hint` | string | Styling hint (inset, processing, expired)        |
| `group_id`          | string | Group related lines together                     |
| `tts_text`          | string | Alternate text for screen readers                |
| `thumbnail`         | list   | `[content_type, binary_data]` for preview images |
| `link_preview`      | map    | Rich link preview data                           |

If your core uses metadata, you can shape how the web client presents or groups output without changing the visible
message body.

**notify() targeting:**

- `notify(player, ...)` — sends to **all** of that player's connections AND writes to the event log (persistent history)
- `notify(connection, ...)` — sends only to that specific connection (negative object number), does NOT write to event log or other connections

The `event_log()` builtin writes to the persistent event log without displaying anything—useful for background logging.

### Presentation Hints

`presentation_hint` guides visual treatment for a line or group:

| Hint         | Visual Treatment                                 |
|--------------|--------------------------------------------------|
| `inset`      | Render in an inset card (look output, summaries) |
| `processing` | Show spinner/animation (in-progress operations)  |
| `expired`    | Faded appearance (stale content)                 |

```moo
metadata = ["presentation_hint" -> "processing"];
notify(player, "Calibrating sensors...", 0, 0, "text/plain", metadata);
```

### Grouping with group_id

When consecutive messages share the same `presentation_hint` and `group_id` (and the same actor, if provided), the web
client visually groups them together. This enables:

- Multi-line look descriptions shown as a single card
- Collapse/expand behavior for grouped output
- Consistent visual treatment across related messages

```moo
// Grouped look description
metadata = ["presentation_hint" -> "inset", "group_id" -> "look:#123"];
notify(player, "You are in the Observatory.", 0, 0, "text/plain", metadata);
notify(player, "A brass telescope points skyward.", 0, 0, "text/plain", metadata);
```

### Message Staleness

The client automatically marks certain messages as "stale" when superseded. For example, when a player runs `look`
again, the previous `look` output is visually dimmed and its links become non-interactive. This helps players understand
which information is current.

## Rewritable Messages

For dynamic content that updates in place (e.g., progress indicators, streaming AI responses), the client supports
rewritable messages.

```moo
// Initial message with rewritable ID
metadata = [
    "presentation_hint" -> "processing",
    "rewritable" -> ["id" -> "task-123", "owner" -> tostr(this), "ttl" -> 30]
];
notify(player, "Processing...", 0, 0, "text/plain", metadata);

// Later: rewrite the message
metadata = ["rewrite_target" -> "task-123"];
notify(player, "Processing complete!", 0, 0, "text/plain", metadata);
```

Rewritable messages have:

- **id**: Unique identifier for the message slot
- **owner**: Object that owns this slot (security check)
- **ttl**: Time-to-live in seconds before expiry
- **fallback** (optional): Content to show if TTL expires without rewrite

## Rich Input Prompts

The web client supports structured input prompts that go beyond simple text input. Use `request_input()` to trigger
these.

### Input Types

| Type                     | UI Control                 | Use Case                  |
|--------------------------|----------------------------|---------------------------|
| `text`                   | Single-line text field     | Names, short answers      |
| `text_area`              | Multi-line textarea        | Descriptions, long text   |
| `number`                 | Number input               | Quantities, coordinates   |
| `choice`                 | Buttons or dropdown        | Multiple choice selection |
| `yes_no`                 | Yes/No buttons             | Binary questions          |
| `yes_no_alternative`     | Yes/No/Alternative buttons | With custom option        |
| `yes_no_alternative_all` | Yes/Yes All/No/Alternative | Batch approvals           |
| `confirmation`           | OK button                  | Acknowledgments           |
| `image`                  | File picker with preview   | Image uploads             |
| `file`                   | File picker                | General file uploads      |

### Input Metadata Fields

| Field                     | Used By                 | Description                          |
|---------------------------|-------------------------|--------------------------------------|
| `input_type`              | All                     | Type of input control                |
| `prompt`                  | All                     | Prompt text (supports Djot)          |
| `tts_prompt`              | All                     | Accessible prompt for screen readers |
| `placeholder`             | text, text_area, number | Placeholder text                     |
| `default`                 | text, text_area, number | Default value                        |
| `choices`                 | choice                  | List of options                      |
| `min` / `max`             | number                  | Value constraints                    |
| `rows`                    | text_area               | Number of rows                       |
| `accept_content_types`    | image, file             | Allowed MIME types                   |
| `max_file_size`           | image, file             | Maximum file size in bytes           |
| `alternative_label`       | yes_no_alternative*     | Label for alternative input          |
| `alternative_placeholder` | yes_no_alternative*     | Placeholder for alternative          |

### Examples

```moo
// Simple text input
request_input(player, ["input_type" -> "text", "prompt" -> "What is your name?"]);

// Number with constraints
request_input(player, [
    "input_type" -> "number",
    "prompt" -> "How many items?",
    "min" -> 1,
    "max" -> 100,
    "default" -> 10
]);

// Multiple choice
request_input(player, [
    "input_type" -> "choice",
    "prompt" -> "Choose a direction:",
    "choices" -> {"North", "South", "East", "West"}
]);

// Yes/No with alternative (for AI agent approvals)
request_input(player, [
    "input_type" -> "yes_no_alternative",
    "prompt" -> "Apply this change?\n```moo\nplayer.score = 100;\n```",
    "alternative_label" -> "Suggest a different approach:"
]);

// Image upload
request_input(player, [
    "input_type" -> "image",
    "prompt" -> "Upload your profile picture:",
    "accept_content_types" -> {"image/png", "image/jpeg", "image/gif"},
    "max_file_size" -> 1048576  // 1MB
]);
```

## present() Targets and Attributes

The `present()` builtin is used to open or update panels and windows. See [Presentations](./presentations.md) for
comprehensive documentation.

Quick reference:

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

## Related Docs

- [Networking](../the-moo-programming-language/networking.md)
- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Presentations](./presentations.md)
- [Accessibility](./accessibility.md)
