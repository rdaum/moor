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

import { type ChildProcess, execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { createServer as createTcpServer } from "node:net";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

export const workspace = resolve(import.meta.dirname, "../../..");
export const members = ["unchanged", "upstream", "local", "converged", "conflict", "edited", "deferred", "unbased"];
export function source(version: "base" | "live" | "incoming") {
    const values = members.map(name => {
        if (version === "base") return 1;
        if (version === "live") return ["local", "converged", "conflict", "edited", "deferred"].includes(name) ? 3 : 1;
        return name === "unchanged" || name === "local" ? 1 : name === "converged" ? 3 : 2;
    });
    return `object #9500 [import_export_id -> "changes_acceptance"]\nname: "Acceptance package"\nowner: #2\nparent: #9501\noverride setting (owner: #2, flags: \"\") = 1;\n`
        + members.map((name, i) =>
            `verb ${name} (this none this) owner: #2 flags: "rxd"\nreturn ${values[i]};\nendverb\n`
        ).join("")
        + "endobject\nobject #9501 [import_export_id -> \"acceptance_parent\"]\nowner: #2\nproperty setting (owner: #2, flags: \"rc\") = 1;\nendobject\n";
}
async function port(): Promise<number> {
    const server = createTcpServer();
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const address = server.address();
    if (!address || typeof address === "string") throw new Error("No listener address");
    await new Promise<void>(done => server.close(() => done()));
    return address.port;
}
async function stop(child?: ChildProcess) {
    if (!child || child.exitCode !== null || child.signalCode !== null) return;
    const exited = once(child, "exit");
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 10_000);
    try {
        await exited;
    } finally {
        clearTimeout(timer);
    }
}

export class ChangesFixture {
    directory = "";
    apiPort = 0;
    browserPort = 0;
    gitUrl = "";
    httpUrl = "";
    revisionA = "";
    revisionB = "";
    private server?: Server;
    private world?: ChildProcess;
    private hosts: ChildProcess[] = [];
    private distributed = false;
    rpcArgs: string[] = [];
    telnetPort = 0;
    private vite?: ChildProcess;
    private log = "";
    private bundleGate?: Promise<void>;
    private bundleArrived?: () => void;
    private releaseBundle?: () => void;
    private gitGate?: Promise<void>;
    private gitArrived?: () => void;
    private releaseGit?: () => void;
    private gitChildren = new Set<ChildProcess>();

