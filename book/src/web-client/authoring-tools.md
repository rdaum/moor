# Authoring and Programming Tools

The web client includes built-in tools for building and editing in-MOO content that mimic the capabilities of a modern
IDE.

These tools use `moor-web-host` endpoints to read and write objects, verbs, and properties. Availability can be
restricted based on permissions and server configuration.

## Access and Permissions

The web client uses the programmer flag from the login/session payload to decide whether to show authoring features in
the UI. If the current player does not have the programmer flag, the object browser and eval panel controls are hidden
and server-driven object browser presentations are dismissed.

All requests still pass through `moor-web-host`, and the daemon enforces permissions on the underlying operations, so
clients cannot bypass server-side access checks.

## Object Browser

The object browser lets builders inspect objects, browse properties and verbs, and navigate object hierarchies. It is
shown when the logged-in player has the programmer flag.

### Opening the Object Browser

- Click the object browser button in the top navigation bar
- Use `present()` from MOO code to target `object-browser`
- On mobile devices, the browser uses a tabbed layout; on desktop, it shows a three-pane split view

### Features

- **Object Search**: Find objects by name pattern, parent type, or object number
- **Object Details**: View name, parent, owner, location, flags (player, programmer, wizard, fertile, readable)
- **Property List**: Browse all properties with owner, permissions, and values
- **Verb List**: Browse all verbs with owner, permissions, and argument specifications
- **Navigation**: Click on object references to navigate (parent, owner, location)
- **Create/Delete**: Create child objects, add properties, add verbs (permissions permitting)

### Object Flags Display

The browser shows object flags as compact letter codes:

| Flag       | Letter | Meaning                   |
|------------|--------|---------------------------|
| Player     | `u`    | Object is a player        |
| Programmer | `p`    | Has programmer privileges |
| Wizard     | `w`    | Has wizard privileges     |
| Readable   | `r`    | Object is readable        |
| Writable   | `W`    | Object is writable        |
| Fertile    | `f`    | Can create children       |

## Verb Editor

The verb editor is powered by Monaco (the same editor engine as VS Code) and provides a full-featured MOO code editing
experience.

### Features

- **Syntax Highlighting**: MOO-specific highlighting for keywords, builtins, strings, comments
- **Autocompletion**: Suggestions for builtin functions, keywords, and common patterns
- **Error Display**: Compile errors shown inline with line/column markers
- **Compile on Save**: Press Ctrl+S (Cmd+S on Mac) or click Save to compile and save
- **Word Wrap**: Toggle word wrapping for long lines
- **Minimap**: Optional code minimap for navigation (hidden on mobile)
- **Font Size**: Adjustable font size (10-24px)
- **Split Mode**: Dock the editor below the narrative or float as a window

### Verb Metadata

The verb editor also supports editing verb metadata:

| Field           | Description                                                            |
|-----------------|------------------------------------------------------------------------|
| **Names**       | Space-separated verb names (e.g., `look l examine`)                    |
| **Owner**       | Object reference for the verb owner                                    |
| **Permissions** | `r` (readable), `w` (writable), `x` (executable), `d` (debug)          |
| **dobj**        | Direct object spec: `none`, `any`, or `this`                           |
| **prep**        | Preposition: `none`, `any`, or specific like `with/using`, `in/inside` |
| **iobj**        | Indirect object spec: `none`, `any`, or `this`                         |

### Navigation

When multiple verbs are open for editing, use the navigation arrows in the title bar to switch between them.

### Keyboard Shortcuts

| Shortcut     | Action              |
|--------------|---------------------|
| Ctrl+S       | Save and compile    |
| Ctrl+/       | Toggle line comment |
| Ctrl+D       | Duplicate line      |
| Ctrl+Shift+K | Delete line         |
| F5           | Toggle word wrap    |

## Property Editor

The property editor provides a Monaco-based editor for editing property values as text. It supports multiple content
types for different editing experiences.

### Content Types

| Type            | Editor Mode                       |
|-----------------|-----------------------------------|
| `text/plain`    | Plain text editing                |
| `text/html`     | HTML with syntax highlighting     |
| `text/markdown` | Markdown with syntax highlighting |

### Features

- Same font size controls as the verb editor
- Split mode support (dock or float)
- Save via REST API or WebSocket command

## Property Value Editor

For structured property values (lists, maps, objects), the property value editor provides a form-based interface instead
of raw text editing.

### Supported Value Types

- **Strings**: Text input
- **Numbers**: Numeric input with type detection (integer vs float)
- **Objects**: Object reference input with validation
- **Lists**: Add/remove list elements, reorder items
- **Maps**: Key-value pair editing

## Text Editor

For long-form text content like descriptions, help text, or notes, the text editor provides a dedicated editing surface
with support for plain text and Djot markup.

The text editor can be opened:

- From the object browser by clicking on a text property
- Via `present()` targeting `text-editor`

## Eval Panel

The eval panel lets programmers evaluate MOO expressions or statements in-place and see the result immediately. It runs
through the server eval endpoint (the same underlying capability as the `eval()` builtin).

### Usage

1. Open the eval panel from the object browser or via keyboard shortcut
2. Enter a full MOO statement. Note that unlike `eval` inside a typical MOO core, you should include the
   `return` keyword and trailing semicolons. Your statement should evaluate to a single value.
3. Press Enter or click Evaluate
4. See the result displayed below

The eval panel is only shown when the player has the programmer flag.

## Opening Editors from MOO Code

All editors can be triggered from MOO code using `present()`:

```moo
// Open verb editor
present(player, "edit-look", "text/plain", "verb-editor", "",
        {{"object", "#123"}, {"verb", "look"}, {"title", "Edit look"}});

// Open property editor
present(player, "edit-desc", "text/plain", "property-editor", "",
        {{"object", "#123"}, {"property", "description"}});

// Open object browser focused on an object
present(player, "browse-obj", "text/plain", "object-browser", "",
        {{"object", "#123"}});

// Dismiss a presentation
present(player, "edit-look");
```

See [Presentations](./presentations.md) for more details on presentation targets and attributes.

## Related Docs

- [Server Builtins](../the-moo-programming-language/built-in-functions/server.md)
- [Object Properties](../the-database/object-properties.md)
- [Object Verbs](../the-database/object-verbs.md)
- [Presentations](./presentations.md)
