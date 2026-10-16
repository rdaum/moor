# moor-web-mcp

Web-host backed MCP stdio server for mooR.

## Configuration

Create a JSON config file:

```json
{
  "timeoutMs": 30000,
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
npm run web:prepare
npm run web-mcp:build
node clients/moor-web-mcp/dist/index.js --config /path/to/moor-web-mcp.json
```

## Cowbell change management

Cowbell exposes its `changes_*` tool catalog through this host. Select an authorized administrator
character before applying changes. The `isWizard` setting helps select a configured account; the
world checks its actual permissions.

Read the `moor://changes-guide` resource for the review workflow. Dynamic tool results contain
`structuredContent` and the same JSON in text. Preserve returned review generations, cursors, and
prepared handles. After a lost reply, read status before repeating a write.

Tool and resource catalogs are cached per world and account. Calls select their catalog through
`moo` and `character`; omitted values use the configured defaults. `wizard: true` requires a
configured wizard account. Refresh the selected catalog with `moo_refresh_dynamic_tools` after
changing the core. Resource lists include world and account URIs. Short guide URIs refer to the
default account. See the
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
run web MCP against isolated HTTP and Git sources. The tests include shared LLM tools, saved drafts,
additions, restart, permission loss, and recovery after a lost reply. They require no paid model
access.

## Errors and deadlines

The bridge uses the MCP SDK for stdio transport, initialization, and cancellation. Tool execution
and argument errors return `isError: true`. Unknown tools return a protocol error.

HTTP requests and commands use `timeoutMs`, which defaults to 30 seconds. Commands on one account
run in order and wait for task completion. A timeout, cancellation, or disconnection can leave an
unknown outcome. The bridge closes that command connection and does not repeat the operation. Check
world state before retrying a write. Commands that request interactive input need a MOO client.

The `moor://moos` resource exposes account metadata without passwords. Protect the configuration
file: it contains the credentials used for every operation.

## Tests

Run from the repository root:

```bash
npm run test:mcp
npm run test:mcp:integration
```

The fast suite uses an MCP client, local HTTP and WebSocket servers, and isolated catalog fixtures.
It covers account routing, resource isolation, errors, authentication, cancellation, and command
completion. CI runs this suite.

The integration command builds the bridge and required Rust services. It starts fresh Cowbell worlds
and drives the built stdio executable through the MCP SDK client. Coverage includes static
programming tools, the shared LLM registry, and change management over HTTP and Git. It uses local
sources and requires no model API key.
