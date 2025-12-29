# Accessibility

The web client is designed to work well with standard browser accessibility tools, including screen readers, keyboard
navigation, and reduced motion preferences.

## Keyboard Navigation

The web client supports full keyboard navigation:

| Key       | Action                                      |
|-----------|---------------------------------------------|
| Tab       | Move focus to next interactive element      |
| Shift+Tab | Move focus to previous element              |
| Enter     | Activate focused button or link             |
| Escape    | Close dialogs, dismiss prompts              |
| Up/Down   | Navigate command history (in command input) |

## Screen Reader Support

The web client uses ARIA attributes and live regions to communicate with screen readers effectively.

### Live Regions

- **Narrative output**: Announced as new content arrives
- **Status changes**: Connection state, loading progress
- **Prompts**: Rich input prompts announced when they appear
- **Errors**: Compile errors and validation messages

### TTS Text Alternatives

When rich visual output would be difficult for screen readers to parse, MOO code can provide alternative text via
metadata:

| Metadata Key | Purpose                                        |
|--------------|------------------------------------------------|
| `tts_text`   | Alternative narration for complex output       |
| `tts_prompt` | Alternative prompt text for rich input prompts |

#### Using raw notify()

The `notify()` builtin accepts metadata as its final argument:

```moo
// Simple TTS alternative with notify()
metadata = ["tts_text" -> "Gate status: open"];
notify(player, "<div class='status'><strong>Gate</strong> OPEN</div>", false, false, "text/html", metadata);

// Table with accessible summary
html = "<table><tr><th>Player</th><th>Score</th></tr><tr><td>Alice</td><td>100</td></tr></table>";
metadata = ["tts_text" -> "Player scores: Alice has 100 points"];
notify(player, html, false, false, "text/html", metadata);
```

#### Using cowbell's event system

The `cowbell` core provides higher-level helpers that work with `notify()` and the web client to make it easier to
provide accessible output:

```moo
// Using $format helpers with metadata
content = $format.table:mk({"Player", "Score"}, {{"Alice", 100}, {"Bob", 85}});
event = $event:mk_info(player, content:compose(player, 'text_html));
event = event:with_tts("Player scores: Alice 100 points, Bob 85 points");
player:inform_current(event);
```

The `:with_tts()` builder method sets the `tts_text` metadata key.

## Rich Input Prompt Accessibility

Rich input prompts are fully accessible:

- Prompts are announced when they appear
- Buttons have explicit labels
- Form inputs have associated labels
- Focus is automatically managed
- Alternative input modes are keyboard accessible

### Example Accessible Prompt

Rich input prompts are triggered via the `read()` builtin with metadata. The `tts_prompt` metadata key provides an
accessible alternative to visually complex prompts:

```moo
// Visual prompt shows formatted SQL, screen readers hear a clear question
read(player, [
    "input_type" -> "yes_no_alternative",
    "prompt" -> "Apply this change?\n```sql\nALTER TABLE users ADD COLUMN verified BOOLEAN;\n```",
    "tts_prompt" -> "Apply database change to add verified column to users table?"
]);
```

## Visual Accessibility

### Color and Contrast

- Light and dark themes are designed with readable contrast
- Color is not the only indicator of meaning
- Links are underlined in addition to being colored

### Font Options

Users can customize fonts for better readability:

- Serif, sans-serif, or monospace fonts
- Adjustable font size (applies to narrative area)

### Reduced Motion

The web client respects the `prefers-reduced-motion` media query:

- Animations are minimized or disabled
- Transitions become instant
- Loading spinners become static indicators

## Content Authoring Guidelines

When creating MOO content for accessibility:

### DO

- Provide `tts_text` for tables, complex formatting, ASCII art
- Use semantic markup (headings, lists) in Djot and HTML
- Write descriptive link text ("View inventory" not "Click here")

### DON'T

- Rely on color alone to convey meaning
- Use tables for layout (only for tabular data)
- Put essential information only in images
- Create rapidly flashing content

## Related Docs

- [Client Output and Presentations](./client-output-and-presentations.md)
- [Networking](../the-moo-programming-language/networking.md)
- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
