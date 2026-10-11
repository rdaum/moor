# moor-web-mcp

Web-host backed MCP stdio server for mooR.

## Configuration

Create a JSON config file:

```json
{
  "defaultMoo": "local-dev",
  "defaultCharacter": "programmer",
  "moos": [
    {
      "id": "local-dev",
      "description": "Main development MOO.",
      "connectAddress": "http://localhost:8080",
      "players": [
        {
          "id": "programmer",
          "username": "programmer",
          "password": "secret",
          "isProgrammer": true,
          "notes": "Default coding character."
        },
        {
          "id": "wizard",
          "username": "wizard",
          "password": "wizard-secret",
          "isWizard": true,
          "notes": "Elevated operations only."
        }
      ]
    }
  ]
}
```

## Run

```bash
npm run build --prefix clients/moor-web-mcp
node clients/moor-web-mcp/dist/index.js --config /path/to/moor-web-mcp.json
```

## Cowbell change management

Cowbell exposes its `changes_*` tool catalog through this host. Select an authorized administrator
character before applying changes. The `isWizard` setting helps select a configured account; the
world checks its actual permissions.

Read the `moor://changes-guide` resource for the review workflow. Dynamic tool results contain
`structuredContent` and the same JSON in text. Preserve returned review generations, cursors, and
prepared handles. After a lost reply, read status before repeating a write.

The catalog and guide come from the default configured character's world. Refresh the catalog with
`moo_refresh_dynamic_tools` after changing the core. See the
[administrator workflow](../../book/src/the-system/object-loading.md#review-updates-with-an-agent)
for decisions, permissions, and recovery.
