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

import { DataEvent } from "@moor/schema/generated/moor-common/data-event";
import { Error as FbError } from "@moor/schema/generated/moor-common/error";
import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { Event } from "@moor/schema/generated/moor-common/event";
import { EventMetadata } from "@moor/schema/generated/moor-common/event-metadata";
import { EventUnion } from "@moor/schema/generated/moor-common/event-union";
import { Exception } from "@moor/schema/generated/moor-common/exception";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { NotifyEvent } from "@moor/schema/generated/moor-common/notify-event";
import { Obj } from "@moor/schema/generated/moor-common/obj";
import { ObjId } from "@moor/schema/generated/moor-common/obj-id";
import { ObjUnion } from "@moor/schema/generated/moor-common/obj-union";
import { PresentEvent } from "@moor/schema/generated/moor-common/present-event";
import { Presentation } from "@moor/schema/generated/moor-common/presentation";
import { Symbol as FbSymbol } from "@moor/schema/generated/moor-common/symbol";
import { TracebackEvent } from "@moor/schema/generated/moor-common/traceback-event";
import { UnpresentEvent } from "@moor/schema/generated/moor-common/unpresent-event";
import { Uuid } from "@moor/schema/generated/moor-common/uuid";
import { ClientEvent } from "@moor/schema/generated/moor-rpc/client-event";
import { ClientEventUnion } from "@moor/schema/generated/moor-rpc/client-event-union";
import { ClientSuccess } from "@moor/schema/generated/moor-rpc/client-success";
import { DaemonToClientReply } from "@moor/schema/generated/moor-rpc/daemon-to-client-reply";
import { DaemonToClientReplyUnion } from "@moor/schema/generated/moor-rpc/daemon-to-client-reply-union";
import { HistoricalNarrativeEvent } from "@moor/schema/generated/moor-rpc/historical-narrative-event";
import { HistoryResponse } from "@moor/schema/generated/moor-rpc/history-response";
import { HistoryResponseReply } from "@moor/schema/generated/moor-rpc/history-response-reply";
import { NarrativeEventMessage } from "@moor/schema/generated/moor-rpc/narrative-event-message";
import { ReplyResult } from "@moor/schema/generated/moor-rpc/reply-result";
import { ReplyResultUnion } from "@moor/schema/generated/moor-rpc/reply-result-union";
import { Var } from "@moor/schema/generated/moor-var/var";
import { VarInt } from "@moor/schema/generated/moor-var/var-int";
import { VarList } from "@moor/schema/generated/moor-var/var-list";
import { VarMap } from "@moor/schema/generated/moor-var/var-map";
import { VarMapPair } from "@moor/schema/generated/moor-var/var-map-pair";
import { VarStr } from "@moor/schema/generated/moor-var/var-str";
import { VarUnion } from "@moor/schema/generated/moor-var/var-union";
import {
    MoorVar,
    parseHistoricalNarrativeEvent,
    parseNarrativeEvent,
    parsePresentationValue,
    parseWsNarrativeEventMessage,
} from "@moor/web-sdk";
import { Encrypter, generateIdentity, identityToRecipient } from "age-encryption";
import { Builder, ByteBuffer } from "flatbuffers";
import { afterEach, expect, it, vi } from "vitest";
import { fetchHistoryFlatBuffer } from "./rpc-fb-history";
import { moorApi } from "./rpc-fb-shared";
import { handleClientEventFlatBuffer } from "./rpc-fb-ws";

type Value = string | number | Value[] | { [key: string]: Value };
function encode(b: Builder, value: Value): number {
    if (typeof value === "string") {
        return Var.createVar(b, VarUnion.VarStr, VarStr.createVarStr(b, b.createString(value)));
    }
    if (typeof value === "number") return Var.createVar(b, VarUnion.VarInt, VarInt.createVarInt(b, BigInt(value)));
    if (Array.isArray(value)) {
        const entries = value.map(item => encode(b, item));
        return Var.createVar(b, VarUnion.VarList, VarList.createVarList(b, VarList.createElementsVector(b, entries)));
    }
    const pairs = Object.entries(value).map(([key, item]) => {
        const k = encode(b, key);
        const v = encode(b, item);
        VarMapPair.startVarMapPair(b);
        VarMapPair.addKey(b, k);
        VarMapPair.addValue(b, v);
        return VarMapPair.endVarMapPair(b);
    });
    return Var.createVar(b, VarUnion.VarMap, VarMap.createVarMap(b, VarMap.createPairsVector(b, pairs)));
}
const symbol = (b: Builder, text: string) => FbSymbol.createSymbol(b, b.createString(text));
const decode = (value: Var): unknown => new MoorVar(value).toJS();
const string = (value: Var) => new MoorVar(value).asString();

