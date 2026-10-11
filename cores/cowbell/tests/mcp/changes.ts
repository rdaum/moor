import assert from "node:assert/strict";
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { once } from "node:events";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createInterface } from "node:readline";
import { ChangesFixture, source, workspace } from "../../../../clients/meadow/e2e/changes-fixture.ts";
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
    async rpc(method: string, params: Json = {}, expectedError?: RegExp) {
        const id = ++this.sequence;
        let timer: NodeJS.Timeout;
        try {
            const response = await new Promise<Json>((resolve, reject) => {
                this.pending.set(id, { resolve, reject });
                timer = setTimeout(() => reject(new Error(`MCP timeout: ${method}\n${this.log}`)), 90000);
                this.child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
            });
            if (expectedError) {
                assert.match(response.error?.message ?? "", expectedError);
                return response.error;
            }
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
        await fixture.start({ distributed: true, browser: false, extraMembers: 65, additions: true });
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
        async function connect() {
            if (host === "native") {
                client = new Mcp(resolve(workspace, "target/debug/moor-mcp-host"), [
                    ...fixture.rpcArgs,
                    "--username",
                    "AcceptanceAdmin",
                    "--password",
                    "test",
                    "--data-dir",
                    resolve(fixture.directory, "mcp-identity"),
                ], fixture.directory);
            } else {
                const config = resolve(fixture.directory, "mcp.json");
                await writeFile(
                    config,
                    JSON.stringify({
                        ...connection,
                        moos: [{
                            ...connection.moos[0],
                            players: [{ ...connection.moos[0].players[0], username: "AcceptanceAdmin" }],
                        }],
                    }),
                );
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
            await client.rpc("tools/list");
        }
        await connect();
        assert.ok(client);
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
            trusted_owners: ["#2"],
            upstream,
        });
        const packages = await client.call("packages");
        assert.ok(packages.packages.acceptance);
        async function settled(review: number | string) {
            for (let i = 0; i < 200; i++) {
                const status = await client!.call("status", { review });
                if (!["fetching", "comparing", "applying"].includes(status.status)) return status;
                await new Promise(done => setTimeout(done, 50));
            }
            throw new Error("Review did not settle");
        }
        // Give the Git baseline a known commit, through the same tools an admin uses.
        if (transport === "git") {
            const current = (await client.call("packages")).packages.acceptance;
            await client.call("set_upstream", {
                package: "acceptance",
                generation: current.generation,
                source: { ...upstream, revision: { commit: `sha1:${fixture.revisionA}` } },
            });
            const adoption = await client.call("adopt", { package: "acceptance" });
            const ready = await settled(adoption.review_id);
            const preview = await client.call("prepare_apply", {
                review: ready.review_id,
                generation: ready.generation,
            });
            assert.equal(preview.can_apply, true, JSON.stringify(preview));
            await client.call("apply", {
                review: ready.review_id,
                generation: ready.generation,
                prepared: preview.prepared,
            });
            assert.equal((await settled(ready.review_id)).status, "complete");
            const adopted = (await client.call("packages")).packages.acceptance;
            await client.call("set_upstream", {
                package: "acceptance",
                generation: adopted.generation,
                source: upstream,
            });
        }
        const started = await client.call("check", { package: "acceptance" });
        const review = started.review_id;
        let status = await settled(review);
        assert.equal(status.status, "ready", JSON.stringify(status));
        await client.call("check", { package: "acceptance" }, "active_review");
        let generation = status.generation;
        async function rows() {
            const collected: Json[] = [];
            let cursor: Json[] = [];
            do {
                const page = await client!.call("review", { review, generation, cursor });
                collected.push(...page.rows);
                cursor = page.cursor;
                if (!cursor.length) assert.equal(collected.length, page.total);
            } while (cursor.length);
            assert.ok(collected.length > 50);
            assert.equal(new Set(collected.map(row => row.id)).size, collected.length);
            return collected;
        }
        let members = await rows();
        await client.call("review", { review, generation, cursor: [review, generation + 1, 1] }, "stale_cursor");
        await client.call("review", { review, generation, cursor: [review] }, "stale_cursor");
        await client.call(
            "resolve",
            { review, generation, member: members[0].id, choice: "overwrite-everything" },
            "invalid_request",
        );
        const oversized = { package: "acceptance", sources: [{ label: "oversized.moo", text: " ".repeat(4194305) }] };
        if (host === "web") {
            await client.rpc("tools/call", { name: "changes_check", arguments: oversized }, /413 Payload Too Large/);
        } else {
            await client.call("check", oversized, "limit_exceeded");
        }
        const before = await assertions.evalExpression(
            actor,
            "return {#9500:upstream(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
        );
        assert.deepEqual(before.js, [1, 3, 3, 3]);
        const find = (name: string): Json => members.find((row: Json) => row.names?.includes(name));
        assert.ok(find("extra_65"), JSON.stringify(members));
        const historical = await client.call("base_source", {
            review,
            generation,
            member: find("edited").id,
            ...(transport === "http" ? { sources: [{ label: "base.moo", text: source("base", 65) }] } : {}),
        });
        generation = historical.generation;
        assert.equal(historical.row.base_text_available, true, JSON.stringify(historical));
        assert.ok(JSON.stringify(historical).includes("return 1;"));
        const programs = await client.call("set_group", {
            review,
            generation,
            name: "programs",
            members: members.filter(row =>
                row.field === "program"
                && !["upstream", "added_helper", "deferred", "unchanged", "local"].some(name =>
                    row.names.includes(name)
                )
            ).map(row => row.member),
        });
        generation = programs.generation;
        members = await rows();
        assert.equal(find("conflict").eligible, true, JSON.stringify(find("conflict")));
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
        const initialized = await client.call("initialize_property", {
            review,
            generation,
            member: find("item_schema").id,
        });
        generation = initialized.generation;
        const grouped = await client.call("set_group", {
            review,
            generation,
            name: "helper-and-programs",
            members: members.filter(row =>
                ["upstream", "added_helper", "item_schema"].some(name => row.names?.includes(name))
            )
                .map(row => row.member),
        });
        generation = grouped.generation;
        const selected = await client.call("resolve_group", {
            review,
            generation,
            name: "helper-and-programs",
            choice: "incoming",
        });
        generation = selected.generation;
        const selectedPrograms = await client.call("resolve_group", {
            review,
            generation,
            name: "programs",
            choice: "incoming",
        });
        generation = selectedPrograms.generation;
        const preserved = await client.call("details", { review, generation, member: find("edited").id });
        assert.equal(preserved.row.choice.choice, "edited", JSON.stringify(preserved));
        assert.ok(JSON.stringify(preserved.row.choice).includes("42"));
        const stateBefore = await assertions.evalExpression(
            actor,
            "return {#9500.setting, property_info(#9500, \"setting\"), #9501.setting, property_info(#9501, \"setting\")};",
        );
        await client.close();
        await assertions.close();
        await fixture.restart();
        assertions = new MoorWebClient(connection);
        await connect();
        status = await settled(review);
        assert.equal(status.generation, generation);
        members = await rows();
        const restored = await client.call("details", { review, generation, member: find("edited").id });
        assert.deepEqual(restored.row.choice, preserved.row.choice);
        const prepared = await client.call("prepare_apply", { review, generation });
        assert.equal(prepared.can_apply, true, JSON.stringify(prepared));
        assert.equal(prepared.counts.add_property, 1, JSON.stringify(prepared));
        assert.equal(prepared.counts.add_verb, 1, JSON.stringify(prepared));
        let effects = [...prepared.effects];
        for (let offset = prepared.next; offset;) {
            const more = await client.call("prepare_apply", { review, generation, offset });
            assert.deepEqual(more.prepared, prepared.prepared);
            effects.push(...more.effects);
            offset = more.next;
        }
        assert.equal(effects.length, prepared.total);
        assert.ok(effects.length > 50);
        // Drop the reply at the external client's boundary, then recover through a fresh process.
        await client.rpc("tools/call", {
            name: "changes_apply",
            arguments: { review, generation, prepared: prepared.prepared },
        });
        await client.close();
        await connect();
        status = await settled(review);
        assert.ok(["partial", "complete"].includes(status.status), JSON.stringify(status));
        const duplicate = await client.call("apply", { review, generation, prepared: prepared.prepared });
        assert.equal(duplicate.generation, status.generation);
        const after = await assertions.evalExpression(
            actor,
            "return {#9500:upstream(), #9500:conflict(), #9500:edited(), #9500:deferred()};",
        );
        assert.deepEqual(after.js, [42, 3, ["#9500", 42], 3]);
        const stateAfter = await assertions.evalExpression(
            actor,
            "return {#9500.setting, property_info(#9500, \"setting\"), #9501.setting, property_info(#9501, \"setting\")};",
        );
        assert.deepEqual(stateAfter.js, stateBefore.js);
        const last = await assertions.evalExpression(
            actor,
            "return {#9500:extra_65(), #9500:added_helper(), #9500:local()};",
        );
        assert.deepEqual(last.js, [2, 42, 3]);
        const result = await client.call("result", { review, generation: status.generation });
        let receipt = [...result.decisions];
        for (let offset = result.next_decision; offset;) {
            const more = await client.call("result", { review, generation: status.generation, offset });
            receipt.push(...more.decisions);
            offset = more.next_decision;
        }
        assert.equal(receipt.length, result.decision_count, JSON.stringify(result));
        assert.ok(receipt.length > 50);
        if (status.status === "partial") {
            status = await client.call("finish", { review, generation: status.generation });
        }
        const history = await client.call("history", { package: "acceptance" });
        assert.equal(history.reviews.length, transport === "git" ? 2 : 1);
        await client.close();
        await assertions.close();
        await fixture.restart();
        assertions = new MoorWebClient(connection);
        await connect();
        const durable = await client.call("result", { review, generation: status.generation });
        assert.deepEqual(durable.decisions, result.decisions);
        const next = await client.call("check", { package: "acceptance" });
        const nextStatus = await settled(next.review_id);
        const nextPage = await client.call("review", { review: next.review_id, generation: nextStatus.generation });
        assert.ok(
            nextPage.rows.some((row: Json) => row.names?.includes("deferred") && row.classification === "conflict"),
        );
        await client.call("discard", { review: next.review_id, generation: nextStatus.generation });

        // Revoke the actual authenticated requester while its worker is suspended.
        const gate = transport === "http" ? fixture.holdHttp() : fixture.holdGit();
        const suspended = await client.call("check", { package: "acceptance" });
        await Promise.race([
            gate.arrived,
            new Promise((_, reject) => setTimeout(() => reject(new Error("Worker did not reach source")), 15000)),
        ]);
        await assertions.evalExpression(actor, "#9502.wizard = false; return true;");
        gate.release();
        await client.call("packages", {}, "permission_denied");
        for (let attempt = 0;; attempt++) {
            const running = await assertions.evalExpression(
                actor,
                `return valid_task($change_manager.pending[${suspended.review_id}]["task"]);`,
            );
            if (!running.js) break;
            assert.ok(attempt < 200, "Revoked fetch did not stop");
            await new Promise(done => setTimeout(done, 50));
        }
        assert.equal(
            (await assertions.evalExpression(
                actor,
                `return length($change_manager.pending[${suspended.review_id}]["sources"]);`,
            )).js,
            0,
        );
        await assertions.evalExpression(actor, "#9502.wizard = true; return true;");
        const rejected = await client.call("status", { review: suspended.review_id });
        assert.equal(rejected.status, "failed", JSON.stringify(rejected));
        await client.call("discard", { review: suspended.review_id, generation: rejected.generation });
        // A proposed object has an explicit address; reviewing it must not create it.
        await client.call("configure_package", {
            package: "new-object",
            objects: [],
            constants: {},
            trusted_owners: ["#2"],
        });
        const newPackage = (await client.call("packages")).packages["new-object"];
        await client.call("set_bindings", {
            package: "new-object",
            generation: newPackage.generation,
            bindings: { HELPER: "#9600" },
            complete: true,
        });
        const objectSource = "object HELPER\nowner: #2\nname: \"New helper\"\n"
            + "property counter (owner: #2, flags: \"r\") = 42;\n"
            + "method run owner: #2\nreturn this.counter;\nendmethod\n"
            + "method initialize owner: #2\nthis.counter = 999;\nendmethod\nendobject";
        const objectReview = await client.call("check", {
            package: "new-object",
            sources: [{ label: "helper.moo", text: objectSource }],
        });
        let objectStatus = await settled(objectReview.review_id);
        const objectRows = await client.call("review", {
            review: objectStatus.review_id,
            generation: objectStatus.generation,
        });
        assert.equal((await assertions.evalExpression(actor, "return valid(#9600);")).js, false);
        objectStatus = await client.call("set_group", {
            review: objectStatus.review_id,
            generation: objectStatus.generation,
            name: "new-helper",
            members: objectRows.rows.map((row: Json) => row.member),
        });
        objectStatus = await client.call("resolve_group", {
            review: objectStatus.review_id,
            generation: objectStatus.generation,
            name: "new-helper",
            choice: "incoming",
        });
        const objectPrepared = await client.call("prepare_apply", {
            review: objectStatus.review_id,
            generation: objectStatus.generation,
        });
        assert.equal(objectPrepared.can_apply, true, JSON.stringify(objectPrepared));
        assert.equal(objectPrepared.counts.add_object, 1);
        await client.call("apply", {
            review: objectStatus.review_id,
            generation: objectStatus.generation,
            prepared: objectPrepared.prepared,
        });
        assert.equal((await settled(objectStatus.review_id)).status, "complete");
        assert.equal((await assertions.evalExpression(actor, "return #9600:run();")).js, 42);

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
