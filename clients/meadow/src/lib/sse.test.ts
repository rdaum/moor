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

import { Uuid } from "@moor/schema/generated/moor-common/uuid";
import { ClientEvent } from "@moor/schema/generated/moor-rpc/client-event";
import { ClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import { DisconnectEvent } from "@moor/schema/generated/moor-rpc/disconnect-event";
import { RequestInputEvent } from "@moor/schema/generated/moor-rpc/request-input-event";
import { TaskSuspendedEvent } from "@moor/schema/generated/moor-rpc/task-suspended-event";
import { Builder } from "flatbuffers";
import { afterEach, expect, it, vi } from "vitest";
import { encodeAction } from "../../../web-sdk/src/actions";
import { readSse, type SseResumeState, SseSessionTransport } from "../../../web-sdk/src/sse";

const streamId = "11111111-1111-1111-1111-111111111111";
const cleanups: (() => void)[] = [];
afterEach(() => {
    for (const cleanup of cleanups.splice(0)) cleanup();
    vi.restoreAllMocks();
    vi.useRealTimers();
});

function eventBytes(sequence: bigint, kind = ClientEventUnion.DisconnectEvent): Uint8Array<ArrayBuffer> {
    const b = new Builder();
    const event = kind === ClientEventUnion.RequestInputEvent
        ? RequestInputEvent.createRequestInputEvent(
            b,
            Uuid.createUuid(b, Uuid.createDataVector(b, new Uint8Array(16).fill(0x22))),
            0,
        )
        : kind === ClientEventUnion.TaskSuspendedEvent
        ? TaskSuspendedEvent.createTaskSuspendedEvent(b, 7n)
        : DisconnectEvent.createDisconnectEvent(b);
    ClientEvent.startClientEvent(b);
    ClientEvent.addSequence(b, sequence);
    ClientEvent.addEventType(b, kind);
    ClientEvent.addEvent(b, event);
    b.finish(ClientEvent.endClientEvent(b));
    return b.asUint8Array().slice();
}

function fixture(resume: SseResumeState = { sequence: 0n, inputRequests: [], pendingTask: false }) {
    let controller!: ReadableStreamDefaultController<Uint8Array>;
    const stream = new ReadableStream<Uint8Array>({
        start(c) {
            controller = c;
        },
    });
    const acks: string[] = [];
    const requests: string[] = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        requests.push(path);
        if (path === "/v1/session") {
            return Response.json({
                client_id: "client",
                client_token: "token",
                stream_id: streamId,
                acknowledged_sequence: "0",
                available_after: "0",
                latest_sequence: "0",
            });
        }
        if (path.startsWith("/v1/events/stream")) {
            return new Response(stream, { headers: { "Content-Type": "text/event-stream" } });
        }
        if (path === "/v1/events/ack") {
            acks.push(JSON.parse(String(init?.body)).sequence);
            return new Response(null, { status: 204 });
        }
        if (path.startsWith("/v1/session/input/") || path === "/v1/session/command") {
            return new Response(null, { status: 202 });
        }
        if (path === "/auth/logout") return new Response(null, { status: 200 });
        throw new Error(`Unexpected request: ${path}`);
    });
    const transport = new SseSessionTransport(
        "",
        {
            mode: "connect",
            credentials: { authToken: "auth" },
        },
        resume,
        () => {},
        fetcher,
    );
    const frame = (text: string) => controller.enqueue(new TextEncoder().encode(text));
    const deliver = (sequence: bigint, kind = ClientEventUnion.DisconnectEvent) => {
        const data = btoa(String.fromCharCode(...eventBytes(sequence, kind)));
        frame(`id: ${streamId}:${sequence}\nevent: delivery\ndata: ${data}\n\n`);
    };
    cleanups.push(() => {
        transport.close();
        try {
            controller.close();
        } catch { /* already cancelled */ }
    });
    return { transport, acks, requests, deliver, frame, controller, resume, fetcher };
}

it("parses split UTF-8 frames, CRLF, comments, and multiple data lines", async () => {
    const bytes = new TextEncoder().encode(
        ":keepalive\r\n\r\nid: x\r\nevent: delivery\r\ndata: hé\r\ndata: two\r\n\r\n",
    );
    const input = new ReadableStream<Uint8Array>({
        start(c) {
            for (const byte of bytes) c.enqueue(Uint8Array.of(byte));
            c.close();
        },
    });
    const events = [];
    for await (const event of readSse(input)) events.push(event);
    expect(events).toEqual([{ id: "x", event: "delivery", data: "hé\ntwo" }]);
});