function wire(type: EventUnion, payload: (b: Builder) => number) {
    const b = new Builder(1024);
    const event = Event.createEvent(b, type, payload(b));
    const id = Uuid.createUuid(b, Uuid.createDataVector(b, new Uint8Array(16).fill(1)));
    const author = encode(b, "author");
    NarrativeEvent.startNarrativeEvent(b);
    NarrativeEvent.addEventId(b, id);
    NarrativeEvent.addAuthor(b, author);
    NarrativeEvent.addEvent(b, event);
    const narrative = NarrativeEvent.endNarrativeEvent(b);
    b.finish(narrative);
    const historyBytes = b.asUint8Array().slice();
    const player = Obj.createObj(b, ObjUnion.ObjId, ObjId.createObjId(b, 1));
    NarrativeEventMessage.startNarrativeEventMessage(b);
    NarrativeEventMessage.addPlayer(b, player);
    NarrativeEventMessage.addEvent(b, narrative);
    const message = NarrativeEventMessage.endNarrativeEventMessage(b);
    b.finish(message);
    const live = NarrativeEventMessage.getRootAsNarrativeEventMessage(new ByteBuffer(b.asUint8Array().slice()));
    b.finish(ClientEvent.createClientEvent(b, ClientEventUnion.NarrativeEventMessage, message, 1n));
    return { live, bytes: b.asUint8Array().slice(), historyBytes };
}
function notify(type: string | null, content: Value = "hello", metadata: Record<string, Value> = {}) {
    return wire(EventUnion.NotifyEvent, b => {
        const value = encode(b, content);
        const ct = type === null ? 0 : symbol(b, type);
        const pairs = Object.entries(metadata).map(([key, item]) => {
            const k = symbol(b, key);
            const v = encode(b, item);
            EventMetadata.startEventMetadata(b);
            EventMetadata.addKey(b, k);
            EventMetadata.addValue(b, v);
            return EventMetadata.endEventMetadata(b);
        });
        const vector = NotifyEvent.createMetadataVector(b, pairs);
        NotifyEvent.startNotifyEvent(b);
        NotifyEvent.addValue(b, value);
        NotifyEvent.addContentType(b, ct);
        NotifyEvent.addMetadata(b, vector);
        return NotifyEvent.endNotifyEvent(b);
    });
}
function parseBoth(live: NarrativeEventMessage) {
    const parsed = parseWsNarrativeEventMessage(live, decode, string);
    expect(parseHistoricalNarrativeEvent(live.event(), decode, string)).toEqual(parsed);
    return parsed;
}
afterEach(() => vi.restoreAllMocks());

it.each([
    [null, "text/plain"],
    ["text_plain", "text/plain"],
    ["text/plain", "text/plain"],
    ["text_djot", "text/djot"],
    ["text/djot", "text/djot"],
    ["text_html", "text/html"],
    ["text/html", "text/html"],
    ["text_x_uri", "text/x-uri"],
    ["text/x-uri", "text/x-uri"],
])("normalizes %s consistently for live, history, and captured output", (wireType, expected) => {
    const { live, bytes } = notify(wireType, ["first", "second"]);
    expect(parseBoth(live)).toMatchObject({ kind: "notify", content: ["first", "second"], contentType: expected });
    expect(parseNarrativeEvent(live.event(), decode, string)).toMatchObject({
        eventType: "NotifyEvent",
        event: { contentType: expected },
    });
    const onNarrativeMessage = vi.fn();
    handleClientEventFlatBuffer(bytes, { onNarrativeMessage });
    expect(onNarrativeMessage).toHaveBeenCalledOnce();
    expect(onNarrativeMessage.mock.calls[0].slice(0, 3)).toEqual([["first", "second"], expect.any(String), expected]);
});

it.each(["application/json", "text/unsupported", "", "text/traceback"])(
    "rejects unsupported notification type %s",
    type => {
        const { live, bytes } = notify(type);
        expect(parseBoth(live)).toBeNull();
        expect(parseNarrativeEvent(live.event(), decode, string)).toBeNull();
        const onNarrativeMessage = vi.fn();
        vi.spyOn(console, "warn").mockImplementation(() => {});
        handleClientEventFlatBuffer(bytes, { onNarrativeMessage });
        expect(onNarrativeMessage).not.toHaveBeenCalled();
    },
);

it.each([42, { unexpected: "object" }, ["text", 42], [["nested"]]].map(content => ({ content })))(
    "rejects malformed notification content $content",
    ({ content }) => {
        const { live } = notify("text/plain", content);
        expect(parseBoth(live)).toBeNull();
    },
);

