import assert from "node:assert/strict";
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { once } from "node:events";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createInterface } from "node:readline";
import { ChangesFixture, workspace } from "../../../../clients/meadow/e2e/changes-fixture.ts";
import { MoorWebClient } from "../../../../clients/moor-web-mcp/src/moor-service.ts";

// Tests inspect the wire format as returned by the real MCP process.
type Json = any;
class Mcp {
    private sequence = 0;
    private pending = new Map<number, { resolve: (value: Json) => void; reject: (error: Error) => void }>();
    private child: ChildProcessWithoutNullStreams;
    private log = "";
    constructor(binary: string, args: string[], cwd: string) {
        this.child = spawn(binary, args, { cwd, stdio: "pipe" });
        this.child.stderr.on("data", chunk => this.log = (this.log + chunk).slice(-16000));
        createInterface({ input: this.child.stdout }).on("line", line => {
            const value = JSON.parse(line);
            if (value.id !== undefined) this.pending.get(value.id)?.resolve(value);
        });
        this.child.on("exit", code => {
            for (const request of this.pending.values()) request.reject(new Error(`MCP exited ${code}: ${this.log}`));
        });
    }
    async rpc(method: string, params: Json = {}) {
        const id = ++this.sequence;
        let timer: NodeJS.Timeout;
        try {
            const response = await new Promise<Json>((resolve, reject) => {
                this.pending.set(id, { resolve, reject });
                timer = setTimeout(() => reject(new Error(`MCP timeout: ${method}\n${this.log}`)), 90000);
                this.child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
            });
            assert.equal(response.error, undefined, JSON.stringify(response));
            return response.result;
        } finally {
            clearTimeout(timer!);
            this.pending.delete(id);
        }
    }
    async call(name: string, args: Json = {}, error?: string) {
        const response = await this.rpc("tools/call", { name: `changes_${name}`, arguments: args });
        const content = response.structuredContent;
        assert.ok(content, JSON.stringify(response));
        assert.deepEqual(JSON.parse(response.content[0].text), content);
        if (error) {
            assert.equal(response.isError, true, JSON.stringify(content));
            assert.equal(content.error.code, error, JSON.stringify(content));
            return content.error;
        }
        assert.equal(content.ok, true, JSON.stringify(content));
        assert.notEqual(response.isError, true);
        return content.result;
    }
    async close() {
        if (this.child.exitCode !== null || this.child.signalCode !== null) return;
        const closed = once(this.child, "exit");
        this.child.kill("SIGTERM");
        const timer = setTimeout(() => this.child.kill("SIGKILL"), 5000);
        try {
            await closed;
        } finally {
            clearTimeout(timer);
        }
    }
}

