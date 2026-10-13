// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import { ClientEvent } from "@moor/schema/generated/moor-rpc/client-event";
import { ClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import { PlayerSwitchedEvent } from "@moor/schema/generated/moor-rpc/player-switched-event";
import { RequestInputEvent } from "@moor/schema/generated/moor-rpc/request-input-event";
import * as flatbuffers from "flatbuffers";
import { actionBody } from "./actions.js";
import { buildAuthHeaders } from "./auth.js";
import type { SessionCredentials, WsAttachOptions } from "./types.js";

/** Common interface consumed by the interactive session hook. */
export interface SessionTransport {
    readonly readyState: number;
    onopen: ((event: Event) => void) | null;
    onmessage: ((event: MessageEvent) => void | Promise<void>) | null;
    onerror: ((event: Event) => void) | null;
    onclose: ((event: CloseEvent) => void) | null;
    send(data: string | Uint8Array<ArrayBuffer> | ArrayBuffer): void;
    close(code?: number, reason?: string): void;
}

/** Kept by the session owner across network reconnects; never advances on notification receipt. */
export interface SseResumeState {
    streamId?: string;
    fresh?: boolean;
    sequence: bigint;
    inputRequests: string[];
    pendingTask: boolean;
}

interface SseFrame {
    event: string;
    data: string;
    id?: string;
}

// A daemon payload is bounded by 16 MiB; allow its base64 expansion and SSE fields.
const MAX_SSE_FRAME_CHARS = 24 * 1024 * 1024;

/** Parse incrementally, bounding incomplete frames even for a malformed peer. */
export async function* readSse(body: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
    const reader = body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
        while (true) {
            const { done, value } = await reader.read();
            if (done) return;
            buffer += decoder.decode(value, { stream: true });
            let boundary: RegExpExecArray | null;
            while ((boundary = /\r?\n\r?\n/.exec(buffer))) {
                const block = buffer.slice(0, boundary.index);
                buffer = buffer.slice(boundary.index + boundary[0].length);
                if (block.length > MAX_SSE_FRAME_CHARS) throw new Error("SSE frame exceeds limit");
                let event = "message";
                let id: string | undefined;
                const data: string[] = [];
                for (const line of block.split(/\r?\n/)) {
                    if (line.startsWith(":")) continue;
                    const colon = line.indexOf(":");
                    const name = colon < 0 ? line : line.slice(0, colon);
                    const value = colon < 0 ? "" : line.slice(colon + 1).replace(/^ /, "");
                    if (name === "event") event = value;
                    if (name === "data") data.push(value);
                    if (name === "id") id = value;
                }
                if (data.length) yield { event, data: data.join("\n"), id };
            }
            if (buffer.length > MAX_SSE_FRAME_CHARS) throw new Error("SSE frame exceeds limit");
        }
    } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
    }
}

function uuidString(bytes: Uint8Array | null | undefined): string {
    if (!bytes || bytes.length !== 16) throw new Error("Missing stream UUID");
    return Array.from(bytes, (b, i) => ([4, 6, 8, 10].includes(i) ? "-" : "") + b.toString(16).padStart(2, "0")).join(
        "",
    );
}

class ExpiredStream extends Error {}
class UnauthorizedStream extends Error {}

/** SSE carries base64 FlatBuffers; the daemon retains them until cumulative acknowledgement. */
export class SseSessionTransport implements SessionTransport {
    readyState = 0;
    onopen: SessionTransport["onopen"] = null;
    onmessage: SessionTransport["onmessage"] = null;
    onerror: SessionTransport["onerror"] = null;
    onclose: SessionTransport["onclose"] = null;
    private readonly abort = new AbortController();
    private credentials: SessionCredentials;
    private heartbeat?: ReturnType<typeof setInterval>;
    private ackTimer?: ReturnType<typeof setTimeout>;
    private acknowledging = false;
    private acknowledgedSequence = 0n;
    private lastNotificationAt = Date.now();
    private sending = false;
    private outgoing: (string | Uint8Array<ArrayBuffer> | ArrayBuffer)[] = [];

    constructor(
        private readonly baseUrl: string,
        private readonly options: WsAttachOptions,
        private readonly resume: SseResumeState,
        private readonly onCredentials: (credentials: { clientId: string; clientToken: string }) => void,
        private readonly fetcher: typeof fetch = (...args) => fetch(...args),
    ) {
        this.credentials = { ...options.credentials };
        if (resume.fresh) {
            this.credentials.clientId = null;
            this.credentials.clientToken = null;
            this.credentials.isInitialAttach = false;
        }
        // Install handlers before attachment can complete, including with an in-process mock.
        queueMicrotask(() => {
            void this.run().catch(error => this.fail(error));
        });
    }