it("decodes notification metadata once and ignores malformed optional values", () => {
    const { live, bytes } = notify("text/plain", "hello", {
        look_kind: "room",
        look_room: { oid: 12 },
        delivery_id: "delivery-1",
        link_preview: { url: "https://example.com", title: 42, image: ["wrong"] },
        rewritable_id: "rewrite-1",
        rewritable_owner: { uuid: "not-a-number" },
        rewritable_ttl: 30,
    });
    const decoder = vi.fn(decode);
    const parsed = parseWsNarrativeEventMessage(live, decoder, string);
    expect(decoder).toHaveBeenCalledTimes(8);
    expect(parsed).toMatchObject({
        kind: "notify",
        eventMeta: { lookKind: "room", lookRoom: { oid: 12 }, deliveryId: "delivery-1" },
        linkPreview: { url: "https://example.com" },
    });
    if (parsed?.kind !== "notify") throw new Error("missing notify");
    expect(parsed.linkPreview?.title).toBeUndefined();
    expect(parsed.linkPreview?.image).toBeUndefined();
    expect(parsed.rewritable).toBeUndefined();
    const onNarrativeMessage = vi.fn();
    handleClientEventFlatBuffer(bytes, { onNarrativeMessage });
    expect(onNarrativeMessage.mock.calls[0][10]).toMatchObject({
        lookRoom: { oid: 12 },
        delivery_id: "delivery-1",
        eventId: "01".repeat(16),
    });
    expect(parseBoth(live)).toEqual(parsed);
});

it("dispatches typed data events without a second decoding path", () => {
    const { live, bytes } = wire(EventUnion.DataEvent, b => {
        const domain = symbol(b, "state");
        const kind = symbol(b, "room_snapshot");
        const value = encode(b, { title: "Room" });
        DataEvent.startDataEvent(b);
        DataEvent.addDomain(b, domain);
        DataEvent.addKind(b, kind);
        DataEvent.addPayload(b, value);
        return DataEvent.endDataEvent(b);
    });
    const expected = { namespace: "state", eventKind: "room_snapshot", payload: { title: "Room" } };
    expect(parseBoth(live)).toEqual({ kind: "data", ...expected });
    const onDataMessage = vi.fn();
    const onNarrativeMessage = vi.fn();
    handleClientEventFlatBuffer(bytes, { onDataMessage, onNarrativeMessage });
    expect(onDataMessage).toHaveBeenCalledExactlyOnceWith({
        ...expected,
        timestamp: expect.any(String),
        eventId: "01".repeat(16),
    });
    expect(onNarrativeMessage).not.toHaveBeenCalled();
});

it.each([
    EventUnion.NotifyEvent,
    EventUnion.TracebackEvent,
    EventUnion.DataEvent,
    EventUnion.PresentEvent,
    EventUnion.UnpresentEvent,
    255,
])("rejects missing payload fields and unknown event tag %i", type => {
    const { live } = wire(type as EventUnion, b => {
        b.startObject(0);
        return b.endObject();
    });
    expect(parseBoth(live)).toBeNull();
});

it("uses generated error accessors for traceback events", () => {
    const { live } = wire(EventUnion.TracebackEvent, b => {
        const message = b.createString("No permission");
        FbError.startError(b);
        FbError.addErrType(b, ErrorCode.E_PERM);
        FbError.addMsg(b, message);
        const error = FbError.endError(b);
        const stack = Exception.createStackVector(b, []);
        const backtrace = Exception.createBacktraceVector(b, [encode(b, "line 1")]);
        const exception = Exception.createException(b, error, stack, backtrace);
        return TracebackEvent.createTracebackEvent(b, exception);
    });
    expect(parseBoth(live)).toMatchObject({
        kind: "traceback",
        tracebackText: "line 1",
        error: { code: "E_PERM", message: "No permission" },
    });
    expect(parseNarrativeEvent(live.event(), decode, string)).toEqual({
        eventType: "TracebackEvent",
        event: { backtrace: ["line 1"], error: { code: "E_PERM", message: "No permission" } },
    });
});

it.each(["text/html", "application/json", "text/x-uri", ""])("validates presentation type %s in every path", type => {
    const { live } = wire(EventUnion.PresentEvent, b => {
        const attrs = Presentation.createAttributesVector(b, []);
        const panel = Presentation.createPresentation(
            b,
            b.createString("panel"),
            b.createString(type),
            b.createString("hello"),
            b.createString("tools"),
            attrs,
        );
        return PresentEvent.createPresentEvent(b, panel);
    });
    const parsed = parseBoth(live);
    if (type === "text/html") expect(parsed).toMatchObject({ kind: "present", presentData: { content_type: type } });
    else expect(parsed).toBeNull();
    const event = live.event()!.event()!;
    // Read the generated accessor only after checking the union tag above.
    const present: PresentEvent = event.event(new PresentEvent());
    expect(parsePresentationValue(present.presentation()) === null).toBe(type !== "text/html");
});

