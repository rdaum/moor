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

import { serveStdio } from "@modelcontextprotocol/server/stdio";
import { loadConfig, resolveDefaultCharacter } from "./config.js";
import { McpServer } from "./mcp-server.js";
import { MoorWebClient } from "./moor-service.js";

async function main(): Promise<void> {
    const args = process.argv.slice(2);
    if (args.length !== 2 || !["--config", "-c"].includes(args[0])) {
        throw new Error("Usage: moor-web-mcp --config <config.json>");
    }
    const config = loadConfig(args[1]);
    const moor = new MoorWebClient(config);
    const handle = serveStdio(() => {
        const server = new McpServer(moor, config.moos, resolveDefaultCharacter(config));
        server.onclose = () => {
            void moor.close();
        };
        return server;
    }, { onerror: error => console.error(error.message) });
    const shutdown = async () => {
        await handle.close();
        await moor.close();
    };
    process.once("SIGINT", () => {
        void shutdown();
    });
    process.once("SIGTERM", () => {
        void shutdown();
    });
    process.stdin.once("end", () => {
        void shutdown();
    });
}

void main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
});
