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
//

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

import {
    buildWsAttach,
    dispatchClientEvent,
    MoorVar,
    parseEvalResultVar,
    parseWsNarrativeEventMessage,
    schedulerErrorToNarrative,
} from "@moor/web-sdk";
import WebSocket from "ws";

import type { CharacterConfig, MoorWebMcpConfig } from "./config.js";

export interface DynamicTool {
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
    targetObj: string;
    targetVerb: string;
}

interface CharacterSessionState {
    authToken: string;
    clientToken: string | null;
    clientId: string | null;
    ws: WebSocket | null;
    wsConnectPromise: Promise<void> | null;
    recentEvents: string[];
    commandChain: Promise<void>;
}

interface CommandResult {
    lines: string[];
    events: unknown[];
    error: boolean;
}

function trimTrailingSlash(value: string): string {
    return value.endsWith("/") ? value.slice(0, -1) : value;
}

function inferWsBaseUrl(baseUrl: string): string {
    const parsed = new URL(baseUrl);
    parsed.protocol = parsed.protocol === "https:" ? "wss:" : "ws:";
    return `${parsed.protocol}//${parsed.host}`;
}

function objectLikeToRef(value: unknown): string {
    if (typeof value === "string") {
        return value;
    }
    if (value && typeof value === "object") {
        const obj = value as { oid?: number; uuid?: string };
        if (typeof obj.oid === "number") {
            return `#${obj.oid}`;
        }
        if (typeof obj.uuid === "string") {
            return `uuid:${obj.uuid}`;
        }
    }
    throw new Error(`Unable to convert object reference: ${JSON.stringify(value)}`);
}

function jsonToMooLiteral(value: unknown): string {
    if (value === null || value === undefined) {
        return "none";
    }
    if (typeof value === "boolean") {
        return value ? "1" : "0";
    }
    if (typeof value === "number") {
        if (!Number.isFinite(value)) {
            throw new Error("Non-finite numeric value is not representable in MOO literal");
        }
        return Number.isInteger(value) ? `${value}` : `${value}`;
    }
    if (typeof value === "string") {
        const escaped = value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
        return `"${escaped}"`;
    }
    if (Array.isArray(value)) {
        return `{${value.map((item) => jsonToMooLiteral(item)).join(", ")}}`;
    }

    const entries = Object.entries(value as Record<string, unknown>);
    return `[${entries.map(([k, v]) => `${jsonToMooLiteral(k)} -> ${jsonToMooLiteral(v)}`).join(", ")}]`;
}

function escapeMooString(value: string): string {
    return value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
}

function ensureOk(response: Response, context: string): Promise<Response> {
    if (response.ok) {
        return Promise.resolve(response);
    }
    return response.text().then((text) => {
        throw new Error(`${context} failed: ${response.status} ${response.statusText} ${text}`);
    });
}

function formatNotifyContent(content: unknown): string {
    if (Array.isArray(content)) {
        return content.map((item) => String(item)).join("\n");
    }
    return String(content);
}

function formatCommandEventForText(event: unknown): string | null {
    if (!event || typeof event !== "object") {
        return null;
    }
    const e = event as Record<string, unknown>;
    if (e.kind === "notify") {
        const text = formatNotifyContent(e.content);
        const contentType = typeof e.contentType === "string" ? e.contentType : "text/plain";
        if (contentType === "text/plain") {
            return text;
        }
        return `[${contentType}] ${text}`;
    }
    if (e.kind === "traceback" && typeof e.tracebackText === "string") {
        return e.tracebackText;
    }
    return null;
}

export class MoorWebClient {
    private readonly config: MoorWebMcpConfig;
    private readonly sessions = new Map<string, CharacterSessionState>();

    constructor(config: MoorWebMcpConfig) {
        this.config = config;
    }

    listCharacters(): CharacterConfig[] {
        return this.config.characters;
    }

    getRecentEvents(characterId: string): string[] {
        return [...(this.sessions.get(characterId)?.recentEvents ?? [])];
    }

    findCharacter(id: string): CharacterConfig {
        const character = this.config.characters.find((c) => c.id === id);
        if (!character) {
            throw new Error(`Unknown character id: ${id}`);
        }
        return character;
    }

    async evalExpression(characterId: string, expression: string): Promise<{ js: unknown; literal: string }> {
        const session = await this.ensureSession(characterId);
        const trimmed = expression.trim();
        const maybeWrapped = /(^return\b|;\s*$)/.test(trimmed) ? expression : `return ${trimmed};`;
        const response = await ensureOk(
            await fetch(`${trimTrailingSlash(this.config.baseUrl)}/v1/eval`, {
                method: "POST",
                headers: {
                    Accept: "application/x-flatbuffers",
                    "Content-Type": "text/plain",
                    "X-Moor-Auth-Token": session.authToken,
                },
                body: maybeWrapped,
            }),
            "eval",
        );

        const bytes = new Uint8Array(await response.arrayBuffer());
        const resultVar = parseEvalResultVar(bytes);
        const decoded = new MoorVar(resultVar);
        return { js: decoded.toJS(), literal: decoded.toLiteral() };
    }

