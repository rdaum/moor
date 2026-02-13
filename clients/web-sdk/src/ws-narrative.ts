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

import { EventUnion } from "@moor/schema/generated/moor-common/event-union";
import { NotifyEvent } from "@moor/schema/generated/moor-common/notify-event";
import { PresentEvent } from "@moor/schema/generated/moor-common/present-event";
import { TracebackEvent } from "@moor/schema/generated/moor-common/traceback-event";
import { UnpresentEvent } from "@moor/schema/generated/moor-common/unpresent-event";
import { NarrativeEventMessage } from "@moor/schema/generated/moor-rpc/narrative-event-message";

import { uuObjIdToString } from "./curie";
import { parsePresentationValue } from "./presentations";

export interface WsEventMetadata {
    verb?: string;
    actor?: any;
    actorName?: string;
    content?: string;
    thisObj?: any;
    thisName?: string;
    dobj?: any;
    dobjName?: string;
    iobj?: any;
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
    content: unknown;
    contentType: string;
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
    presentData: {
        id: string | null;
        content: string | null;
        content_type: string;
        target: string | null;
        attributes: Array<[string, string]>;
    };
}

export interface WsUnpresentEvent {
    kind: "unpresent";
    presentationId: string | null;
}

export interface WsTracebackEvent {
    kind: "traceback";
    tracebackText: string;
}

export type ParsedWsNarrativeEvent = WsNotifyEvent | WsPresentEvent | WsUnpresentEvent | WsTracebackEvent;

function normalizeContentType(contentType: string | null): string {
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

export function parseWsNarrativeEventMessage(
    narrative: NarrativeEventMessage,
    decodeVarToJs: (value: unknown) => unknown,
    decodeVarToString: (value: unknown) => string | null,
): ParsedWsNarrativeEvent | null {
    const event = narrative.event();
    if (!event) {
        return null;
    }

    const eventData = event.event();
    if (!eventData) {
        return null;
    }

    const innerEventType = eventData.eventType();
    switch (innerEventType) {
        case EventUnion.NotifyEvent: {
            const notify = eventData.event(new NotifyEvent()) as NotifyEvent | null;
            if (!notify) {
                return null;
            }

            const value = notify.value();
            if (!value) {
                return null;
            }

            const content = decodeVarToJs(value);
            const contentTypeSym = notify.contentType();
            const contentType = normalizeContentType(contentTypeSym ? contentTypeSym.value() : null);
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

                if (keyValue === "presentation_hint" && typeof decoded === "string") {
                    presentationHint = decoded;
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
                } else if (keyValue === "timestamp" && typeof decoded === "number") {
                    eventMeta.timestamp = decoded;
                } else if (keyValue === "link_preview" && typeof decoded === "object" && decoded !== null) {
                    const link = decoded as any;
                    linkPreview = {
                        url: link.url || "",
                        title: link.title || undefined,
                        description: link.description || undefined,
                        image: link.image || undefined,
                        site_name: link.site_name || undefined,
                    };
                } else if (keyValue === "rewritable_id" && typeof decoded === "string") {
                    rewritableId = decoded;
                } else if (keyValue === "rewritable_owner" && decoded && typeof decoded === "object") {
                    const ref = decoded as any;
                    if (ref.oid !== undefined) {
                        rewritableOwner = `oid:${ref.oid}`;
                    } else if (ref.uuid !== undefined) {
                        rewritableOwner = `uuid:${uuObjIdToString(BigInt(ref.uuid))}`;
                    }
                } else if (keyValue === "rewritable_ttl" && typeof decoded === "number") {
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
            const present = eventData.event(new PresentEvent()) as PresentEvent | null;
            if (!present) {
                return null;
            }
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
            const unpresent = eventData.event(new UnpresentEvent()) as UnpresentEvent | null;
            if (!unpresent) {
                return null;
            }
            return {
                kind: "unpresent",
                presentationId: unpresent.presentationId(),
            };
        }
        case EventUnion.TracebackEvent: {
            const traceback = eventData.event(new TracebackEvent()) as TracebackEvent | null;
            if (!traceback) {
                return null;
            }
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
        default:
            return null;
    }
}
