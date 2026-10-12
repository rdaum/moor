// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//

import react from "@vitejs/plugin-react";
import { execSync } from "child_process";
import { hostname } from "node:os";
import { resolve } from "path";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

// Get git commit hash at build time
const getGitHash = () => {
    try {
        return execSync("git rev-parse --short HEAD").toString().trim();
    } catch (e) {
        return "unknown";
    }
};

export default defineConfig({
    plugins: [react(), wasm()],
    root: "src",
    publicDir: "../public",
    build: {
        target: "esnext",
        outDir: "../dist",
        emptyOutDir: true,
        sourcemap: process.env.MEADOW_SOURCEMAPS === "true",
        manifest: true,
        rolldownOptions: {
            input: {
                main: resolve(import.meta.dirname, "src/index.html"),
            },
        },
    },
    worker: {
        format: "es",
    },
    define: {
        // Monaco Editor requires this to be defined
        global: "globalThis",
        // Inject git hash as a compile-time constant
        "__GIT_HASH__": JSON.stringify(getGitHash()),
    },
    optimizeDeps: {
        include: ["monaco-editor/editor"],
        exclude: ["@moor/schema"],
    },
    resolve: {
        alias: {
            "@": resolve(import.meta.dirname, "./src"),
            "@/components": resolve(import.meta.dirname, "./src/components"),
        },
    },
    server: {
        port: 3000,
        allowedHosts: [hostname()],
        proxy: {
            "/v1": process.env.MOOR_API_URL || "http://localhost:8080",
            "/ws": {
                target: process.env.MOOR_WS_URL || "ws://localhost:8080",
                ws: true,
                configure(proxy) {
                    proxy.on("proxyReqWs", (request, incoming) => {
                        const value = new URL(incoming.url ?? "/", "http://localhost").searchParams.get("attempt");
                        // Log only the bounded correlation ID, never headers or credentials.
                        const attempt = value && /^[0-9a-f-]{36}$/i.test(value) ? value : undefined;
                        const startedAt = performance.now();
                        const log = (phase: string, status?: number) => {
                            const elapsedMs = Math.round(performance.now() - startedAt);
                            const noteworthy = phase === "error" || phase === "rejected"
                                || (phase === "upgraded" && elapsedMs >= 1000);
                            if (!noteworthy && process.env.VITE_WS_DEBUG !== "true") return;
                            const write = noteworthy ? console.warn : console.debug;
                            write(
                                "[WebSocket proxy] handshake",
                                JSON.stringify({
                                    attempt,
                                    phase,
                                    at: new Date().toISOString(),
                                    elapsedMs,
                                    ...(status === undefined ? {} : { status }),
                                }),
                            );
                        };
                        log("forwarding");
                        request.once("socket", socket => {
                            socket.once("lookup", () => log("upstream_dns_finished"));
                            socket.once("connect", () => log("upstream_connected"));
                        });
                        request.once("upgrade", response => log("upgraded", response.statusCode));
                        request.once("response", response => log("rejected", response.statusCode));
                        request.once("error", () => log("error"));
                    });
                },
            },
            "/health": process.env.MOOR_API_URL || "http://localhost:8080",
            "/version": process.env.MOOR_API_URL || "http://localhost:8080",
            "/auth": process.env.MOOR_API_URL || "http://localhost:8080",
            "/webhooks": process.env.MOOR_API_URL || "http://localhost:8080",
        },
    },
});
