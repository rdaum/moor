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
import { resolve } from "path";
import { defineConfig } from "vite";
import topLevelAwait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

export default defineConfig({
    plugins: [react(), wasm(), topLevelAwait()],
    root: "web-client/src",
    publicDir: "../public",
    build: {
        outDir: "../../dist",
        emptyOutDir: true,
        sourcemap: true,
        rollupOptions: {
            input: {
                main: resolve(__dirname, "web-client/src/index.html"),
            },
            external: ["argon2-browser"],
        },
    },
    define: {
        // Monaco Editor requires this to be defined
        global: "globalThis",
    },
    optimizeDeps: {
        include: ["monaco-editor"],
        exclude: ["argon2-browser"],
    },
    resolve: {
        alias: {
            "@": resolve(__dirname, "./web-client/src"),
            "@/components": resolve(__dirname, "./web-client/src/components"),
        },
    },
    server: {
        port: 3000,
        proxy: {
            "/api": process.env.MOOR_API_URL || "http://localhost:8080",
            "/ws": {
                target: process.env.MOOR_WS_URL || "ws://localhost:8080",
                ws: true,
            },
            "/auth": process.env.MOOR_API_URL || "http://localhost:8080",
            "/eval": process.env.MOOR_API_URL || "http://localhost:8080",
            "/verbs": process.env.MOOR_API_URL || "http://localhost:8080",
            "/properties": process.env.MOOR_API_URL || "http://localhost:8080",
            "/objects": process.env.MOOR_API_URL || "http://localhost:8080",
            "/system_property": process.env.MOOR_API_URL || "http://localhost:8080",
            "/fb": process.env.MOOR_API_URL || "http://localhost:8080",
        },
    },
});
