// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import { Client } from "@modelcontextprotocol/client";
import { InMemoryTransport } from "@modelcontextprotocol/server";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { type CharacterRef, loadConfig, type MoorWebMcpConfig } from "../src/config.ts";
import { McpServer } from "../src/mcp-server.ts";
import { type DynamicTool, MoorWebClient } from "../src/moor-service.ts";
import { resourceUri } from "../src/tool-catalog.ts";

const config: MoorWebMcpConfig = {
    moos: ["a", "b"].map(id => ({
        id,
        description: id,
        connectAddress: "http://unused",
        players: [{ id: "builder", username: id, password: `secret-${id}`, isProgrammer: true }],
    })),
};
const actor = { mooId: "a", playerId: "builder" };

class World extends MoorWebClient {
    calls: { character: CharacterRef; target: string; args: unknown }[] = [];
    fetches: string[] = [];
    generation = 0;
    fail = false;
    override async refreshDynamicTools(character: CharacterRef) {
        this.fetches.push(character.mooId);
        if (this.fail) throw new Error("World unavailable");
        return [{
            name: "inspect",
            description: "Inspect",
            inputSchema: { type: "object", properties: {} },
            targetObj: character.mooId === "a" ? "#10" : `#${20 + this.generation}`,
            targetVerb: "inspect",
        }];
    }
    override async refreshDynamicResources(character: CharacterRef) {
        return [{
            uri: "moor://guide",
            name: "Guide",
            description: "Guide",
            mimeType: "text/plain",
            content: character.mooId,
        }];
    }
    override async executeDynamicTool(character: CharacterRef, tool: DynamicTool, args: unknown) {
        this.calls.push({ character, target: tool.targetObj, args });
        return { js: { ok: true }, literal: "" };
    }
    override async evalExpression(): Promise<never> {
        throw new Error("E_PERM: permission denied");
    }
}

async function bridge(t: TestContext) {
    const world = new World(config);
    const server = new McpServer(world, config.moos, actor);
    const client = new Client({ name: "test", version: "1" });
    const [a, b] = InMemoryTransport.createLinkedPair();
    await server.connect(a);
    await client.connect(b);
    t.after(async () => {
        await client.close();
        await server.close();
        await world.close();
    });
    return { world, client };
}

test("catalog dispatch works before listing and stays scoped across refreshes", async t => {
    const { world, client } = await bridge(t);
    await client.callTool({ name: "inspect", arguments: { moo: "b" } });
    await client.callTool({ name: "inspect", arguments: {} });
    assert.deepEqual(world.calls.map(call => call.target), ["#20", "#10"]);
    const list = await client.listTools();
    assert.equal(list.tools.filter(tool => tool.name === "inspect").length, 1);
    world.generation = 1;
    await client.callTool({ name: "moo_refresh_dynamic_tools", arguments: { moo: "b" } });
    await client.callTool({ name: "inspect", arguments: { moo: "b" } });
    await client.callTool({ name: "inspect", arguments: {} });
    assert.deepEqual(world.calls.map(call => call.target), ["#20", "#10", "#21", "#10"]);
    assert.deepEqual(world.fetches, ["b", "a", "b"]);
    assert.deepEqual(world.calls[0].args, {});
});

test("resource metadata omits passwords and guides keep their world identity", async t => {
    const { client } = await bridge(t);
    const metadata = await client.readResource({ uri: "moor://moos" });
    assert.doesNotMatch(JSON.stringify(metadata), /secret-|password/);
    const resources = await client.listResources();
    const uri = resourceUri({ mooId: "b", playerId: "builder" }, "moor://guide");
    assert.ok(resources.resources.some(resource => resource.uri === uri));
    const scoped = (await client.readResource({ uri })).contents[0];
    const fallback = (await client.readResource({ uri: "moor://guide" })).contents[0];
    assert.ok("text" in scoped && "text" in fallback);
    assert.equal(scoped.text, "b");
    assert.equal(fallback.text, "a");
});

test("execution and validation failures are tool results; unknown tools are protocol errors", async t => {
    const { world, client } = await bridge(t);
    const denied = await client.callTool({ name: "moo_eval", arguments: { expression: "1" } });
    assert.equal(denied.isError, true);
    assert.match(JSON.stringify(denied.content), /E_PERM/);
    assert.equal((await client.callTool({ name: "moo_eval", arguments: { expression: false } })).isError, true);
    assert.equal((await client.callTool({ name: "inspect", arguments: { wizard: true } })).isError, true);
    assert.equal(world.calls.length, 0);
    await assert.rejects(client.callTool({ name: "absent" }), error => (error as { code: number }).code === -32602);
    world.fail = true;
    assert.equal((await client.callTool({ name: "moo_refresh_dynamic_tools", arguments: { moo: "b" } })).isError, true);
    world.fail = false;
    assert.notEqual((await client.callTool({ name: "inspect", arguments: { moo: "b" } })).isError, true);
});

test("configuration rejects invalid selections and never quotes malformed credential JSON", t => {
    const dir = mkdtempSync(join(tmpdir(), "moor-mcp-config-"));
    t.after(() => rmSync(dir, { recursive: true, force: true }));
    const path = join(dir, "config.json");
    writeFileSync(path, "{\"password\":\"secret-password\", invalid}");
    assert.throws(() => loadConfig(path), error => {
        assert.doesNotMatch(String(error), /secret-password/);
        return /Invalid JSON/.test(String(error));
    });
    writeFileSync(path, JSON.stringify({ ...config, defaultCharacter: "missing" }));
    assert.throws(() => loadConfig(path), /defaultCharacter/);
    writeFileSync(path, JSON.stringify({ ...config, timeoutMs: 0 }));
    assert.throws(() => loadConfig(path), /timeoutMs/);
    writeFileSync(path, JSON.stringify(config));
    assert.deepEqual(loadConfig(path), config);
});