async function run(host: "native" | "web", transport: "http" | "git") {
    const fixture = new ChangesFixture();
    let client: Mcp | undefined;
    let assertions: MoorWebClient | undefined;
    try {
        await fixture.start({ distributed: true, browser: false });
        const connection = {
            defaultMoo: "test",
            defaultCharacter: "wizard",
            moos: [{
                id: "test",
                description: "Isolated acceptance world",
                connectAddress: `http://127.0.0.1:${fixture.apiPort}`,
                players: [{
                    id: "wizard",
                    username: "ArchWizard",
                    password: "test",
                    isWizard: true,
                }],
            }],
        };
        assertions = new MoorWebClient(connection);
        const actor = { mooId: "test", playerId: "wizard" };
        if (host === "native") {
            client = new Mcp(resolve(workspace, "target/debug/moor-mcp-host"), [
                ...fixture.rpcArgs,
                "--username",
                "ArchWizard",
                "--password",
                "test",
                "--data-dir",
                resolve(fixture.directory, "mcp-identity"),
            ], fixture.directory);
        } else {
            const config = resolve(fixture.directory, "mcp.json");
            await writeFile(config, JSON.stringify(connection));
            client = new Mcp(process.execPath, [
                "--import",
                "tsx",
                "clients/moor-web-mcp/src/index.ts",
                "--config",
                config,
            ], workspace);
        }
        await client.rpc("initialize", {
            protocolVersion: "2024-11-05",
            capabilities: {},
            clientInfo: { name: "acceptance", version: "1" },
        });
        const catalog = await client.rpc("tools/list");
        assert.equal(catalog.tools.filter((tool: Json) => tool.name.startsWith("changes_")).length, 25);
        const resources = await client.rpc("resources/list");
        assert.ok(resources.resources.some((resource: Json) => resource.uri === "moor://changes-guide"));
        const guide = await client.rpc("resources/read", { uri: "moor://changes-guide" });
        assert.match(guide.contents[0].text, /Do not bypass/);
        assert.equal((await client.call("capabilities")).actual_wizard, true);
        await client.call("status", { review: "9007199254740993" }, "missing_review");
        await client.call("packages", { actor: "#2" }, "invalid_request");
        const upstream = transport === "http"
            ? { transport, url: fixture.httpUrl }
            : { transport, repository: fixture.gitUrl, revision: { ref: "refs/heads/main" }, path: "cores/sample/src" };
        await client.call("configure_package", {
            package: "acceptance",
            objects: ["#9500", "#9501"],
            constants: {},
            upstream,
        });
        const started = await client.call("check", { package: "acceptance" });
        const review = started.review_id;
        async function settled() {
            for (let i = 0; i < 200; i++) {
                const status = await client!.call("status", { review });
                if (!["fetching", "comparing", "applying"].includes(status.status)) return status;
                await new Promise(done => setTimeout(done, 50));
            }
            throw new Error("Review did not settle");
        }
        let status = await settled();
        assert.equal(status.status, "ready", JSON.stringify(status));
        await client.call("check", { package: "acceptance" }, "active_review");
        let generation = status.generation;
        const page = await client.call("review", { review, generation });
        const before = await assertions.evalExpression(
            actor,
            "return {#9500:upstream(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
        );
        assert.deepEqual(before.js, [1, 3, 3, 3]);
        const find = (name: string): Json => page.rows.find((row: Json) => JSON.stringify(row).includes(`:${name}`));
        for (const [name, choice] of [["conflict", "local"], ["deferred", "defer"]]) {
            const result = await client.call("resolve", { review, generation, member: find(name).id, choice });
            generation = result.generation;
        }
        const invalid = await client.call("resolve", {
            review,
            generation,
            member: find("edited").id,
            choice: "edited",
            program: "return (;",
        });
        generation = invalid.generation;
        assert.equal(invalid.validation[0].valid, false);
        const program = "const value = \"#9500\";\nreturn {value, 42};";
        const saved = await client.call("resolve", {
            review,
            generation,
            member: find("edited").id,
            choice: "edited",
            program,
        });
        generation = saved.generation;
        assert.equal(saved.validation[0].valid, true);
        const details = await client.call("details", { review, generation, member: find("edited").id });
        assert.ok(JSON.stringify(details).includes("42"));
        await client.call(
            "resolve",
            { review, generation: 1, member: find("edited").id, choice: "incoming" },
            "stale_generation",
        );
        const prepared = await client.call("prepare_apply", { review, generation });
        assert.equal(prepared.can_apply, true, JSON.stringify(prepared));
        await client.call("apply", { review, generation, prepared: prepared.prepared });
        status = await settled();
        assert.ok(["partial", "complete"].includes(status.status), JSON.stringify(status));
        const duplicate = await client.call("apply", { review, generation, prepared: prepared.prepared });
        assert.equal(duplicate.generation, status.generation);
        const after = await assertions.evalExpression(
            actor,
            "return {#9500:upstream(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
        );
        assert.deepEqual(after.js, [2, 3, ["#9500", 42], 3]);
        const result = await client.call("result", { review, generation: status.generation });
        assert.ok(result.total > 0 || result.decision_count > 0, JSON.stringify(result));
        if (status.status === "partial") {
            status = await client.call("finish", { review, generation: status.generation });
        }
        const history = await client.call("history", { package: "acceptance" });
        assert.equal(history.reviews.length, 1);
        if (host === "native" && transport === "http") {
            // A write commits at suspend, then its reply arrives after the host timeout.
            // The host must reconnect without executing the unknown mutation again.
            const probeCode = [
                "#0.mcp_probe_count = #0.mcp_probe_count + 1;",
                "suspend(61);",
                "return [\"ok\" -> true];",
            ];
            const catalogLine =
                "tools = {@tools, [\"name\" -> \"changes_reply_probe\", \"description\" -> \"Acceptance timeout fixture\", \"input_schema\" -> [\"type\" -> \"object\"], \"target_obj\" -> #0, \"target_verb\" -> \"mcp_probe\"]};";
            await assertions.evalExpression(
                actor,
                `
                add_property(#0, "mcp_probe_count", 0, {player, ""});
                add_verb(#0, {player, "xd", "mcp_probe"}, {"this", "none", "this"});
                set_verb_code(#0, "mcp_probe", {${probeCode.map(line => JSON.stringify(line)).join(",")}});
                const catalog = verb_code(#0, "external_agent_tools");
                return set_verb_code(#0, "external_agent_tools", {@catalog[1..$ - 1], ${
                    JSON.stringify(catalogLine)
                }, "return tools;"});
            `,
            );
            await client.rpc("tools/call", { name: "moo_refresh_dynamic_tools", arguments: {} });
            console.log("Checking lost reply without automatic mutation retry (60 seconds).");
            await client.call("reply_probe", {}, "outcome_unknown");
            const count = await assertions.evalExpression(actor, "return #0.mcp_probe_count;");
            assert.equal(count.js, 1);
            await client.call("packages");
        }
        console.log(`PASS ${host} MCP / ${transport} upstream`);
    } finally {
        await assertions?.close();
        await client?.close();
        await fixture.close();
    }
}
for (const host of (process.argv.includes("--web-only") ? ["web"] : ["native", "web"]) as ("native" | "web")[]) {
    for (const transport of ["http", "git"] as const) await run(host, transport);
}