    async start(options: { distributed?: boolean; browser?: boolean } = {}) {
        this.distributed = options.distributed ?? false;
        this.directory = await mkdtemp(resolve(tmpdir(), "moor-changes-"));
        try {
            this.apiPort = await port();
            this.browserPort = await port();
            const base = resolve(this.directory, "base");
            const live = resolve(this.directory, "live");
            await cp(resolve(workspace, "cores/cowbell/src"), base, { recursive: true });
            const administrator = (await readFile(resolve(base, "arch_wizard.moo"), "utf8"))
                .replaceAll("ARCH_WIZARD", "#9502").replace("\"arch_wizard\"", "\"acceptance_admin\"").replace(
                    "\"ArchWizard\"",
                    "\"AcceptanceAdmin\"",
                );
            await writeFile(resolve(base, "acceptance_admin.moo"), administrator);
            await writeFile(resolve(base, "changes_acceptance.moo"), source("base"));
            await cp(base, live, { recursive: true });
            await writeFile(resolve(live, "changes_acceptance.moo"), source("live"));
            const repo = resolve(this.directory, "repo.git");
            await mkdir(resolve(repo, "cores/sample/src"), { recursive: true });
            const git = (...args: string[]) =>
                execFileSync("git", [
                    "-c",
                    "user.name=Acceptance",
                    "-c",
                    "user.email=acceptance@example.invalid",
                    ...args,
                ], {
                    cwd: repo,
                    encoding: "utf8",
                    env: { ...process.env, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null" },
                }).trim();
            git("init", "-b", "main");
            for (const version of ["base", "incoming"] as const) {
                await writeFile(resolve(repo, "cores/sample/src/probe.moo"), source(version));
                git("add", ".");
                git("commit", "-m", version);
                if (version === "base") this.revisionA = git("rev-parse", "HEAD");
                else this.revisionB = git("rev-parse", "HEAD");
            }
            this.server = createServer(async (request, response) => {
                const url = new URL(request.url!, "http://localhost");
                if (url.pathname === "/bundle.moo") {
                    this.bundleArrived?.();
                    await this.bundleGate;
                    response.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
                    response.end(source("incoming"));
                    return;
                }
                this.gitArrived?.();
                await this.gitGate;
                const input: Buffer[] = [];
                for await (const chunk of request) input.push(Buffer.from(chunk));
                const body = Buffer.concat(input);
                const child = spawn("git", ["http-backend"], {
                    env: {
                        ...process.env,
                        GIT_CONFIG_NOSYSTEM: "1",
                        GIT_CONFIG_GLOBAL: "/dev/null",
                        GIT_PROJECT_ROOT: this.directory,
                        GIT_HTTP_EXPORT_ALL: "1",
                        PATH_INFO: url.pathname,
                        QUERY_STRING: url.search.slice(1),
                        REQUEST_METHOD: request.method,
                        CONTENT_LENGTH: String(body.length),
                        CONTENT_TYPE: request.headers["content-type"] ?? "",
                        HTTP_GIT_PROTOCOL: request.headers["git-protocol"] as string ?? "",
                    },
                });
                this.gitChildren.add(child);
                const chunks: Buffer[] = [];
                child.stdout.on("data", chunk => chunks.push(chunk));
                child.stderr.on("data", chunk => {
                    this.log += chunk;
                });
                child.on("close", code => {
                    this.gitChildren.delete(child);
                    const bytes = Buffer.concat(chunks);
                    const end = bytes.indexOf("\r\n\r\n");
                    if (code !== 0 || end < 0) {
                        response.writeHead(500);
                        response.end();
                        return;
                    }
                    let status = 200;
                    const headers: Record<string, string> = {};
                    for (const line of bytes.subarray(0, end).toString().split("\r\n")) {
                        const split = line.indexOf(":");
                        const key = line.slice(0, split), value = line.slice(split + 1).trim();
                        if (key.toLowerCase() === "status") status = parseInt(value);
                        else headers[key] = value;
                    }
                    response.writeHead(status, headers);
                    response.end(bytes.subarray(end + 4));
                });
                child.stdin.end(body);
            });
            this.server.listen(0, "127.0.0.1");
            await once(this.server, "listening");
            const address = this.server.address();
            if (!address || typeof address === "string") throw new Error("Missing Git address");
            this.gitUrl = `http://127.0.0.1:${address.port}/repo.git`;
            this.httpUrl = `http://127.0.0.1:${address.port}/bundle.moo`;
            const config = (await readFile(resolve(workspace, "moor-dev.yaml"), "utf8"))
                .replace(
                    "input_path: \"cores/cowbell/src\"",
                    `input_path: ${JSON.stringify(live)}\n  baseline_objdef_dir: ${JSON.stringify(base)}`,
                )
                .replace(
                    "output_path: \"development-export\"",
                    `output_path: ${JSON.stringify(resolve(this.directory, "export"))}`,
                )
                .replace("enabled: true\n    address:", "enabled: false\n    address:")
                .replace("listen_address: \"0.0.0.0:8080\"", `listen_address: "127.0.0.1:${this.apiPort}"`);
            await writeFile(
                resolve(this.directory, "config.yaml"),
                this.distributed ? config.split("services:")[0] : config,
            );
            await this.startWorld();
            if (options.browser === false) return;
            this.vite = this.launch(
                process.execPath,
                [
                    resolve(workspace, "node_modules/vite/bin/vite.js"),
                    "--host",
                    "127.0.0.1",
                    "--port",
                    String(this.browserPort),
                    "--strictPort",
                ],
                resolve(workspace, "clients/meadow"),
                { MOOR_API_URL: `http://127.0.0.1:${this.apiPort}` },
            );
            await this.ready(`http://127.0.0.1:${this.browserPort}/health`, this.vite);
        } catch (error) {
            await this.close();
            throw new Error(`${error}\n${this.log}`);
        }
    }
    private launch(binary: string, args: string[], cwd: string, extra: Record<string, string> = {}) {
        const env = { ...process.env, ...extra };
        delete env.MOOR_GIT_UPSTREAM;
        delete env.MOOR_BASELINE_OBJDEF_DIR;
        const child = spawn(binary, args, { cwd, env, stdio: ["ignore", "pipe", "pipe"] });
        child.once("error", error => {
            this.log = (this.log + error.message).slice(-40_000);
        });
        child.stdout!.on("data", chunk => {
            this.log = (this.log + chunk).slice(-40_000);
        });
        child.stderr!.on("data", chunk => {
            this.log = (this.log + chunk).slice(-40_000);
        });
        return child;
    }
    private async ready(url: string, child: ChildProcess) {
        for (let attempt = 0; attempt < 300; attempt++) {
            if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Service exited\n${this.log}`);
            try {
                if ((await fetch(url, { signal: AbortSignal.timeout(1000) })).ok) return;
            } catch { /* Wait for startup. */ }
            await new Promise(done => setTimeout(done, 100));
        }
        throw new Error(`Service did not start: ${url}\n${this.log}`);
    }
    private async startWorld() {
        if (this.distributed) {
            const sockets = ["rpc", "events", "workers-request", "workers-response"];
            const address = (name: string) => `ipc://${this.directory}/${name}.sock`;
            const enrollment = `tcp://127.0.0.1:${await port()}`;
            const token = resolve(this.directory, "enrollment-token");
            const common = [
                "--enrollment-address",
                enrollment,
                "--enrollment-token-file",
                token,
                ...sockets.flatMap(name => [`--${name}-address`, address(name)]),
            ];
            this.rpcArgs = common;
            this.world = this.launch(resolve(workspace, "target/debug/moor-daemon"), [
                resolve(this.directory, "data"),
                "--config-file",
                resolve(this.directory, "config.yaml"),
                "--generate-keypair",
                "--private-key",
                resolve(this.directory, "signing.pem"),
                "--public-key",
                resolve(this.directory, "verifying.pem"),
                "--enrollment-listen",
                enrollment,
                "--enrollment-token-file",
                token,
                ...sockets.flatMap(name => [`--${name}-listen`, address(name)]),
            ], this.directory);
            for (let attempt = 0;; attempt++) {
                if (this.world.exitCode !== null || attempt > 600) throw new Error(`Daemon did not start\n${this.log}`);
                try {
                    await access(token);
                    await access(resolve(this.directory, "rpc.sock"));
                    break;
                } catch { /* starting */ }
                await new Promise(done => setTimeout(done, 100));
            }
            this.telnetPort = await port();
            for (
                const [name, args] of [
                    ["moor-web-host", ["--listen-address", `127.0.0.1:${this.apiPort}`]],
                    ["moor-telnet-host", [
                        "--telnet-address",
                        "127.0.0.1",
                        "--telnet-port",
                        String(this.telnetPort),
                        "--health-check-port",
                        "0",
                    ]],
                    ["moor-curl-worker", []],
                    ["moor-git-worker", ["--work-dir", resolve(this.directory, "jobs")]],
                ] as [string, string[]][]
            ) {
                this.hosts.push(
                    this.launch(resolve(workspace, "target/debug", name), [
                        ...common,
                        "--data-dir",
                        resolve(this.directory, name),
                        ...args,
                    ], this.directory),
                );
            }
            await this.ready(`http://127.0.0.1:${this.apiPort}/health`, this.hosts[0]);
            return;
        }
        this.world = this.launch(
            process.env.MOOR_TEST_BINARY ?? resolve(workspace, "target/debug/moor"),
            [
                resolve(this.directory, "data"),
                "--db",
                "world.db",
                "--config-file",
                resolve(this.directory, "config.yaml"),
                "--generate-keypair",
                "--enable-curl-worker",
                "--enable-git-worker",
            ],
            this.directory,
            { XDG_CONFIG_HOME: resolve(this.directory, "config"), XDG_DATA_HOME: resolve(this.directory, "share") },
        );
        await this.ready(`http://127.0.0.1:${this.apiPort}/health`, this.world);
    }
    holdHttp() {
        this.bundleGate = new Promise(done => {
            this.releaseBundle = done;
        });
        const arrived = new Promise<void>(done => {
            this.bundleArrived = done;
        });
        return { arrived, release: () => this.releaseBundle?.() };
    }
    holdGit() {
        this.gitGate = new Promise(done => {
            this.releaseGit = done;
        });
        const arrived = new Promise<void>(done => {
            this.gitArrived = done;
        });
        return { arrived, release: () => this.releaseGit?.() };
    }
    async restart() {
        await Promise.all(this.hosts.map(stop));
        this.hosts = [];
        await stop(this.world);
        await this.startWorld();
    }
    get browserUrl() {
        return `http://127.0.0.1:${this.browserPort}/@fs/${resolve(workspace, "clients/meadow/e2e/review.html")}`;
    }
    async close() {
        this.releaseBundle?.();
        this.releaseGit?.();
        await stop(this.vite);
        await Promise.all(this.hosts.map(stop));
        await stop(this.world);
        await Promise.all([...this.gitChildren].map(stop));
        if (this.server) {
            this.server.closeAllConnections();
            await new Promise<void>(done => this.server!.close(() => done()));
        }
        if (this.directory) await rm(this.directory, { recursive: true, force: true });
    }
}
