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
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import assert from "node:assert/strict";

/** Real protocol client shared by the core acceptance workflows. */
export class Mcp {
    private readonly client = new Client({ name: "moor-acceptance", version: "1" });
    private readonly transport: StdioClientTransport;
    constructor(command: string, args: string[], cwd: string) {
        this.transport = new StdioClientTransport({ command, args, cwd, stderr: "inherit" });
    }
    async rpc(
        method: "initialize" | "tools/list" | "tools/call" | "resources/list" | "resources/read",
        params: any = {},
        expectedError?: RegExp,
    ): Promise<any> {
        if (method === "initialize") {
            await this.client.connect(this.transport);
            return this.client.getServerVersion();
        }
        const result = await this.client.request({ method, params }, { timeout: 90000 });
        if (expectedError) {
            assert.equal(result.isError, true, JSON.stringify(result));
            assert.match(JSON.stringify(result.content), expectedError);
        }
        return result;
    }
    async call(name: string, args: Record<string, unknown> = {}, error?: string) {
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
        await this.client.close();
    }
}