it("sends structured actions with their context instead of using them as prompt replies", async () => {
    const f = fixture();
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.resume.inputRequests.push("prompt");
    const action = {
        kind: "command",
        command: "go east",
        exit: { source: "oid:12", destination: "oid:13", passage: "door" },
    };
    f.transport.send(encodeAction(action));
    expect(f.requests).not.toContain("/v1/session/command");
    expect(f.resume.inputRequests).toEqual(["prompt"]);
    f.transport.send("the prompt answer");
    await vi.waitFor(() => expect(f.requests).toContain("/v1/session/command"));
    const request = f.fetcher.mock.calls.find(([path]) => path === "/v1/session/command")![1]!;
    expect(request.headers).toMatchObject({ "Content-Type": "application/json", "X-Moor-Client-Token": "token" });
    expect(JSON.parse(new TextDecoder().decode(request.body as Uint8Array))).toEqual(action);
    const reply = f.fetcher.mock.calls.find(([path]) => path === "/v1/session/input/prompt")![1]!;
    expect(reply.body).toBe("the prompt answer");
    expect(f.resume.inputRequests).toEqual([]);
});

it("delivers the exact FlatBuffer without a payload GET and ACKs only after processing", async () => {
    const f = fixture();
    let release!: () => void;
    const gate = new Promise<void>(resolve => {
        release = resolve;
    });
    const received: Uint8Array[] = [];
    f.transport.onmessage = async event => {
        received.push(new Uint8Array(event.data));
        await gate;
    };
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.deliver(1n);
    f.deliver(2n);
    await vi.waitFor(() => expect(received).toHaveLength(1));
    expect(f.acks).toEqual([]);
    expect(f.resume.sequence).toBe(0n);
    expect(f.requests).toEqual(["/v1/session", `/v1/events/stream?stream_id=${streamId}&after=0`]);
    release();
    await vi.waitFor(() => expect(f.acks).toEqual(["2"]));
    expect(received).toEqual([eventBytes(1n), eventBytes(2n)]);
    expect(f.requests).toHaveLength(3);
});

it("does not acknowledge a handler failure and retries from the processed cursor", async () => {
    const f = fixture();
    f.transport.onmessage = () => {
        throw new Error("cannot apply");
    };
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.deliver(1n);
    await vi.waitFor(() => expect(f.transport.readyState).toBe(3));
    expect(f.acks).toEqual([]);
    expect(f.resume.sequence).toBe(0n);
    const retry = fixture(f.resume);
    retry.transport.onmessage = vi.fn();
    await vi.waitFor(() => expect(retry.transport.readyState).toBe(1));
    retry.deliver(1n);
    await vi.waitFor(() => expect(retry.acks).toEqual(["1"]));
    expect(retry.requests[1]).toContain("after=0");
});

it("retains exact 64-bit sequence values in resume, delivery, and ACK", async () => {
    const start = 9007199254740993n;
    const f = fixture({ streamId, sequence: start, inputRequests: [], pendingTask: false });
    f.transport.onmessage = vi.fn();
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.deliver(start + 1n);
    await vi.waitFor(() => expect(f.acks).toEqual([(start + 1n).toString()]));
    expect(f.requests[1]).toContain(`after=${start}`);
});

it("rejects gaps without ACK and requests fresh-session recovery", async () => {
    const f = fixture();
    const closed = vi.fn();
    f.transport.onclose = closed;
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.deliver(2n);
    await vi.waitFor(() => expect(closed).toHaveBeenCalled());
    expect(closed.mock.calls[0][0].code).toBe(4009);
    expect(f.acks).toEqual([]);
    expect(f.resume.fresh).toBe(true);
    expect(f.requests).toContain("/auth/logout");
});

it("renews liveness while idle without acknowledging heartbeat watermarks", async () => {
    vi.useFakeTimers();
    const f = fixture();
    await vi.advanceTimersByTimeAsync(1);
    expect(f.transport.readyState).toBe(1);
    f.frame("event: heartbeat\ndata: 99\n\n");
    await vi.advanceTimersByTimeAsync(10000);
    expect(f.acks).toEqual(["0"]);
});

