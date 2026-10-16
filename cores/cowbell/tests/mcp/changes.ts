import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { ChangesFixture, source, workspace } from "../../../../clients/meadow/e2e/changes-fixture.ts";
import { MoorWebClient } from "../../../../clients/moor-web-mcp/src/moor-service.ts";
import { Mcp } from "../../../../clients/moor-web-mcp/tests/mcp-client.ts";

// Tests inspect the wire format as returned by the real MCP process.
type Json = any;

async function run(transport: "http" | "git") {
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
                "clients/moor-web-mcp/dist/index.js",
                "--config",
                config,
            ], workspace);
            await client.rpc("initialize");
            await client.rpc("tools/list");
        }
        await connect();
        assert.ok(client);
        const catalog = await client.rpc("tools/list");
        assert.equal(catalog.tools.filter((tool: Json) => tool.name.startsWith("changes_")).length, 25);
        if (transport === "http") {
            // Exercise the domain providers moved by the LLM refactor through web MCP.
            const registry = (await assertions.evalExpression(actor, "return #0:external_agent_tools();")).js;
            assert.ok(Array.isArray(registry));
            for (const tool of registry) {
                assert.ok(catalog.tools.some((listed: Json) => listed.name === tool.name), tool.name);
            }
            async function tool(name: string, args: Json = {}, error = false) {
                const response = await client!.rpc("tools/call", { name, arguments: args });
                assert.equal(response.isError ?? false, error, JSON.stringify(response));
                assert.ok(response.structuredContent, JSON.stringify(response));
                assert.deepEqual(JSON.parse(response.content[0].text), response.structuredContent);
                return response.structuredContent;
            }
            await assertions.evalExpression(actor, "#9502.wizard = false; return true;");
            assert.match((await tool("list_prototypes")).value, /\$thing/);
            assert.match(
                (await tool("doc_lookup", { target: "$llm_programming_tools:program_verb" })).value,
                /Compile code/,
            );
            const created = await tool("create_object", { parent: "$thing", name: "MCP tool probe" });
            const object = created.value.match(/\((#[^)]+)\)/)?.[1];
            assert.ok(object, JSON.stringify(created));
            assert.equal((await assertions.evalExpression(actor, `return ${object}.owner == #9502;`)).js, true);
            assert.match((await tool("inspect_object", { object })).value, /MCP tool probe/);
            assert.match((await tool("list_rules", { object })).value, /Rules on/);
            await tool("add_verb", { object, verb: "probe", permissions: "rxd" });
            await tool("program_verb", { object, verb: "probe", code: "const value = \"#123\";\nreturn value;" });
            assert.equal((await assertions.evalExpression(actor, `return ${object}:probe();`)).js, "#123");
            await client.rpc("tools/call", {
                name: "program_verb",
                arguments: { object, verb: "probe", code: "return 0;", actor: "#2" },
            }, /additional properties/);
            await client.rpc("tools/call", {
                name: "program_verb",
                arguments: { object: "#9500", verb: "upstream", code: "return 0;" },
            }, /E_PERM/);
            const failure = await tool("program_verb", { object, verb: "probe", code: "return (;" }, true);
            assert.equal(failure.ok, false);
            assert.equal(failure.code, "compile_error");
            assert.equal((await assertions.evalExpression(actor, `return ${object}:probe();`)).js, "#123");
            async function builtin(name: string, args: Json = {}, error = false) {
                const response = await client!.rpc("tools/call", { name: `moo_${name}`, arguments: args });
                assert.equal(response.isError ?? false, error, JSON.stringify(response));
                return response.content[0].text as string;
            }
            assert.match(await builtin("eval", { expression: "return player;" }), /9502/);
            assert.match(await builtin("resolve", { object: "me" }), /9502/);
            assert.match(await builtin("list_verbs", { object }), /probe/);
            assert.match(await builtin("get_verb", { object, name: "probe" }), /#123/);
            assert.match(await builtin("invoke_verb", { object, verb: "probe", args: [] }), /#123/);
            await builtin("program_verb", { object, name: "probe", code: "return args;" });
            const values = [true, null, "#123", "line one\nline two"];
            const invoked = await builtin("invoke_verb", { object, verb: "probe", args: values });
            assert.deepEqual(JSON.parse(invoked.split("\n\n")[1]), [true, { oid: -1 }, "#123", "line one\nline two"]);
            await builtin("program_verb", { object, name: "probe", code: "return (;" }, true);
            await builtin("set_property", { object, name: "name", valueLiteral: "\"MCP renamed\"" });
            assert.match(await builtin("get_property", { object, name: "name" }), /MCP renamed/);
            assert.match(await builtin("list_properties", { object, inherited: true }), /description/);
            assert.match(await builtin("function_help", { function_name: "tostr" }), /tostr/);
            assert.match(await builtin("command", { command: "look", include_metadata: true }), /events/);
            const metadata = await client.rpc("resources/read", { uri: "moor://moos" });
            assert.doesNotMatch(metadata.contents[0].text, /password/);
            console.log("PASS web MCP / static tools and command completion");
            await tool("recycle_object", { object });
            assert.equal((await assertions.evalExpression(actor, `return valid(${object});`)).js, false);
            await assertions.evalExpression(actor, "#9502.wizard = true; return true;");
            const buildingGuide = await client.rpc("resources/read", { uri: "moor://building-guide" });
            assert.match(buildingGuide.contents[0].text, /list_prototypes/);
            console.log(`PASS web MCP / shared LLM tools (${registry.length} registered)`);
        }
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
        await client.call("check", oversized, "invalid_request");
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

        console.log(`PASS web MCP / ${transport} upstream`);
    } finally {
        await assertions?.close();
        await client?.close();
        await fixture.close();
    }
}
for (const transport of ["http", "git"] as const) await run(transport);