    async command(characterId: string, command: string): Promise<CommandResult> {
        const session = await this.ensureSession(characterId);
        await this.ensureWs(characterId, session);

        let resolveResult!: (result: CommandResult) => void;
        let resolved = false;
        const done = new Promise<CommandResult>((resolve) => {
            resolveResult = resolve;
        });

        const lines: string[] = [];
        const events: unknown[] = [];
        let errored = false;
        let timeoutHandle: ReturnType<typeof setTimeout> | undefined;

        const finish = () => {
            if (resolved) {
                return;
            }
            resolved = true;
            if (timeoutHandle) {
                clearTimeout(timeoutHandle);
            }
            resolveResult({ lines, events, error: errored });
        };

        const installIdleTimeout = (ms: number) => {
            if (timeoutHandle) {
                clearTimeout(timeoutHandle);
            }
            timeoutHandle = setTimeout(() => finish(), ms);
        };

        const ws = session.ws;
        if (!ws || ws.readyState !== WebSocket.OPEN) {
            throw new Error("WebSocket not connected");
        }

        const onMessage = (data: unknown) => {
            try {
                const bytes = data instanceof Uint8Array ? data : new Uint8Array(data as ArrayBuffer);
                dispatchClientEvent(bytes, {
                    onNarrativeEventMessage: (narrative: unknown) => {
                        const parsed = parseWsNarrativeEventMessage(
                            narrative,
                            (value: unknown) => new MoorVar(value as any).toJS(),
                            (value: unknown) => new MoorVar(value as any).asString(),
                        );
                        if (parsed) {
                            events.push(parsed);
                            const text = formatCommandEventForText(parsed);
                            if (text !== null) {
                                lines.push(text);
                            }
                            installIdleTimeout(700);
                        }
                    },
                    onTaskErrorEvent: (taskError: any) => {
                        const schedulerError = taskError.error();
                        if (schedulerError) {
                            const narrative = schedulerErrorToNarrative(schedulerError);
                            if (narrative) {
                                lines.push(narrative.message);
                            }
                        }
                        errored = true;
                        finish();
                    },
                    onTaskSuccessEvent: () => {
                        finish();
                    },
                });
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                if (message.includes("empty client event")) {
                    return;
                }
                lines.push(`WebSocket decode error: ${message}`);
                events.push({ kind: "decode_error", message });
                errored = true;
                finish();
            }
        };

        const run = async () => {
            ws.on("message", onMessage);
            try {
                installIdleTimeout(5000);
                ws.send(command);
                return await done;
            } finally {
                ws.off("message", onMessage);
            }
        };

        const resultPromise = session.commandChain.then(() => run());
        session.commandChain = resultPromise.then(
            () => undefined,
            () => undefined,
        );
        return resultPromise;
    }

    async requestJson(characterId: string, path: string, init?: RequestInit): Promise<unknown> {
        const session = await this.ensureSession(characterId);
        const response = await ensureOk(
            await fetch(`${trimTrailingSlash(this.config.baseUrl)}${path}`, {
                ...(init ?? {}),
                headers: {
                    Accept: "application/json",
                    "X-Moor-Auth-Token": session.authToken,
                    ...(init?.headers ?? {}),
                },
            }),
            path,
        );
        return response.json();
    }

    async refreshDynamicTools(characterId: string): Promise<DynamicTool[]> {
        const { js } = await this.evalExpression(characterId, "return #0:external_agent_tools();");
        if (!Array.isArray(js)) {
            return [];
        }

        const results: DynamicTool[] = [];
        for (const entry of js) {
            if (!entry || typeof entry !== "object") {
                continue;
            }
            const asMap = entry as Record<string, unknown>;
            if (typeof asMap.name !== "string" || typeof asMap.target_verb !== "string") {
                continue;
            }

            let targetObj: string;
            try {
                targetObj = objectLikeToRef(asMap.target_obj);
            } catch {
                continue;
            }

            results.push({
                name: asMap.name,
                description: typeof asMap.description === "string" ? asMap.description : asMap.name,
                inputSchema: typeof asMap.input_schema === "object" && asMap.input_schema
                    ? asMap.input_schema as Record<string, unknown>
                    : { type: "object", properties: {} },
                targetObj,
                targetVerb: asMap.target_verb,
            });
        }
        return results;
    }

