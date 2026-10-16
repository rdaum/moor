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

import { ClientEvent } from "@moor/schema/generated/moor-rpc/client-event";
import { ClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import { TaskSuccessEvent } from "@moor/schema/generated/moor-rpc/task-success-event";
import { Var } from "@moor/schema/generated/moor-var/var";
import { VarInt } from "@moor/schema/generated/moor-var/var-int";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import { Builder } from "flatbuffers";
import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { test, type TestContext } from "node:test";
import { WebSocketServer } from "ws";
import { MoorWebClient } from "../src/moor-service.ts";

const actor = { mooId: "test", playerId: "builder" };
function completed() {
    const b = new Builder();
    const value = Var.createVar(b, VarUnion.VarInt, VarInt.createVarInt(b, 1n));
    TaskSuccessEvent.startTaskSuccessEvent(b);
    TaskSuccessEvent.addTaskId(b, 1n);
    TaskSuccessEvent.addResult(b, value);
    const success = TaskSuccessEvent.endTaskSuccessEvent(b);
    b.finish(ClientEvent.createClientEvent(b, ClientEventUnion.TaskSuccessEvent, success, 0n));
    return b.asUint8Array();
}

async function host(t: TestContext, timeoutMs = 1000) {
    let logins = 0;
    let writes = 0;
    const commands: string[] = [];
    const http = createServer((req, res) => {
        req.resume();
        if (req.url === "/auth/connect") {
            logins++;
            setTimeout(() => {
                res.setHeader("X-Moor-Auth-Token", `token-${logins}`);
                res.end();
            }, 10);
        } else if (req.url === "/expired") {
            writes++;
            res.writeHead(401).end("Expired token");
        } else if (req.url === "/rejected") {
            res.end(JSON.stringify({ result: { Failure: { error: "Permission denied" } } }));
        } else if (req.url === "/hang") {
            // The client must abort this request.
        } else res.end(JSON.stringify({ ok: true }));
    });
    const ws = new WebSocketServer({ server: http });
    ws.on("connection", socket => {
        socket.on("message", raw => {
            const command = raw.toString();
            commands.push(command);
            if (command === "hang") return;
            if (command === "disconnect") return socket.close();
            setTimeout(() => {
                if (socket.readyState === socket.OPEN) socket.send(completed());
            }, command === "slow" ? 80 : 5);
        });
    });
    http.listen(0, "127.0.0.1");
    await once(http, "listening");
    const address = http.address();
    assert.ok(address && typeof address !== "string");
    const client = new MoorWebClient({
        timeoutMs,
        moos: [{
            id: "test",
            description: "Test",
            connectAddress: `http://127.0.0.1:${address.port}`,
            players: [{ id: "builder", username: "Builder", password: "test" }],
        }],
    });
    t.after(async () => {
        await client.close();
        for (const socket of ws.clients) socket.terminate();
        ws.close();
        http.closeAllConnections();
        await new Promise<void>(resolve => http.close(() => resolve()));
    });
    return { client, commands, logins: () => logins, writes: () => writes };
}

test("concurrent requests share authentication and expired mutations are never replayed", async t => {
    const h = await host(t);
    await Promise.all([h.client.requestJson(actor, "/read"), h.client.requestJson(actor, "/read")]);
    assert.equal(h.logins(), 1);
    await assert.rejects(h.client.requestJson(actor, "/expired", { method: "POST" }), /401/);
    assert.equal(h.writes(), 1);
    await h.client.requestJson(actor, "/read");
    assert.equal(h.logins(), 2);
    await assert.rejects(h.client.requestJson(actor, "/rejected"), /Permission denied/);
});

test("HTTP requests have a deadline and propagate cancellation", async t => {
    const { client } = await host(t, 100);
    await assert.rejects(client.requestJson(actor, "/hang"), { name: "TimeoutError" });
    const cancellation = new AbortController();
    const pending = client.requestJson(actor, "/hang", undefined, cancellation.signal);
    cancellation.abort();
    await assert.rejects(pending, { name: "AbortError" });
});

test("commands serialize and wait for task completion", async t => {
    const h = await host(t);
    const slow = h.client.command(actor, "slow");
    const next = h.client.command(actor, "next");
    await new Promise(resolve => setTimeout(resolve, 40));
    assert.deepEqual(h.commands, ["slow"]);
    assert.equal((await slow).error, false);
    assert.equal((await next).error, false);
    assert.deepEqual(h.commands, ["slow", "next"]);
});

test("command timeout and disconnect report unknown outcomes and permit a fresh connection", async t => {
    const h = await host(t, 100);
    const timeout = await h.client.command(actor, "hang");
    assert.equal(timeout.error, true);
    assert.match(timeout.lines.join(), /timed out.*unknown/);
    assert.equal((await h.client.command(actor, "next")).error, false);
    const disconnected = await h.client.command(actor, "disconnect");
    assert.equal(disconnected.error, true);
    assert.match(disconnected.lines.join(), /closed.*unknown/);
    assert.equal((await h.client.command(actor, "next")).error, false);
    assert.equal(h.commands.filter(command => command === "hang").length, 1);
});
