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

import { DataEvent } from "@moor/schema/generated/moor-common/data-event";
import { EventUnion, unionToEventUnion } from "@moor/schema/generated/moor-common/event-union";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { NotifyEvent } from "@moor/schema/generated/moor-common/notify-event";
import { PresentEvent } from "@moor/schema/generated/moor-common/present-event";
import { TracebackEvent } from "@moor/schema/generated/moor-common/traceback-event";
import { UnpresentEvent } from "@moor/schema/generated/moor-common/unpresent-event";
import { HistoryResponseReply } from "@moor/schema/generated/moor-rpc/history-response-reply";
import * as flatbuffers from "flatbuffers";

import { ParsedPresentation, parsePresentationValue } from "./presentations";
import { parseClientReplyUnion } from "./reply";

function replyTypeName(value: unknown): string {
    return (value as any)?.constructor?.name ?? typeof value;
}

export interface EncryptedHistoricalEvent {
    encryptedBlob: Uint8Array;
    isHistorical: boolean;
}

export function parseEncryptedHistoryEvents(bytes: Uint8Array): EncryptedHistoricalEvent[] {
    const replyUnion = parseClientReplyUnion(bytes, "History fetch");
    if (!(replyUnion instanceof HistoryResponseReply)) {
        throw new Error(`Unexpected reply type: ${replyTypeName(replyUnion)}`);
    }

    const historyResponse = replyUnion.response();
    if (!historyResponse) {
        throw new Error("Missing history response");
    }

    const events: EncryptedHistoricalEvent[] = [];
    for (let i = 0; i < historyResponse.eventsLength(); i++) {
        const historicalEvent = historyResponse.events(i);
        if (!historicalEvent) {
            continue;
        }
        const encryptedBlob = historicalEvent.encryptedBlobArray();
        if (!encryptedBlob) {
            continue;
        }
        events.push({
            encryptedBlob,
            isHistorical: historicalEvent.isHistorical(),
        });
    }
    return events;
}

export interface ParsedNarrativeEventEnvelope {
    eventId: string;
    timestampNanos: number;
    event: unknown;
    narrativeEvent: NarrativeEvent;
}

export interface ParsedHistoricalNotifyEvent {
    kind: "notify";
    content: unknown;
    contentType: "text/plain" | "text/djot" | "text/html";
    presentationHint?: string;
    groupId?: string;
    deliveryId?: string;
    thumbnail?: {
        contentType: string;
        data: string;
    };
}

export interface ParsedHistoricalTracebackEvent {
    kind: "traceback";
    tracebackText: string;
}

export interface ParsedHistoricalPresentEvent {
    kind: "present";
    presentation: ParsedPresentation;
}

export interface ParsedHistoricalUnpresentEvent {
    kind: "unpresent";
    presentationId: string;
}

export interface ParsedHistoricalDataEvent {
    kind: "data";
    namespace: string;
    eventKind: string;
    payload: unknown;
}

export type ParsedHistoricalNarrativeEvent =
    | ParsedHistoricalNotifyEvent
    | ParsedHistoricalTracebackEvent
    | ParsedHistoricalPresentEvent
    | ParsedHistoricalUnpresentEvent
    | ParsedHistoricalDataEvent;

function normalizeContentType(contentType: string | null): "text/plain" | "text/djot" | "text/html" {
    if (contentType === "text_djot" || contentType === "text/djot") {
        return "text/djot";
    }
    if (contentType === "text_html" || contentType === "text/html") {
        return "text/html";
    }
    return "text/plain";
}

