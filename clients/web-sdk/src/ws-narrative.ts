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
import { ErrorCode } from "@moor/schema/generated/moor-common/error-code";
import { EventUnion, unionToEventUnion } from "@moor/schema/generated/moor-common/event-union";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";
import { NotifyEvent } from "@moor/schema/generated/moor-common/notify-event";
import { PresentEvent } from "@moor/schema/generated/moor-common/present-event";
import { TracebackEvent } from "@moor/schema/generated/moor-common/traceback-event";
import { UnpresentEvent } from "@moor/schema/generated/moor-common/unpresent-event";
import { NarrativeEventMessage } from "@moor/schema/generated/moor-rpc/narrative-event-message";
import { Var } from "@moor/schema/generated/moor-var/var";
import { AnnotationTable, decodeAnnotations } from "./annotations.js";
import {
    isNarrativeContent,
    NarrativeContent,
    NarrativeNotifyContentType,
    parseNarrativeContentType,
} from "./content-types.js";

import { uuObjIdToString } from "./curie.js";
import { parsePresentationValue, PresentationData } from "./presentations.js";

export interface WsEventMetadata {
    eventId?: string;
    lookKind?: string;
    lookRoom?: unknown;
    look_room?: unknown;
    deliveryId?: string;
    delivery_id?: string;
    annotations?: AnnotationTable;
    /** Opaque core-owned content; rendered bodies remain the display path. */
    cowbellDocument?: unknown;
    collapseTitle?: string;
    verb?: string;
    actor?: unknown;
    actorName?: string;
    content?: string;
    thisObj?: unknown;
    thisName?: string;
    dobj?: unknown;
    dobjName?: string;
    iobj?: unknown;
    timestamp?: number;
    enableEmojis?: boolean;
}

export interface WsLinkPreview {
    url: string;
    title?: string;
    description?: string;
    image?: string;
    site_name?: string;
}

export interface WsRewritable {
    id: string;
    owner: string;
    ttl: number;
    fallback?: string;
}

export interface WsNotifyEvent {
    kind: "notify";
    content: NarrativeContent;
    contentType: NarrativeNotifyContentType;
    noNewline: boolean;
    presentationHint?: string;
    groupId?: string;
    ttsText?: string;
    thumbnail?: { contentType: string; data: string };
    linkPreview?: WsLinkPreview;
    eventMeta?: WsEventMetadata;
    rewritable?: WsRewritable;
    rewriteTarget?: string;
}

export interface WsPresentEvent {
    kind: "present";
    presentData: PresentationData;
}

export interface WsUnpresentEvent {
    kind: "unpresent";
    presentationId: string;
}

export interface WsTracebackEvent {
    kind: "traceback";
    tracebackText: string;
    backtrace: string[];
    error: { code: string | null; message: string | null } | null;
}

export interface WsDataEvent {
    kind: "data";
    namespace: string;
    eventKind: string;
    payload: unknown;
}

export type ParsedNarrativePayload =
    | WsNotifyEvent
    | WsPresentEvent
    | WsUnpresentEvent
    | WsTracebackEvent
    | WsDataEvent;

export type ParsedWsNarrativeEvent = ParsedNarrativePayload;