it("continues delivery and releases queued commands while an ACK response is delayed", async () => {
    vi.useFakeTimers();
    const f = fixture();
    const original = f.fetcher.getMockImplementation()!;
    let releaseAck!: (response: Response) => void;
    const ackResponse = new Promise<Response>(resolve => {
        releaseAck = resolve;
    });
    const acknowledgements: string[] = [];
    f.fetcher.mockImplementation(async (input, init) => {
        if (String(input) === "/v1/events/ack") {
            acknowledgements.push(JSON.parse(String(init?.body)).sequence);
            return ackResponse;
        }
        return original(input, init);
    });
    f.transport.onmessage = vi.fn();
    await vi.advanceTimersByTimeAsync(1);
    f.transport.send("first");
    f.transport.send("second");
    f.deliver(1n);
    await vi.advanceTimersByTimeAsync(101);
    expect(acknowledgements).toEqual(["1"]);
    expect(f.requests.filter(path => path === "/v1/session/command")).toHaveLength(1);
    f.deliver(2n, ClientEventUnion.TaskSuspendedEvent);
    await vi.advanceTimersByTimeAsync(1);
    expect(f.resume.sequence).toBe(2n);
    expect(f.transport.onmessage).toHaveBeenCalledTimes(2);
    expect(f.requests.filter(path => path === "/v1/session/command")).toHaveLength(2);
    expect(acknowledgements).toEqual(["1"]);
    releaseAck(new Response(null, { status: 204 }));
    await vi.advanceTimersByTimeAsync(101);
    expect(acknowledgements).toEqual(["1", "2"]);
});

it("coalesces a burst into one cumulative ACK with no payload requests", async () => {
    vi.useFakeTimers();
    const f = fixture();
    f.transport.onmessage = vi.fn();
    await vi.advanceTimersByTimeAsync(1);
    f.deliver(1n);
    f.deliver(2n);
    f.deliver(3n);
    await vi.advanceTimersByTimeAsync(1);
    expect(f.resume.sequence).toBe(3n);
    expect(f.acks).toEqual([]);
    expect(f.requests).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(101);
    expect(f.acks).toEqual(["3"]);
    expect(f.requests).toEqual(["/v1/session", `/v1/events/stream?stream_id=${streamId}&after=0`, "/v1/events/ack"]);
});

it("sends connection input through HTTP and waits for task completion before the next command", async () => {
    const f = fixture();
    f.transport.onmessage = vi.fn();
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    f.transport.send("ask");
    await vi.waitFor(() => expect(f.requests.filter(path => path === "/v1/session/command")).toHaveLength(1));
    f.deliver(1n, ClientEventUnion.RequestInputEvent);
    await vi.waitFor(() => expect(f.acks).toEqual(["1"]));
    f.transport.send("answer");
    await vi.waitFor(() => expect(f.requests).toContain("/v1/session/input/22222222-2222-2222-2222-222222222222"));
    f.transport.send("next command");
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(f.requests.filter(path => path === "/v1/session/command")).toHaveLength(1);
    expect(f.resume.inputRequests).toEqual([]);
});

it.each(["invalid-base64", "wrong-generation"])("rejects %s without ACK", async mode => {
    const f = fixture();
    f.transport.onmessage = vi.fn();
    await vi.waitFor(() => expect(f.transport.readyState).toBe(1));
    const data = mode === "invalid-base64" ? "!" : btoa(String.fromCharCode(...eventBytes(1n)));
    f.frame(`event: delivery\nid: other:1\ndata: ${data}\n\n`);
    await vi.waitFor(() => expect(f.transport.readyState).toBe(3));
    expect(f.acks).toEqual([]);
    expect(f.resume.sequence).toBe(0n);
    expect(f.transport.onmessage).not.toHaveBeenCalled();
});

it("accepts payload frames larger than 8 KiB across network chunks", async () => {
    const data = "x".repeat(32768);
    const input = new ReadableStream<Uint8Array>({
        start(c) {
            const bytes = new TextEncoder().encode(`event: delivery\ndata: ${data}\n\n`);
            for (let i = 0; i < bytes.length; i += 1024) c.enqueue(bytes.slice(i, i + 1024));
            c.close();
        },
    });
    const frames = [];
    for await (const frame of readSse(input)) frames.push(frame);
    expect(frames).toEqual([{ event: "delivery", data, id: undefined }]);
});
