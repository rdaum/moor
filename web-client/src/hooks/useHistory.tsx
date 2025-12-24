// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

import { useCallback, useState } from "react";
import { NarrativeMessage } from "../components/Narrative";
import { EventUnion } from "../generated/moor-common/event-union";
import { NarrativeEvent } from "../generated/moor-common/narrative-event";
import { NotifyEvent } from "../generated/moor-common/notify-event";
import { TracebackEvent } from "../generated/moor-common/traceback-event";
import { MoorVar } from "../lib/MoorVar";
import { fetchHistoryFlatBuffer } from "../lib/rpc-fb";

// Filter out MCP sequences from historical messages
const filterMCPSequences = (messages: NarrativeMessage[]): NarrativeMessage[] => {
    const filtered: NarrativeMessage[] = [];
    let inMCPSpool = false;

    for (const message of messages) {
        const content = Array.isArray(message.content) ? message.content.join("").trim() : message.content.trim();

        // Filter out ALL MCP messages (anything starting with "#$#")
        if (content.startsWith("#$#")) {
            // Check if this starts an MCP edit sequence
            if (content.startsWith("#$# edit")) {
                inMCPSpool = true;
            }
            continue; // Skip all MCP command lines
        }

        // Check if this ends an MCP spool sequence
        if (inMCPSpool && content === ".") {
            inMCPSpool = false;
            continue; // Skip the terminator
        }

        // Skip any content while we're in an MCP spool
        if (inMCPSpool) {
            continue;
        }

        // Keep all other messages
        filtered.push(message);
    }

    return filtered;
};

type TimestampValue = number | string | { secs_since_epoch?: number; nanos_since_epoch?: number } | null;

const normalizeLegacyContentType = (value?: string): "text/plain" | "text/djot" | "text/html" => {
    if (!value) {
        return "text/plain";
    }
    const normalized = value.replace("_", "/").toLowerCase();
    if (normalized === "text/djot") {
        return "text/djot";
    }
    if (normalized === "text/html") {
        return "text/html";
    }
    return "text/plain";
};

interface HistoricalEvent {
    event_id: string;
    timestamp?: TimestampValue;
    event_type?: string;
    message?: unknown;
    author?: unknown;
    player?: unknown;
    is_historical?: boolean;
    data?: unknown;
    narrative_event?: NarrativeEvent;
    event?: unknown;
}

