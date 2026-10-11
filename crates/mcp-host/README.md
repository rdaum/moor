# moor-mcp-host

TLDR: Model Context Protocol host for exposing mooR runtime operations to MCP clients.

Downstream uses:

- Leaf host crate; no production runtime crate depends on it.
- Uses `moor-runtime-api` and `moor-zmq-client` to talk to a running mooR runtime.
- Keep MCP protocol translation here; shared runtime requests and events belong in
  `moor-runtime-api`.

## Cowbell change management

Cowbell supplies the `changes_*` tools and the `moor://changes-guide` resource. The host discovers
them through the core's external-agent catalog.

Build the host from the repository root:

```sh
cargo build -p moor-mcp-host
```

Configure your MCP client to start `target/debug/moor-mcp-host` over stdio. Use the daemon's RPC and
enrollment addresses and its enrollment token file:

```sh
target/debug/moor-mcp-host \
  --rpc-address tcp://127.0.0.1:7899 \
  --enrollment-address tcp://127.0.0.1:7900 \
  --enrollment-token-file /path/to/enrollment-token \
  --data-dir /path/to/mcp-identity \
  --username AdminName --password PASSWORD
```

Replace the addresses and paths with your daemon settings. The account selected by `--username` is
the default requester. For a separate administrator connection, set `--wizard-username` and
`--wizard-password`, then select `wizard: true` in tool arguments. The world checks the selected
account's actual authority on each operation. Room ownership and tool visibility do not grant
permission. Git access and new object creation require an actual wizard.

Read `moor://changes-guide` before an update. A request to find updates permits inspection and
review preparation. A request to merge updates permits guarded application within the requested
scope. Local conflicts and property initialization can still require a policy decision.

Start with `changes_packages`, then `changes_check` and `changes_status`. Read review pages and save
decisions through `changes_resolve`. Use `changes_prepare_apply` to inspect all effects before
application. Pass its `prepared` handle unchanged to `changes_apply`. After a lost reply, read
status and retained results before another write. Use `changes_finish` to close a partial review
without accepting deferred changes.

Results provide structured JSON and the same JSON as text. Preserve returned generations, cursors,
and decimal-string identifiers exactly. The host does not automatically repeat a tool mutation after
an unknown outcome. Refresh dynamic tools with `moo_refresh_dynamic_tools` after a core update.

See the
[administrator walkthrough](../../book/src/the-system/object-loading.md#review-updates-with-an-agent)
for decisions, groups, history repair, and recovery. The
[acceptance instructions](../../cores/cowbell/RUNTIME_TESTING.md#change-tools-and-agent-workflows)
run both MCP hosts against isolated HTTP and Git sources.
