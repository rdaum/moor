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

import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { SchedulerErrorUnion } from "@moor/schema/generated/moor-rpc/scheduler-error-union";
import { TaskAbortedException } from "@moor/schema/generated/moor-rpc/task-aborted-exception";
import {
    buildWsAttach,
    bytesFromWebSocketMessage,
    decodeCredentialsUpdatedEvent,
    dispatchClientEvent,
    handleWebSocketControlFrame,
    MoorVar,
    parseEvalResultVar,
    parseWsNarrativeEventMessage,
    schedulerErrorToNarrative,
} from "@moor/web-sdk";
import { VerbCallFailure } from "@moor/web-sdk/verb";
import WebSocket from "ws";

import type { CharacterRef, MooConfig, MoorWebMcpConfig, PlayerConfig } from "./config.js";

export interface DynamicTool {
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
    targetObj: string;
    targetVerb: string;
}

export interface DynamicResource {
    uri: string;
    name: string;
    description: string;
    mimeType: string;
    content: string;
}

interface CharacterSessionState {
    authToken: string;
    clientToken: string | null;
    clientId: string | null;
    ws: WebSocket | null;
    wsConnectPromise: Promise<void> | null;
    recentEvents: string[];
    commandChain: Promise<void>;
    command?: { event: (bytes: Uint8Array) => void; fail: (message: string) => void };
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

function mooJson(value: unknown): string {
    return `parse_json("${escapeMooString(JSON.stringify(value))}")`;
}

function mooObject(reference: string): string {
    if (/^oid:-?\d+$/.test(reference)) return `#${reference.slice(4)}`;
    if (/^uuid:[0-9a-f]{6}-[0-9a-f]{10}$/i.test(reference)) return `#${reference.slice(5)}`;
    if (/^#(?:-?\d+|[0-9a-f]{6}-[0-9a-f]{10})$/i.test(reference)) return reference;
    throw new Error(`Invalid target object reference: ${reference}`);
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
    private readonly connecting = new Map<string, Promise<CharacterSessionState>>();
    private readonly shutdown = new AbortController();
    private readonly timeoutMs: number;

    constructor(config: MoorWebMcpConfig) {
        this.config = config;
        this.timeoutMs = config.timeoutMs ?? 30000;
    }

    getRecentEvents(character: CharacterRef): string[] {
        return [...(this.sessions.get(this.sessionKey(character))?.recentEvents ?? [])];
    }

    findCharacter(character: CharacterRef): { moo: MooConfig; player: PlayerConfig } {
        const moo = this.config.moos.find((candidate) => candidate.id === character.mooId);
        if (!moo) {
            throw new Error(`Unknown moo id: ${character.mooId}`);
        }
        const player = moo.players.find((candidate) => candidate.id === character.playerId);
        if (!player) {
            throw new Error(`Unknown player id ${character.playerId} in moo ${moo.id}`);
        }
        return { moo, player };
    }

    async evalExpression(
        character: CharacterRef,
        expression: string,
        signal?: AbortSignal,
    ): Promise<{ js: unknown; literal: string }> {
        const trimmed = expression.trim();
        const body = /(^return\b|;\s*$)/.test(trimmed) ? expression : `return ${trimmed};`;
        const response = await this.authorizedRequest(character, "/v1/eval", {
            method: "POST",
            headers: { Accept: "application/x-flatbuffers", "Content-Type": "text/plain" },
            body,
            signal,
        });

        const bytes = new Uint8Array(await response.arrayBuffer());
        try {
            const decoded = new MoorVar(parseEvalResultVar(bytes));
            return { js: decoded.toJS(), literal: decoded.toLiteral() };
        } catch (error) {
            if (error instanceof VerbCallFailure) {
                const scheduler = error.failure.error();
                if (scheduler?.errorType() === SchedulerErrorUnion.TaskAbortedException) {
                    const aborted = scheduler.error(new TaskAbortedException()) as TaskAbortedException;
                    const detail = aborted.exception()?.error();
                    if (detail) error.message = `Eval: ${ErrorCode[detail.errType()]} (${detail.msg() ?? ""})`;
                }
            }
            throw error;
        }
    }

    async command(character: CharacterRef, command: string, signal?: AbortSignal): Promise<CommandResult> {
        const session = await this.ensureSession(character);
        const run = async (): Promise<CommandResult> => {
            this.shutdown.signal.throwIfAborted();
            signal?.throwIfAborted();
            await this.ensureWs(character, session);
            const ws = session.ws!;
            return new Promise<CommandResult>(resolve => {
                const lines: string[] = [];
                const events: unknown[] = [];
                let settled = false;
                const finish = (error: boolean, message?: string) => {
                    if (settled) return;
                    settled = true;
                    clearTimeout(timer);
                    signal?.removeEventListener("abort", cancelled);
                    session.command = undefined;
                    if (message) lines.push(message);
                    // A timed-out command may still run. Discard its connection so late
                    // completion cannot be mistaken for the next queued command's result.
                    if (error && message) ws.terminate();
                    resolve({ lines, events, error });
                };
                const cancelled = () => finish(true, "Command cancelled; its outcome may be unknown.");
                const timer = setTimeout(
                    () => finish(true, "Command timed out; its outcome may be unknown."),
                    this.timeoutMs,
                );
                signal?.addEventListener("abort", cancelled, { once: true });
                session.command = {
                    fail: message => finish(true, message),
                    event: bytes =>
                        dispatchClientEvent(bytes, {
                            onNarrativeEventMessage: narrative => {
                                const parsed = parseWsNarrativeEventMessage(narrative, value =>
                                    new MoorVar(value).toJS(), value =>
                                    new MoorVar(value).asString());
                                if (parsed) {
                                    events.push(parsed);
                                    const text = formatCommandEventForText(parsed);
                                    if (text !== null) {
                                        lines.push(text);
                                    }
                                }
                            },
                            onTaskErrorEvent: event => {
                                const error = event.error();
                                const narrative = error ? schedulerErrorToNarrative(error) : null;
                                if (narrative) {
                                    lines.push(narrative.message);
                                }
                                finish(true);
                            },
                            onTaskSuccessEvent: () =>
                                finish(false),
                            onRequestInputEvent: () =>
                                finish(true, "Command requires interactive input; use a MOO client."),
                        }),
                };
                if (signal?.aborted) cancelled();
                else {ws.send(command, error => {
                        if (error) finish(true, error.message);
                    });}
            });
        };
        const pending = session.commandChain.then(run);
        session.commandChain = pending.then(() => undefined, () => undefined);
        return pending;
    }

    private fetch(url: string, init: RequestInit): Promise<Response> {
        const signals = [this.shutdown.signal, AbortSignal.timeout(this.timeoutMs)];
        if (init.signal) signals.push(init.signal);
        return fetch(url, { ...init, signal: AbortSignal.any(signals) });
    }

    async requestJson(
        character: CharacterRef,
        path: string,
        init?: RequestInit,
        signal?: AbortSignal,
    ): Promise<unknown> {
        const response = await this.authorizedRequest(character, path, { ...init, signal: signal ?? init?.signal });
        const data = await response.json();
        const reply = data?.result?.ClientSuccess?.reply?.reply;
        // HTTP 200 carries both successful RPC replies and rejected operations.
        const failure = data?.result?.Failure
            ?? reply?.VerbProgramResponseReply?.response?.response?.VerbProgramFailure;
        if (failure) throw new Error(`${path} failed: ${JSON.stringify(failure)}`);
        return reply ?? data;
    }

    private async authorizedRequest(character: CharacterRef, path: string, init: RequestInit): Promise<Response> {
        const session = await this.ensureSession(character);
        const { moo } = this.findCharacter(character);
        const response = await this.fetch(`${trimTrailingSlash(moo.connectAddress)}${path}`, {
            ...init,
            headers: { Accept: "application/json", "X-Moor-Auth-Token": session.authToken, ...init.headers },
        });
        if (response.status === 401 && this.sessions.get(this.sessionKey(character)) === session) {
            this.sessions.delete(this.sessionKey(character));
            session.ws?.terminate();
        }
        return ensureOk(response, path);
    }

    async refreshDynamicTools(character: CharacterRef): Promise<DynamicTool[]> {
        const { js } = await this.evalExpression(
            character,
            "return respond_to(#0, \"external_agent_tools\") ? #0:external_agent_tools() | {};",
        );
        if (!Array.isArray(js)) throw new Error("Tool registry must return a list");

        const results: DynamicTool[] = [];
        for (const entry of js) {
            if (!entry || typeof entry !== "object") throw new Error("Invalid tool registry entry");
            const asMap = entry as Record<string, unknown>;
            if (
                typeof asMap.name !== "string" || !/^[a-zA-Z0-9_-]{1,64}$/.test(asMap.name)
                || typeof asMap.target_verb !== "string" || !asMap.target_verb.length
                || results.some(tool => tool.name === asMap.name)
            ) throw new Error("Invalid or duplicate tool registry name");

            const targetObj = mooObject(objectLikeToRef(asMap.target_obj));
            if (
                !asMap.input_schema || typeof asMap.input_schema !== "object" || Array.isArray(asMap.input_schema)
                || (asMap.input_schema as Record<string, unknown>).type !== "object"
            ) throw new Error(`Invalid schema for ${asMap.name}`);

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

    async refreshDynamicResources(character: CharacterRef): Promise<DynamicResource[]> {
        const { js } = await this.evalExpression(
            character,
            "return respond_to(#0, \"external_agent_resources\") ? #0:external_agent_resources() | {};",
        );
        if (!Array.isArray(js)) throw new Error("Resource registry must return a list");
        const seen = new Set<string>();
        for (const entry of js) {
            if (
                !entry || typeof entry !== "object"
                || !["uri", "name", "description", "mimeType", "content"].every(key => typeof entry[key] === "string")
                || seen.has(entry.uri) || entry.uri === "moor://moos" || /^moor:\/\/(events|world)\//.test(entry.uri)
            ) throw new Error("Invalid or duplicate resource registry entry");
            seen.add(entry.uri);
        }
        return js;
    }

    async executeDynamicTool(
        character: CharacterRef,
        tool: DynamicTool,
        args: unknown,
        signal?: AbortSignal,
    ): Promise<{ js: unknown; literal: string }> {
        // Decode actual JSON in the world so booleans, strings and multiline
        // programs use the same mapping as internal agents.
        const expression = `return ${mooObject(tool.targetObj)}:(${mooJson(tool.targetVerb)})(${
            mooJson(args ?? {})
        }, player);`;
        return this.evalExpression(character, expression, signal);
    }

    async invokeVerbViaEval(
        character: CharacterRef,
        objectRef: string,
        verb: string,
        args: unknown[],
        signal?: AbortSignal,
    ): Promise<{ js: unknown; literal: string }> {
        return this.evalExpression(
            character,
            `return ${mooObject(objectRef)}:(${mooJson(verb)})(@${mooJson(args)});`,
            signal,
        );
    }

    async functionHelp(
        character: CharacterRef,
        functionName: string,
        signal?: AbortSignal,
    ): Promise<{ js: unknown; literal: string }> {
        return this.evalExpression(character, `return function_help(${mooJson(functionName)});`, signal);
    }

    async propertyValue(character: CharacterRef, object: string, name: string, signal?: AbortSignal) {
        // Native property access also supports built-in fields such as name.
        return this.evalExpression(character, `return ${mooObject(object)}.(${mooJson(name)});`, signal);
    }

    async close(): Promise<void> {
        this.shutdown.abort();
        for (const session of this.sessions.values()) {
            if (session.ws) {
                session.command?.fail("MCP connection closed; command outcome may be unknown.");
                session.ws.terminate();
            }
            session.ws = null;
            session.wsConnectPromise = null;
        }
    }

    private sessionKey(character: CharacterRef): string {
        return JSON.stringify([character.mooId, character.playerId]);
    }

    private async ensureSession(character: CharacterRef): Promise<CharacterSessionState> {
        const key = this.sessionKey(character);
        const existing = this.sessions.get(key);
        if (existing) {
            return existing;
        }

        let pending = this.connecting.get(key);
        if (!pending) {
            pending = this.login(character);
            this.connecting.set(key, pending);
            pending.finally(() => {
                this.connecting.delete(key);
            }).catch(() => {});
        }
        return pending;
    }

    private async login(character: CharacterRef): Promise<CharacterSessionState> {
        const key = this.sessionKey(character);
        const selected = this.findCharacter(character);
        const formBody = new URLSearchParams({
            player: selected.player.username,
            password: selected.player.password,
        }).toString();

        const response = await ensureOk(
            await this.fetch(`${trimTrailingSlash(selected.moo.connectAddress)}/auth/connect`, {
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
            throw new Error(
                `auth/connect missing X-Moor-Auth-Token for ${selected.moo.id}/${selected.player.id}`,
            );
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

        this.sessions.set(key, session);
        return session;
    }

    private async ensureWs(character: CharacterRef, session: CharacterSessionState): Promise<void> {
        if (session.ws && session.ws.readyState === WebSocket.OPEN) {
            return;
        }
        if (session.wsConnectPromise) {
            await session.wsConnectPromise;
            return;
        }

        const { moo } = this.findCharacter(character);
        const baseWsUrl = moo.wsConnectAddress ?? inferWsBaseUrl(moo.connectAddress);
        const { wsUrl, protocols } = buildWsAttach(baseWsUrl, {
            mode: "connect",
            credentials: {
                authToken: session.authToken,
                clientId: session.clientId,
                clientToken: session.clientToken,
            },
        });

        const ws = new WebSocket(wsUrl, protocols, { handshakeTimeout: this.timeoutMs });
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
            ws.once("close", () => reject(new Error("WebSocket closed during connection")));
        });
        session.wsConnectPromise = wsConnectPromise;

        const clearIfCurrent = () => {
            if (session.ws === ws) session.command?.fail("WebSocket closed; command outcome may be unknown.");
            if (session.ws === ws) {
                session.ws = null;
            }
            if (session.wsConnectPromise === wsConnectPromise) {
                session.wsConnectPromise = null;
            }
        };

        ws.on("message", (rawData: unknown) => {
            if (session.ws !== ws) return;
            try {
                const bytes = bytesFromWebSocketMessage(rawData);
                if (!bytes) {
                    return;
                }
                if (handleWebSocketControlFrame(bytes, (payload) => ws.send(payload))) {
                    return;
                }
                dispatchClientEvent(bytes, {
                    onNarrativeEventMessage: narrative => {
                        const parsed = parseWsNarrativeEventMessage(
                            narrative,
                            value => new MoorVar(value).toJS(),
                            value => new MoorVar(value).asString(),
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
                    onCredentialsUpdatedEvent: credentials => {
                        const updated = decodeCredentialsUpdatedEvent(credentials);
                        if (!updated) {
                            return;
                        }
                        session.clientId = updated.clientId;
                        session.clientToken = updated.clientToken;
                    },
                });
                session.command?.event(bytes);
            } catch (error) {
                session.command?.fail(
                    `WebSocket decode error: ${error instanceof Error ? error.message : String(error)}`,
                );
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