export const useHistory = (authToken: string | null, encryptionKey: string | null = null) => {
    const [historyBoundary, setHistoryBoundary] = useState<number | null>(null);
    const [earliestHistoryEventId, setEarliestHistoryEventId] = useState<string | null>(null);
    const [isLoadingHistory, setIsLoadingHistory] = useState(false);
    const [shouldShowDisconnectDivider, setShouldShowDisconnectDivider] = useState(false);

    // Set history boundary timestamp to prevent duplicates with WebSocket events
    // If lastMessageBeforeDisconnect is provided and the gap is > 10 minutes, mark to show divider
    const setHistoryBoundaryNow = useCallback((lastMessageBeforeDisconnect?: number) => {
        const boundary = Date.now();
        setHistoryBoundary(boundary);

        // Calculate if we should show divider (gap > 10 minutes = 600000 ms)
        if (lastMessageBeforeDisconnect && lastMessageBeforeDisconnect > 0) {
            const disconnectGapMs = boundary - lastMessageBeforeDisconnect;
            const tenMinutesMs = 10 * 60 * 1000;
            setShouldShowDisconnectDivider(disconnectGapMs > tenMinutesMs);
        } else {
            setShouldShowDisconnectDivider(false);
        }
    }, []);

    // Check if a WebSocket event timestamp is before history boundary (duplicate)
    const isHistoricalDuplicate = useCallback((eventTimestamp: number): boolean => {
        return historyBoundary !== null && eventTimestamp < historyBoundary;
    }, [historyBoundary]);

    const convertFlatBufferHistoricalEvent = useCallback((event: HistoricalEvent): NarrativeMessage | null => {
        try {
            const narrativeEvent = event.narrative_event;
            if (!narrativeEvent) {
                return null;
            }
            const timestamp = typeof event.timestamp === "number" ? event.timestamp : Date.now();
            const eventId = event.event_id;

            const eventData = narrativeEvent.event();
            if (!eventData) {
                return null;
            }

            const eventType = eventData.eventType();

            let messageContent: string | string[] = "";
            let contentType: "text/plain" | "text/djot" | "text/html" | "text/traceback" = "text/plain";
            let presentationHint: string | undefined;
            let groupId: string | undefined;
            let thumbnail: { contentType: string; data: string } | undefined;

            switch (eventType) {
                case EventUnion.NotifyEvent: {
                    const notify = eventData.event(new NotifyEvent());
                    if (!notify) break;

                    const value = notify.value();
                    if (!value) break;

                    // Convert the Var to JavaScript value
                    messageContent = new MoorVar(value).toJS();

                    // Get content type
                    const contentTypeSym = notify.contentType();
                    if (contentTypeSym && contentTypeSym.value()) {
                        const ct = contentTypeSym.value();
                        // Normalize content type
                        if (ct === "text_djot" || ct === "text/djot") {
                            contentType = "text/djot";
                        } else if (ct === "text_html" || ct === "text/html") {
                            contentType = "text/html";
                        } else {
                            contentType = "text/plain";
                        }
                    }

                    // Extract metadata fields directly from top-level entries (same as live handler)
                    const metadataLength = notify.metadataLength();
                    for (let mi = 0; mi < metadataLength; mi++) {
                        const metadata = notify.metadata(mi);
                        if (metadata) {
                            const key = metadata.key();
                            const keyValue = key ? key.value() : null;
                            const metaValue = metadata.value();
                            const value = metaValue ? new MoorVar(metaValue).toJS() : null;

                            if (keyValue === "presentation_hint" && typeof value === "string") {
                                presentationHint = value;
                            } else if (keyValue === "group_id" && typeof value === "string") {
                                groupId = value;
                            } else if (keyValue === "thumbnail" && Array.isArray(value) && value.length === 2) {
                                // thumbnail is [content_type, binary_data]
                                const [contentType, binaryData] = value;
                                if (
                                    typeof contentType === "string"
                                    && binaryData instanceof Uint8Array
                                ) {
                                    // Convert binary data to base64 data URL
                                    const base64 = btoa(String.fromCharCode(...binaryData));
                                    thumbnail = {
                                        contentType,
                                        data: `data:${contentType};base64,${base64}`,
                                    };
                                }
                            }
                        }
                    }
                    break;
                }

                case EventUnion.TracebackEvent: {
                    const traceback = eventData.event(new TracebackEvent());
                    if (!traceback) break;

                    const exception = traceback.exception();
                    if (!exception) break;

                    // Build traceback text from backtrace frames
                    const tracebackLines: string[] = [];
                    for (let i = 0; i < exception.backtraceLength(); i++) {
                        const backtraceVar = exception.backtrace(i);
                        if (backtraceVar) {
                            const line = new MoorVar(backtraceVar).asString();
                            if (line) {
                                tracebackLines.push(line);
                            }
                        }
                    }

                    messageContent = tracebackLines.join("\n");
                    contentType = "text/traceback";
                    break;
                }

                case EventUnion.PresentEvent: {
                    // Presentations are handled separately, skip for now
                    return null;
                }

                case EventUnion.UnpresentEvent: {
                    // Unpresent events are handled separately, skip for now
                    return null;
                }

                default:
                    console.warn(`Unknown event type: ${eventType}`);
                    return null;
            }

            return {
                id: `history_${eventId}_${timestamp}`,
                content: messageContent,
                type: "narrative",
                timestamp,
                isHistorical: true,
                contentType,
                presentationHint,
                groupId,
                thumbnail: thumbnail,
            };
        } catch (error) {
            console.error("Failed to convert FlatBuffer event:", error);
            return null;
        }
    }, []);

    // Convert historical event to narrative message format (all events become narrative)
    const convertHistoricalEvent = useCallback((event: HistoricalEvent): NarrativeMessage | null => {
        if (event.narrative_event) {
            return convertFlatBufferHistoricalEvent(event);
        }

        try {
            let messageContent = "";
            let contentType: "text/plain" | "text/djot" | "text/html" = "text/plain";

            if (event.message && typeof event.message === "object") {
                const msg = event.message as Record<string, unknown>;
                const payloadContent = msg.content;
                switch (msg.type) {
                    case "notify":
                        if (typeof payloadContent === "string") {
                            messageContent = payloadContent;
                        } else if (payloadContent !== undefined) {
                            messageContent = JSON.stringify(payloadContent);
                        }
                        contentType = normalizeLegacyContentType(
                            typeof msg.content_type === "string" ? msg.content_type : undefined,
                        );
                        break;
                    case "traceback":
                        messageContent = `ERROR: ${typeof msg.error === "string" ? msg.error : ""}`;
                        break;
                    case "present":
                        messageContent = `[Presentation: ${
                            typeof msg.presentation === "string" ? msg.presentation : ""
                        }]`;
                        break;
                    case "unpresent":
                        messageContent = `[Closed: ${typeof msg.id === "string" ? msg.id : ""}]`;
                        break;
                    default:
                        if (typeof payloadContent === "string") {
                            messageContent = payloadContent;
                        } else if (payloadContent !== undefined) {
                            messageContent = JSON.stringify(payloadContent);
                        } else {
                            messageContent = JSON.stringify(msg);
                        }
                        contentType = normalizeLegacyContentType(
                            typeof msg.content_type === "string" ? msg.content_type : undefined,
                        );
                        break;
                }
            } else if (typeof event.message === "string") {
                messageContent = event.message;
            } else if (event.data) {
                messageContent = typeof event.data === "string" ? event.data : JSON.stringify(event.data);
            }

            let timestamp = Date.now();
            if (event.timestamp) {
                if (
                    typeof event.timestamp === "object" && event.timestamp !== null
                    && "secs_since_epoch" in event.timestamp
                ) {
                    const ts = event.timestamp as { secs_since_epoch?: number; nanos_since_epoch?: number };
                    if (typeof ts.secs_since_epoch === "number") {
                        timestamp = (ts.secs_since_epoch * 1000) + Math.floor((ts.nanos_since_epoch || 0) / 1000000);
                    }
                } else if (typeof event.timestamp === "string" || typeof event.timestamp === "number") {
                    const parsed = new Date(event.timestamp).getTime();
                    if (!Number.isNaN(parsed)) {
                        timestamp = parsed;
                    }
                }
            }

            return {
                id: `history_${event.event_id}_${timestamp}`,
                content: messageContent,
                type: "narrative",
                timestamp,
                isHistorical: true,
                contentType,
            };
        } catch (error) {
            console.error("Failed to convert legacy history event:", error);
            return null;
        }
    }, [convertFlatBufferHistoricalEvent]);

    // Fetch history from API
    const fetchHistory = useCallback(async (
        limit: number = 100,
        sinceSeconds?: number,
        untilEvent?: string,
    ): Promise<NarrativeMessage[]> => {
        if (!authToken) {
            throw new Error("No auth token available");
        }

        setIsLoadingHistory(true);

        try {
            // Use FlatBuffer endpoint with client-side decryption
            const events = await fetchHistoryFlatBuffer(
                authToken,
                encryptionKey,
                limit,
                sinceSeconds,
                untilEvent,
            );

            // Convert events to narrative messages
            const narrativeMessages: NarrativeMessage[] = [];
            for (const event of events as HistoricalEvent[]) {
                const message = convertHistoricalEvent(event);
                if (message) {
                    narrativeMessages.push(message);
                }
            }

            // Filter out MCP sequences before returning
            const filteredMessages = filterMCPSequences(narrativeMessages);

            // Update earliest event ID for pagination
            if (events.length > 0) {
                setEarliestHistoryEventId((events[0] as HistoricalEvent).event_id);
            }

            return filteredMessages;
        } catch (error) {
            console.error("Failed to fetch more history:", error);
            throw error;
        } finally {
            setIsLoadingHistory(false);
        }
    }, [authToken, convertHistoricalEvent, encryptionKey]);

    // Calculate optimal initial load based on viewport
    const calculateInitialLoad = useCallback(() => {
        // Estimate messages needed to fill viewport + some overflow for scrolling
        const viewportHeight = window.innerHeight;
        const estimatedMessageHeight = 25; // pixels per line of text
        const messagesNeededToFill = Math.ceil(viewportHeight / estimatedMessageHeight);

        // Add 50% more messages to ensure scrollable content
        const initialLoad = Math.min(Math.max(messagesNeededToFill * 1.5, 20), 100);

        return Math.floor(initialLoad);
    }, []);

    // Fetch initial history on connect (dynamically sized based on viewport)
    const fetchInitialHistory = useCallback(async (): Promise<NarrativeMessage[]> => {
        const dynamicLimit = calculateInitialLoad();
        return await fetchHistory(dynamicLimit, 86400); // 24 hours = 86400 seconds
    }, [fetchHistory, calculateInitialLoad]);

    // Fetch more history for infinite scroll
    const fetchMoreHistory = useCallback(async (): Promise<NarrativeMessage[]> => {
        if (!earliestHistoryEventId) {
            return [];
        }
        return await fetchHistory(50, undefined, earliestHistoryEventId);
    }, [fetchHistory, earliestHistoryEventId]);

    return {
        historyBoundary,
        setHistoryBoundaryNow,
        isHistoricalDuplicate,
        fetchInitialHistory,
        fetchMoreHistory,
        isLoadingHistory,
        shouldShowDisconnectDivider,
    };
};
