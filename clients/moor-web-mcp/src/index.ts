// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

import readline from "node:readline";

import { loadConfig, resolveDefaultCharacter } from "./config.js";
import { McpServer } from "./mcp-server.js";
import { MoorWebClient } from "./moor-service.js";

function parseArgs(argv: string[]): { configPath: string } {
    let configPath = "";
    for (let i = 0; i < argv.length; i++) {
        if ((argv[i] === "--config" || argv[i] === "-c") && argv[i + 1]) {
            configPath = argv[i + 1];
            i++;
        }
    }
    if (!configPath) {
        throw new Error("Usage: moor-web-mcp --config <config.json|config.yaml>");
    }
    return { configPath };
}

async function main(): Promise<void> {
    const args = parseArgs(process.argv.slice(2));
    const config = loadConfig(args.configPath);
    const defaultCharacterId = resolveDefaultCharacter(config);
    const moor = new MoorWebClient(config);
    const server = new McpServer(moor, config.characters, defaultCharacterId);

    const rl = readline.createInterface({
        input: process.stdin,
        crlfDelay: Infinity,
    });

    rl.on("line", async (line) => {
        await server.handleLine(line, (outLine) => {
            process.stdout.write(`${outLine}\n`);
        });
    });

    const shutdown = async () => {
        rl.close();
        await moor.close();
        process.exit(0);
    };

    process.on("SIGINT", () => {
        void shutdown();
    });
    process.on("SIGTERM", () => {
        void shutdown();
    });

    process.on("uncaughtException", (error: unknown) => {
        const formatted = error instanceof Error ? (error.stack ?? error.message) : String(error);
        process.stderr.write(`uncaughtException: ${formatted}\n`);
    });
    process.on("unhandledRejection", (reason: unknown) => {
        process.stderr.write(`unhandledRejection: ${String(reason)}\n`);
    });
}

void main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
    process.exit(1);
});