function bytesToDataUrl(contentType: string, bytes: Uint8Array): string {
    let binary = "";
    for (let i = 0; i < bytes.length; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    return `data:${contentType};base64,${btoa(binary)}`;
}

export function parseHistoricalNarrativeEvent(
    narrativeEvent: NarrativeEvent,
    decodeVarToJs: (value: unknown) => unknown,
    decodeVarToString: (value: unknown) => string | null,
): ParsedHistoricalNarrativeEvent | null {
    const eventData = narrativeEvent.event();
    if (!eventData) {
        return null;
    }
    const eventType = eventData.eventType();
    const eventUnion = unionToEventUnion(eventType, (obj) => eventData.event(obj));
    if (!eventUnion) {
        return null;
    }

    switch (eventType) {
        case EventUnion.NotifyEvent: {
            const notify = eventUnion as NotifyEvent;
            const value = notify.value();
            if (!value) {
                return null;
            }

            let presentationHint: string | undefined;
            let groupId: string | undefined;
            let deliveryId: string | undefined;
            let thumbnail: { contentType: string; data: string } | undefined;

            const metadataLength = notify.metadataLength();
            for (let i = 0; i < metadataLength; i++) {
                const metadata = notify.metadata(i);
                if (!metadata) {
                    continue;
                }
                const key = metadata.key();
                const keyValue = key ? key.value() : null;
                const metadataValue = metadata.value();
                const decoded = metadataValue ? decodeVarToJs(metadataValue) : null;

                if (keyValue === "presentation_hint" && typeof decoded === "string") {
                    presentationHint = decoded;
                } else if (keyValue === "group_id" && typeof decoded === "string") {
                    groupId = decoded;
                } else if (keyValue === "delivery_id" && typeof decoded === "string") {
                    deliveryId = decoded;
                } else if (keyValue === "thumbnail" && Array.isArray(decoded) && decoded.length === 2) {
                    const thumbContentType = decoded[0];
                    const binaryData = decoded[1];
                    if (typeof thumbContentType === "string" && binaryData instanceof Uint8Array) {
                        thumbnail = {
                            contentType: thumbContentType,
                            data: bytesToDataUrl(thumbContentType, binaryData),
                        };
                    }
                }
            }

            return {
                kind: "notify",
                content: decodeVarToJs(value),
                contentType: normalizeContentType(notify.contentType()?.value() || null),
                presentationHint,
                groupId,
                deliveryId,
                thumbnail,
            };
        }
        case EventUnion.TracebackEvent: {
            const traceback = eventUnion as TracebackEvent;
            const exception = traceback.exception();
            if (!exception) {
                return null;
            }
            const tracebackLines: string[] = [];
            for (let i = 0; i < exception.backtraceLength(); i++) {
                const backtraceVar = exception.backtrace(i);
                if (!backtraceVar) {
                    continue;
                }
                const line = decodeVarToString(backtraceVar);
                if (line) {
                    tracebackLines.push(line);
                }
            }
            return {
                kind: "traceback",
                tracebackText: tracebackLines.join("\n"),
            };
        }
        case EventUnion.PresentEvent: {
            const present = eventUnion as PresentEvent;
            const parsedPresentation = parsePresentationValue(
                present?.presentation() ?? null,
                { requireId: true },
            );
            if (!parsedPresentation) {
                return null;
            }
            return {
                kind: "present",
                presentation: parsedPresentation,
            };
        }
        case EventUnion.UnpresentEvent: {
            const unpresent = eventUnion as UnpresentEvent;
            const presentationId = unpresent?.presentationId();
            if (!presentationId) {
                return null;
            }
            return {
                kind: "unpresent",
                presentationId,
            };
        }
        case EventUnion.DataEvent: {
            const data = eventUnion as DataEvent;
            const namespace = data.domain()?.value();
            const eventKind = data.kind()?.value();
            const payloadRef = data.payload();
            if (!namespace || !eventKind || !payloadRef) {
                return null;
            }
            return {
                kind: "data",
                namespace,
                eventKind,
                payload: decodeVarToJs(payloadRef),
            };
        }
        default:
            return null;
    }
}

export function parseNarrativeEventEnvelope(bytes: Uint8Array): ParsedNarrativeEventEnvelope | null {
    const narrativeEvent = NarrativeEvent.getRootAsNarrativeEvent(
        new flatbuffers.ByteBuffer(bytes),
    );

    const eventData = narrativeEvent.event();
    if (!eventData) {
        return null;
    }

    const eventId = narrativeEvent.eventId()?.dataArray();
    const eventIdStr = eventId
        ? Array.from(eventId).map((b: number) => b.toString(16).padStart(2, "0")).join("")
        : "";

    return {
        eventId: eventIdStr,
        timestampNanos: Number(narrativeEvent.timestamp()),
        event: eventData,
        narrativeEvent,
    };
}
