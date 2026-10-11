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

For a dedicated change-management connection, set `defaultCharacter` to the administrator's account
ID. For a separate configured administrator account, select `wizard: true` in tool arguments. Git
access and new object creation require an actual wizard.

A request to find updates permits inspection and review preparation. A request to merge updates
permits guarded application within the requested scope. Local conflicts and property initialization
can still require a policy decision.

Start with `changes_packages`, then `changes_check` and `changes_status`. Read review pages and save
decisions through `changes_resolve`. Use `changes_prepare_apply` to inspect all effects before
application. Pass its `prepared` handle unchanged to `changes_apply`. Use `changes_finish` to close
a partial review without accepting deferred changes.

The
[acceptance instructions](../../cores/cowbell/RUNTIME_TESTING.md#change-tools-and-agent-workflows)
run both MCP hosts against isolated HTTP and Git sources. The tests include saved drafts, additions,
restart, permission loss, and recovery after a lost reply. They require no paid model access.
