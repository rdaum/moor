# Accessibility

The web client is designed to work well with standard browser accessibility tools, and it supports additional metadata to
improve narration for screen readers.

## Screen Reader Narration

When the server emits narrative events with metadata, the web client recognizes:

- `tts_text`: Alternate narration text for output lines (used instead of richly formatted output).
- `tts_prompt`: Alternate prompt text for rich input prompts.

This lets you provide clear spoken text even when the visual presentation is highly formatted.

## Examples

```moo
// Use a short narration for complex HTML output in logged history
metadata = ["tts_text" -> "Gate status: open"];
event_log(player, "<div class='badge'><strong>Gate</strong><span>Open</span></div>", 'text_html, metadata);

// Provide a simpler spoken prompt for rich input
metadata = ["tts_prompt" -> "Name your ship:"];
event_log(player, "<p><strong>Name your ship</strong></p>", 'text_html, metadata);
```

In most cores, helper verbs pair `notify()` with `event_log()` so the same metadata applies to both live output and
history.

## Practical Guidance

- Prefer short, descriptive `tts_text` values for complex HTML or djot output.
- Use `tts_prompt` when presenting rich input prompts that would otherwise be read poorly.

Related docs:

- [Networking](../the-moo-programming-language/networking.md)
- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