it("preserves unpresent IDs", () => {
    const { live } = wire(
        EventUnion.UnpresentEvent,
        b => UnpresentEvent.createUnpresentEvent(b, b.createString("panel")),
    );
    expect(parseBoth(live)).toEqual({ kind: "unpresent", presentationId: "panel" });
});

it("returns validated history payloads while retaining pagination across rejected events", async () => {
    const identity = await generateIdentity();
    const encrypter = new Encrypter();
    encrypter.addRecipient(await identityToRecipient(identity));
    const valid = notify("text_x_uri", "https://example.com");
    const invalid = notify("application/json");
    const blobs = await Promise.all([valid, invalid].map(event => encrypter.encrypt(event.historyBytes)));
    const b = new Builder(1024);
    const player = Obj.createObj(b, ObjUnion.ObjId, ObjId.createObjId(b, 1));
    const id = Uuid.createUuid(b, Uuid.createDataVector(b, new Uint8Array(16).fill(2)));
    const events = blobs.map(blob => {
        const vector = HistoricalNarrativeEvent.createEncryptedBlobVector(b, blob);
        HistoricalNarrativeEvent.startHistoricalNarrativeEvent(b);
        HistoricalNarrativeEvent.addPlayer(b, player);
        HistoricalNarrativeEvent.addEventId(b, id);
        HistoricalNarrativeEvent.addEncryptedBlob(b, vector);
        HistoricalNarrativeEvent.addIsHistorical(b, true);
        return HistoricalNarrativeEvent.endHistoricalNarrativeEvent(b);
    });
    const vector = HistoryResponse.createEventsVector(b, events);
    HistoryResponse.startHistoryResponse(b);
    HistoryResponse.addEvents(b, vector);
    HistoryResponse.addEarliestEventId(b, id);
    HistoryResponse.addHasMoreBefore(b, true);
    const reply = HistoryResponseReply.createHistoryResponseReply(b, HistoryResponse.endHistoryResponse(b));
    const daemon = DaemonToClientReply.createDaemonToClientReply(
        b,
        DaemonToClientReplyUnion.HistoryResponseReply,
        reply,
    );
    b.finish(
        ReplyResult.createReplyResult(b, ReplyResultUnion.ClientSuccess, ClientSuccess.createClientSuccess(b, daemon)),
    );
    vi.spyOn(moorApi, "getFlatBuffer").mockResolvedValue(b.asUint8Array());
    const page = await fetchHistoryFlatBuffer("token", identity);
    expect(page.eventCount).toBe(2);
    expect(page.hasMoreBefore).toBe(true);
    expect(page.earliestEventId).toBe("02".repeat(16));
    expect(page.events).toHaveLength(1);
    expect(page.events[0].event).toEqual(parseBoth(valid.live));
    expect(page.events[0].event).toMatchObject({ kind: "notify", contentType: "text/x-uri" });
});

it.each<{ owner: Value; expected: string }>([{ owner: { oid: 7 }, expected: "oid:7" }, {
    owner: { uuid: "1" },
    expected: "uuid:000000-0000000001",
}])(
    "retains valid rewrite owners $expected",
    ({ owner, expected }) => {
        const { live } = notify("text/plain", "hello", {
            rewritable_id: "r",
            rewritable_owner: owner,
            rewritable_ttl: 30,
            link_preview: { url: "https://example.com", title: "Example" },
        });
        expect(parseBoth(live)).toMatchObject({
            rewritable: { id: "r", owner: expected, ttl: 30 },
            linkPreview: { title: "Example" },
        });
    },
);

it("carries the same opaque Cowbell document through live delivery and historical replay", () => {
    const document: Value = {
        version: 1,
        blocks: [{ kind: "paragraph", children: [{ kind: "strong", children: [{ kind: "text", text: "brass" }] }] }],
    };
    const { live: wireLive, historyBytes } = notify("text/html", "<p><strong>brass</strong></p>", {
        cowbell_document: document,
        annotations: {},
    });
    const live = parseWsNarrativeEventMessage(wireLive, decode, string);
    if (live?.kind !== "notify") throw new Error("notification missing");
    const history = parseHistoricalNarrativeEvent(
        NarrativeEvent.getRootAsNarrativeEvent(new ByteBuffer(historyBytes)),
        decode,
        string,
    );
    if (history?.kind !== "notify") throw new Error("history missing");
    expect(live.eventMeta?.cowbellDocument).toEqual(document);
    expect(history.eventMeta?.cowbellDocument).toEqual(document);
    expect(history.content).toBe(live.content);
    expect(history.eventMeta?.annotations).toEqual(live.eventMeta?.annotations);
});