function bytesToDataUrl(contentType: string, bytes: Uint8Array): string {
    let binary = "";
    for (let i = 0; i < bytes.length; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    return `data:${contentType};base64,${btoa(binary)}`;
}

export function parseWsNarrativeEventMessage(
    narrative: NarrativeEventMessage,
    decodeVarToJs: (value: Var) => unknown,
    decodeVarToString: (value: Var) => string | null,
): ParsedNarrativePayload | null {
    return parseNarrativeValue(narrative.event(), decodeVarToJs, decodeVarToString);
}

/** Decode each narrative payload once for live, captured, and historical events. */
export function parseNarrativeValue(
    event: NarrativeEvent | null,
    decodeVarToJs: (value: Var) => unknown,
    decodeVarToString: (value: Var) => string | null,
): ParsedNarrativePayload | null {
    if (!event) {
        return null;
    }

    const eventData = event.event();
    if (!eventData) {
        return null;
    }

    const innerEventType = eventData.eventType();
    const payload = unionToEventUnion(innerEventType, obj => eventData.event(obj));
    switch (innerEventType) {
        case EventUnion.NotifyEvent: {
            if (!(payload instanceof NotifyEvent)) return null;
            const notify = payload;

            const value = notify.value();
            if (!value) {
                return null;
            }

            const content = decodeVarToJs(value);
            const contentTypeSym = notify.contentType();
            const contentType = parseNarrativeContentType(contentTypeSym ? contentTypeSym.value() : null);
            if (!contentType || !isNarrativeContent(content)) return null;
            const noNewline = notify.noNewline();

            let presentationHint: string | undefined;
            let groupId: string | undefined;
            let ttsText: string | undefined;
            let thumbnail: { contentType: string; data: string } | undefined;
            let linkPreview: WsLinkPreview | undefined;
            let rewritableId: string | undefined;
            let rewritableOwner: string | undefined;
            let rewritableTtl: number | undefined;
            let rewritableFallback: string | undefined;
            let rewriteTarget: string | undefined;
            const eventMeta: WsEventMetadata = {};

            const metadataLength = notify.metadataLength();
            for (let i = 0; i < metadataLength; i++) {
                const metadata = notify.metadata(i);
                if (!metadata) {
                    continue;
                }
                const key = metadata.key();
                const keyValue = key ? key.value() : null;
                const metaValue = metadata.value();
                const decoded = metaValue ? decodeVarToJs(metaValue) : null;

                if (keyValue === "look_kind" && typeof decoded === "string") {
                    eventMeta.lookKind = decoded;
                } else if (keyValue === "look_room") {
                    eventMeta.lookRoom = decoded;
                } else if (keyValue === "delivery_id" && typeof decoded === "string") {
                    eventMeta.deliveryId = decoded;
                    eventMeta.delivery_id = decoded;
                } else if (keyValue === "cowbell_document") {
                    eventMeta.cowbellDocument = decoded;
                } else if (keyValue === "annotations") {
                    eventMeta.annotations = decodeAnnotations(decoded);
                } else if (keyValue === "presentation_hint" && typeof decoded === "string") {
                    presentationHint = decoded;
                } else if (keyValue === "collapse_title" && typeof decoded === "string" && decoded.trim()) {
                    eventMeta.collapseTitle = decoded;
                } else if (keyValue === "group_id" && typeof decoded === "string") {
                    groupId = decoded;
                } else if (keyValue === "tts_text" && typeof decoded === "string") {
                    ttsText = decoded;
                } else if (keyValue === "thumbnail" && Array.isArray(decoded) && decoded.length === 2) {
                    const thumbContentType = decoded[0];
                    const binaryData = decoded[1];
                    if (typeof thumbContentType === "string" && binaryData instanceof Uint8Array) {
                        thumbnail = {
                            contentType: thumbContentType,
                            data: bytesToDataUrl(thumbContentType, binaryData),
                        };
                    }
                } else if (keyValue === "verb" && typeof decoded === "string") {
                    eventMeta.verb = decoded;
                } else if (keyValue === "actor") {
                    eventMeta.actor = decoded;
                } else if (keyValue === "actor_name" && typeof decoded === "string") {
                    eventMeta.actorName = decoded;
                } else if (keyValue === "content" && typeof decoded === "string") {
                    eventMeta.content = decoded;
                } else if (keyValue === "this_obj") {
                    eventMeta.thisObj = decoded;
                } else if (keyValue === "this_name" && typeof decoded === "string") {
                    eventMeta.thisName = decoded;
                } else if (keyValue === "dobj") {
                    eventMeta.dobj = decoded;
                } else if (keyValue === "dobj_name" && typeof decoded === "string") {
                    eventMeta.dobjName = decoded;
                } else if (keyValue === "iobj") {
                    eventMeta.iobj = decoded;
                } else if (keyValue === "timestamp" && typeof decoded === "number" && Number.isFinite(decoded)) {
                    eventMeta.timestamp = decoded;
                } else if (keyValue === "link_preview" && typeof decoded === "object" && decoded !== null) {
                    if ("url" in decoded && typeof decoded.url === "string") {
                        linkPreview = {
                            url: decoded.url,
                            title: "title" in decoded && typeof decoded.title === "string" ? decoded.title : undefined,
                            description: "description" in decoded && typeof decoded.description === "string"
                                ? decoded.description
                                : undefined,
                            image: "image" in decoded && typeof decoded.image === "string" ? decoded.image : undefined,
                            site_name: "site_name" in decoded && typeof decoded.site_name === "string"
                                ? decoded.site_name
                                : undefined,
                        };
                    }
                } else if (keyValue === "rewritable_id" && typeof decoded === "string") {
                    rewritableId = decoded;
                } else if (keyValue === "rewritable_owner" && decoded && typeof decoded === "object") {
                    if ("oid" in decoded && typeof decoded.oid === "number" && Number.isSafeInteger(decoded.oid)) {
                        rewritableOwner = `oid:${decoded.oid}`;
                    } else if (
                        "uuid" in decoded && typeof decoded.uuid === "string" && /^\d{1,20}$/.test(decoded.uuid)
                    ) {
                        const packed = BigInt(decoded.uuid);
                        if (packed <= 0xffffffffffffffffn) rewritableOwner = `uuid:${uuObjIdToString(packed)}`;
                    }
                } else if (
                    keyValue === "rewritable_ttl" && typeof decoded === "number" && Number.isFinite(decoded)
                    && decoded >= 0
                ) {
                    rewritableTtl = decoded;
                } else if (keyValue === "rewritable_fallback" && typeof decoded === "string") {
                    rewritableFallback = decoded;
                } else if (keyValue === "rewrite_target" && typeof decoded === "string") {
                    rewriteTarget = decoded;
                } else if (keyValue === "enable_emojis" && typeof decoded === "boolean") {
                    eventMeta.enableEmojis = decoded;
                }
            }

            return {
                kind: "notify",
                content,
                contentType,
                noNewline,
                presentationHint,
                groupId,
                ttsText,
                thumbnail,
                linkPreview,
                eventMeta: Object.keys(eventMeta).length > 0 ? eventMeta : undefined,
                rewritable: rewritableId && rewritableOwner && rewritableTtl !== undefined
                    ? {
                        id: rewritableId,
                        owner: rewritableOwner,
                        ttl: rewritableTtl,
                        fallback: rewritableFallback,
                    }
                    : undefined,
                rewriteTarget,
            };
        }
        case EventUnion.PresentEvent: {
            if (!(payload instanceof PresentEvent)) return null;
            const present = payload;
            const parsedPresentation = parsePresentationValue(present.presentation());
            if (!parsedPresentation) {
                return null;
            }
            return {
                kind: "present",
                presentData: {
                    id: parsedPresentation.id,
                    content: parsedPresentation.content,
                    content_type: parsedPresentation.contentType,
                    target: parsedPresentation.target,
                    attributes: parsedPresentation.attributes,
                },
            };
        }
        case EventUnion.UnpresentEvent: {
            if (!(payload instanceof UnpresentEvent)) return null;
            const unpresent = payload;
            const presentationId = unpresent.presentationId();
            return presentationId ? { kind: "unpresent", presentationId } : null;
        }
        case EventUnion.TracebackEvent: {
            if (!(payload instanceof TracebackEvent)) return null;
            const traceback = payload;
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
            const error = exception.error();
            return {
                kind: "traceback",
                tracebackText: tracebackLines.join("\n"),
                backtrace: tracebackLines,
                error: error
                    ? {
                        code: error.customSymbol()?.value() ?? ErrorCode[error.errType()] ?? null,
                        message: error.msg(),
                    }
                    : null,
            };
        }
        case EventUnion.DataEvent: {
            if (!(payload instanceof DataEvent)) return null;
            const data = payload;
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
