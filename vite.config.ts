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

export default defineConfig({
    plugins: [react()],
    root: "web-client/src",
    publicDir: "../public",
    build: {
        outDir: "../../dist",
        emptyOutDir: true,
        rollupOptions: {
            input: {
                main: resolve(__dirname, "web-client/src/index.html"),
            },
        },
    },
    define: {
        // Monaco Editor requires this to be defined
        global: "globalThis",
    },
    optimizeDeps: {
        include: ["monaco-editor"],
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
            "/api": "http://localhost:8080",
            "/ws": {
                target: "ws://localhost:8080",
                ws: true,
            },
            "/auth": "http://localhost:8080",
            "/eval": "http://localhost:8080",
            "/verbs": "http://localhost:8080",
            "/properties": "http://localhost:8080",
            "/objects": "http://localhost:8080",
            "/system_property": "http://localhost:8080",
        },
    },
});