    async executeDynamicTool(
        characterId: string,
        tool: DynamicTool,
        args: unknown,
    ): Promise<{ js: unknown; literal: string }> {
        const mooArgs = jsonToMooLiteral(args ?? {});
        const expression = `return ${tool.targetObj}:${tool.targetVerb}(${mooArgs}, player);`;
        return this.evalExpression(characterId, expression);
    }

    async invokeVerbViaEval(
        characterId: string,
        objectRef: string,
        verb: string,
        args: unknown[],
    ): Promise<{ js: unknown; literal: string }> {
        const argLiteral = jsonToMooLiteral(args);
        return this.evalExpression(characterId, `return ${objectRef}:${verb}(@${argLiteral});`);
    }

    async functionHelp(characterId: string, functionName: string): Promise<{ js: unknown; literal: string }> {
        const escaped = escapeMooString(functionName);
        return this.evalExpression(characterId, `return function_help("${escaped}");`);
    }

    async close(): Promise<void> {
        for (const session of this.sessions.values()) {
            if (session.ws) {
                session.ws.close();
            }
            session.ws = null;
            session.wsConnectPromise = null;
        }
    }

    private async ensureSession(characterId: string): Promise<CharacterSessionState> {
        const existing = this.sessions.get(characterId);
        if (existing) {
            return existing;
        }

        const character = this.findCharacter(characterId);
        const formBody = new URLSearchParams({
            player: character.username,
            password: character.password,
        }).toString();

        const response = await ensureOk(
            await fetch(`${trimTrailingSlash(this.config.baseUrl)}/auth/connect`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/x-www-form-urlencoded",
                },
                body: formBody,
            }),
            "auth/connect",
        );

        const authToken = response.headers.get("X-Moor-Auth-Token");
        if (!authToken) {
            throw new Error(`auth/connect missing X-Moor-Auth-Token for character ${character.id}`);
        }

        const session: CharacterSessionState = {
            authToken,
            clientToken: response.headers.get("X-Moor-Client-Token"),
            clientId: response.headers.get("X-Moor-Client-Id"),
            ws: null,
            wsConnectPromise: null,
            recentEvents: [],
            commandChain: Promise.resolve(),
        };

        this.sessions.set(characterId, session);
        return session;
    }

    private async ensureWs(characterId: string, session: CharacterSessionState): Promise<void> {
        if (session.ws && session.ws.readyState === WebSocket.OPEN) {
            return;
        }
        if (session.wsConnectPromise) {
            await session.wsConnectPromise;
            return;
        }

        const baseWsUrl = this.config.wsBaseUrl ?? inferWsBaseUrl(this.config.baseUrl);
        const { wsUrl, protocols } = buildWsAttach(baseWsUrl, {
            mode: "connect",
            credentials: {
                authToken: session.authToken,
                clientId: session.clientId,
                clientToken: session.clientToken,
            },
        });

        const ws = new WebSocket(wsUrl, protocols);
        session.ws = ws;
        const wsConnectPromise = new Promise<void>((resolve, reject) => {
            const onOpen = () => {
                ws.off("error", onError);
                resolve();
            };
            const onError = (error: Error) => {
                ws.off("open", onOpen);
                reject(error);
            };
            ws.once("open", onOpen);
            ws.once("error", onError);
        });
        session.wsConnectPromise = wsConnectPromise;

        const clearIfCurrent = () => {
            if (session.ws === ws) {
                session.ws = null;
            }
            if (session.wsConnectPromise === wsConnectPromise) {
                session.wsConnectPromise = null;
            }
        };

        ws.on("message", (rawData: unknown) => {
            try {
                const bytes = rawData instanceof Uint8Array ? rawData : new Uint8Array(rawData as ArrayBuffer);
                dispatchClientEvent(bytes, {
                    onNarrativeEventMessage: (narrative: unknown) => {
                        const parsed = parseWsNarrativeEventMessage(
                            narrative,
                            (value: unknown) => new MoorVar(value as any).toJS(),
                            (value: unknown) => new MoorVar(value as any).asString(),
                        );
                        if (parsed?.kind !== "notify") {
                            return;
                        }
                        const text = Array.isArray(parsed.content) ? parsed.content.join("\n") : String(parsed.content);
                        session.recentEvents.push(text);
                        if (session.recentEvents.length > 200) {
                            session.recentEvents.shift();
                        }
                    },
                });
            } catch {
                // Ignore non-client-event frames while keeping connection alive.
            }
        });
        ws.on("close", clearIfCurrent);
        ws.on("error", clearIfCurrent);

        try {
            await wsConnectPromise;
        } catch (error) {
            clearIfCurrent();
            throw error;
        } finally {
            if (session.wsConnectPromise === wsConnectPromise) {
                session.wsConnectPromise = null;
            }
        }
    }
}