    private async request(path: string, init: RequestInit = {}, streaming = false): Promise<Response> {
        const response = await this.fetcher(this.baseUrl + path, {
            ...init,
            headers: { ...buildAuthHeaders(this.credentials), ...init.headers },
            signal: streaming ? this.abort.signal : AbortSignal.any([this.abort.signal, AbortSignal.timeout(10000)]),
            cache: "no-store",
        });
        if (response.status === 401) throw new UnauthorizedStream("Session credentials expired");
        if (response.status === 410) throw new ExpiredStream("Event delivery expired; reconnecting");
        if (!response.ok) throw new Error(`Session request failed: ${response.status}`);
        return response;
    }

    private async run(): Promise<void> {
        const response = await this.request("/v1/session", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                initial: !!this.credentials.isInitialAttach,
                create: this.options.mode === "create",
            }),
        });
        const attached = await response.json() as {
            client_id: string;
            client_token: string;
            stream_id: string;
            acknowledged_sequence: string;
            available_after: string;
            latest_sequence: string;
        };
        if (this.abort.signal.aborted) return;
        this.credentials.clientId = attached.client_id;
        this.credentials.clientToken = attached.client_token;
        this.onCredentials({ clientId: attached.client_id, clientToken: attached.client_token });
        const acknowledged = BigInt(attached.acknowledged_sequence);
        this.acknowledgedSequence = acknowledged;
        if (BigInt(attached.available_after) > acknowledged) throw new ExpiredStream("Unacknowledged events expired");
        if (this.resume.streamId !== attached.stream_id) {
            if (acknowledged > 0n) throw new ExpiredStream("Connection control state is unavailable");
            this.resume.fresh = false;
            this.resume.streamId = attached.stream_id;
            this.resume.sequence = acknowledged;
            this.resume.inputRequests = [];
            this.resume.pendingTask = false;
        }
        if (this.resume.sequence < acknowledged) throw new ExpiredStream("Local delivery cursor is behind the server");
        const stream = await this.request(
            `/v1/events/stream?stream_id=${attached.stream_id}&after=${this.resume.sequence}`,
            {},
            true,
        );
        if (!stream.body || !stream.headers.get("content-type")?.startsWith("text/event-stream")) {
            throw new Error("Missing SSE response body");
        }
        this.lastNotificationAt = Date.now();
        this.readyState = 1;
        this.scheduleAcknowledgement();
        this.heartbeat = setInterval(() => {
            if (Date.now() - this.lastNotificationAt > 20000) {
                this.fail(new Error("Event stream heartbeat timed out"));
                return;
            }
            void this.acknowledge().catch(error => this.fail(error));
        }, 10000);
        this.onopen?.(new Event("open"));
        for await (const frame of readSse(stream.body)) {
            if (this.abort.signal.aborted) return;
            if (frame.event === "reset") throw new ExpiredStream("Event delivery expired; reconnecting");
            if (frame.event === "retry") throw new Error("Daemon unavailable");
            this.lastNotificationAt = Date.now();
            if (frame.event === "heartbeat") continue;
            if (frame.event !== "delivery") throw new Error("Unexpected SSE event");
            const decoded = atob(frame.data);
            const bytes = new Uint8Array(decoded.length);
            for (let i = 0; i < decoded.length; i++) bytes[i] = decoded.charCodeAt(i);
            const event = ClientEvent.getRootAsClientEvent(new flatbuffers.ByteBuffer(bytes));
            const sequence = event.sequence();
            if (frame.id !== `${this.resume.streamId}:${sequence}`) throw new Error("Invalid stream event ID");
            if (sequence !== this.resume.sequence + 1n) throw new ExpiredStream("Event delivery sequence gap");
            // Await application dispatch before advancing the cumulative acknowledgement.
            if (!this.onmessage) throw new Error("No event handler is installed");
            await this.onmessage(new MessageEvent("message", { data: bytes.buffer }));
            if (this.abort.signal.aborted) return;
            this.applyControl(event);
            this.resume.sequence = sequence;
            this.scheduleAcknowledgement();
            void this.pump().catch(error => this.fail(error));
        }
        if (!this.abort.signal.aborted) throw new Error("Event stream closed");
    }

    private applyControl(event: ClientEvent): void {
        switch (event.eventType()) {
            case ClientEventUnion.RequestInputEvent: {
                const input = event.event(new RequestInputEvent()) as RequestInputEvent;
                this.resume.inputRequests.push(uuidString(input.requestId()?.dataArray()));
                break;
            }
            case ClientEventUnion.TaskSuccessEvent:
            case ClientEventUnion.TaskErrorEvent:
            case ClientEventUnion.TaskSuspendedEvent:
                this.resume.pendingTask = false;
                break;
            case ClientEventUnion.PlayerSwitchedEvent: {
                const switched = event.event(new PlayerSwitchedEvent()) as PlayerSwitchedEvent;
                const token = switched.newAuthToken()?.token();
                if (!token) throw new Error("Missing switched player credentials");
                this.credentials.authToken = token;
                break;
            }
        }
    }

    /** Coalesce processed batches without putting ACK latency on the delivery path. */
    private scheduleAcknowledgement(): void {
        if (
            this.abort.signal.aborted || this.ackTimer !== undefined || this.acknowledging
            || this.resume.sequence <= this.acknowledgedSequence
        ) return;
        this.ackTimer = setTimeout(() => {
            this.ackTimer = undefined;
            void this.acknowledge().catch(error => this.fail(error));
        }, 100);
    }

    private async acknowledge(): Promise<void> {
        if (this.acknowledging || this.abort.signal.aborted || !this.resume.streamId) return;
        clearTimeout(this.ackTimer);
        this.ackTimer = undefined;
        this.acknowledging = true;
        const sequence = this.resume.sequence;
        try {
            await this.request("/v1/events/ack", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ stream_id: this.resume.streamId, sequence: sequence.toString() }),
            });
            this.acknowledgedSequence = sequence;
        } finally {
            this.acknowledging = false;
            this.scheduleAcknowledgement();
        }
    }

    send(data: string | Uint8Array<ArrayBuffer> | ArrayBuffer): void {
        if (this.readyState !== 1) throw new Error("Session is not connected");
        if (typeof data !== "string") {
            const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
            if (bytes.length === 1 && (bytes[0] === 0 || bytes[0] === 1)) return;
        }
        if (this.outgoing.length >= 64) throw new Error("Command queue is full");
        this.outgoing.push(data);
        void this.pump().catch(error => this.fail(error));
    }

    private async pump(): Promise<void> {
        if (this.sending) return;
        this.sending = true;
        try {
            while (this.outgoing.length && !this.abort.signal.aborted) {
                const inputId = this.resume.inputRequests[0];
                if (this.resume.pendingTask && !inputId) return;
                // A prompt reply may pass a queued action, but cannot consume it.
                const index = inputId ? this.outgoing.findIndex(data => !actionBody(data)) : 0;
                if (index < 0) return;
                const data = this.outgoing.splice(index, 1)[0];
                const action = actionBody(data);
                if (!inputId && !action && typeof data !== "string") throw new Error("Command must be text");
                if (!inputId) this.resume.pendingTask = true;
                // Commands are never retried automatically: a lost reply can follow successful execution.
                await this.request(inputId ? `/v1/session/input/${inputId}` : "/v1/session/command", {
                    method: "POST",
                    headers: {
                        "Content-Type": action
                            ? "application/json"
                            : typeof data === "string"
                            ? "text/plain"
                            : "application/x-flatbuffers",
                    },
                    body: action ?? data,
                });
                if (inputId) this.resume.inputRequests.shift();
            }
        } finally {
            this.sending = false;
        }
    }

    private fail(error: unknown): void {
        if (this.abort.signal.aborted) return;
        if (error instanceof ExpiredStream) {
            // An expired queue cannot restore connection-local control state. Start a fresh session;
            // the normal connect path restores history and presentation snapshots.
            void this.fetcher(this.baseUrl + "/auth/logout", {
                method: "POST",
                headers: buildAuthHeaders(this.credentials),
            }).catch(() => {});
            this.resume.fresh = true;
            this.resume.streamId = undefined;
            this.resume.sequence = 0n;
        }
        this.onerror?.(new Event("error"));
        this.close(
            error instanceof ExpiredStream ? 4009 : error instanceof UnauthorizedStream ? 4401 : 4001,
            error instanceof Error ? error.message : "Event stream failed",
        );
    }

    close(code = 1000, reason = ""): void {
        if (this.readyState === 3) return;
        this.readyState = 3;
        clearInterval(this.heartbeat);
        clearTimeout(this.ackTimer);
        this.abort.abort();
        this.outgoing = [];
        if (reason === "LOGOUT") {
            void this.fetcher(this.baseUrl + "/auth/logout", {
                method: "POST",
                headers: buildAuthHeaders(this.credentials),
            }).catch(() => {});
        }
        this.onclose?.(new CloseEvent("close", { code, reason }));
    }
}
