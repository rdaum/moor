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

import { EventUnion, unionToEventUnion } from "@moor/schema/generated/moor-common/event-union";
import { NarrativeEvent } from "@moor/schema/generated/moor-common/narrative-event";

export type NarrativeNotifyContentType = "text/plain" | "text/djot" | "text/html" | "text/x-uri";

export type ParsedNarrativeEvent =
    | {
        eventType: "NotifyEvent";
        event: {
            value: unknown;
            contentType: NarrativeNotifyContentType;
        };
    }
    | {
        eventType: "PresentEvent";
        event: {
            presentation: {
                id: string | null;
                contentType: string | null;
                content: string | null;
                target: string | null;
            } | null;
        };
    }
    | {
        eventType: "UnpresentEvent";
        event: {
            presentationId: string | null;
        };
    }
    | {
        eventType: "TracebackEvent";
        event: {
            error: {
                code: string | null;
                message: string | null;
            } | null;
            backtrace: string[];
        };
    };

function normalizeContentType(contentType: string | null): NarrativeNotifyContentType {
    if (!contentType) {
        return "text/plain";
    }
    if (contentType === "text_djot" || contentType === "text/djot") {
        return "text/djot";
    }
    if (contentType === "text_html" || contentType === "text/html") {
        return "text/html";
    }
    if (contentType === "text_x_uri" || contentType === "text/x-uri") {
        return "text/x-uri";
    }
    return "text/plain";
}

export function parseNarrativeEvent(
    narrativeEvent: NarrativeEvent | null,
    decodeVarToJs: (value: unknown) => unknown,
    decodeVarToString: (value: unknown) => string | null,
): ParsedNarrativeEvent | null {
    if (!narrativeEvent) {
        return null;
    }

    const eventObj = narrativeEvent.event();
    if (!eventObj) {
        return null;
    }

    const eventType = eventObj.eventType();
    const eventUnion = unionToEventUnion(
        eventType,
        (obj: any) => eventObj.event(obj),
    );
    if (!eventUnion) {
        return null;
    }

    switch (eventType) {
        case EventUnion.NotifyEvent: {
            const notifyEvent = eventUnion as any;
            const value = notifyEvent.value();
            const contentTypeSym = notifyEvent.contentType();
            return {
                eventType: "NotifyEvent",
                event: {
                    value: value ? decodeVarToJs(value) : "",
                    contentType: normalizeContentType(contentTypeSym ? contentTypeSym.value() : null),
                },
            };
        }
        case EventUnion.PresentEvent: {
            const presentEvent = eventUnion as any;
            const presentation = presentEvent.presentation();
            return {
                eventType: "PresentEvent",
                event: {
                    presentation: presentation
                        ? {
                            id: presentation.id(),
                            contentType: presentation.contentType(),
                            content: presentation.content(),
                            target: presentation.target(),
                        }
                        : null,
                },
            };
        }
        case EventUnion.UnpresentEvent: {
            const unpresentEvent = eventUnion as any;
            return {
                eventType: "UnpresentEvent",
                event: {
                    presentationId: unpresentEvent.presentationId(),
                },
            };
        }
        case EventUnion.TracebackEvent: {
            const tracebackEvent = eventUnion as any;
            const exception = tracebackEvent.exception();
            let errorInfo: { code: string | null; message: string | null } | null = null;
            const backtrace: string[] = [];
            if (exception) {
                const error = exception.error();
                if (error) {
                    errorInfo = {
                        code: error.code(),
                        message: error.message(),
                    };
                }
                const backtraceLen = exception.backtraceLength();
                for (let i = 0; i < backtraceLen; i++) {
                    const varFb = exception.backtrace(i);
                    if (!varFb) {
                        continue;
                    }
                    const frame = decodeVarToString(varFb);
                    if (frame) {
                        backtrace.push(frame);
                    }
                }
            }
            return {
                eventType: "TracebackEvent",
                event: {
                    error: errorInfo,
                    backtrace,
                },
            };
        }
        default:
            return null;
    }
}
