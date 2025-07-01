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

// Filter out MCP sequences from historical messages
const filterMCPSequences = (messages: NarrativeMessage[]): NarrativeMessage[] => {
    const filtered: NarrativeMessage[] = [];
    let inMCPSpool = false;

    for (const message of messages) {
        const content = message.content.trim();

        // Check if this starts an MCP edit sequence
        if (content.startsWith("#$# edit")) {
            inMCPSpool = true;
            continue; // Skip the MCP command line
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

interface HistoricalEvent {
    event_id: string;
    timestamp: any; // Can be string, number, or object with secs_since_epoch/nanos_since_epoch
    event_type?: string;
    message: any; // Object with type, content, etc.
    author?: any;
    player?: any;
    is_historical?: boolean;
    data?: any; // Fallback field
}

interface HistoryResponse {
    events: ReadonlyArray<HistoricalEvent>;
    meta: {
        total_events: number;
        time_range: readonly [string, string];
        has_more_before: boolean;
        earliest_event_id: string | null;
        latest_event_id: string | null;
    };
}

export const useHistory = (authToken: string | null) => {
    const [historyBoundary, setHistoryBoundary] = useState<number | null>(null);
    const [earliestHistoryEventId, setEarliestHistoryEventId] = useState<string | null>(null);
    const [isLoadingHistory, setIsLoadingHistory] = useState(false);

    // Set history boundary timestamp to prevent duplicates with WebSocket events
    const setHistoryBoundaryNow = useCallback(() => {
        const boundary = Date.now();
        setHistoryBoundary(boundary);
    }, []);

    // Check if a WebSocket event timestamp is before history boundary (duplicate)
    const isHistoricalDuplicate = useCallback((eventTimestamp: number): boolean => {
        return historyBoundary !== null && eventTimestamp < historyBoundary;
    }, [historyBoundary]);

    // Convert historical event to narrative message format (all events become narrative)
    const convertHistoricalEvent = useCallback((event: HistoricalEvent): NarrativeMessage | null => {
        try {
            // Extract message content from the actual event structure
            let messageContent = "";
            let contentType: "text/plain" | "text/djot" | "text/html" = "text/plain";

            if (event.message && typeof event.message === "object") {
                const msg = event.message as any;
                if (msg.type === "notify") {
                    messageContent = msg.content || "";
                    // Extract content type from the message object
                    if (msg.content_type) {
                        contentType = msg.content_type;
                    }
                } else if (msg.type === "traceback") {
                    messageContent = `ERROR: ${msg.error || ""}`;
                } else if (msg.type === "present") {
                    messageContent = `[Presentation: ${msg.presentation || ""}]`;
                } else if (msg.type === "unpresent") {
                    messageContent = `[Closed: ${msg.id || ""}]`;
                } else {
                    messageContent = msg.content || JSON.stringify(msg);
                    if (msg.content_type) {
                        contentType = msg.content_type;
                    }
                }
            } else if (typeof event.message === "string") {
                messageContent = event.message;
            } else if (event.data) {
                // Fallback to data field if it exists
                messageContent = typeof event.data === "string" ? event.data : JSON.stringify(event.data);
            }

            // Extract timestamp from the actual event structure
            let timestamp: number = Date.now(); // Default fallback

            if (event.timestamp) {
                if (typeof event.timestamp === "object" && event.timestamp !== null) {
                    const ts = event.timestamp as any;
                    if (ts.secs_since_epoch && typeof ts.secs_since_epoch === "number") {
                        timestamp = (ts.secs_since_epoch * 1000) + Math.floor((ts.nanos_since_epoch || 0) / 1000000);
                    }
                } else if (typeof event.timestamp === "string" || typeof event.timestamp === "number") {
                    const parsed = new Date(event.timestamp).getTime();
                    if (!isNaN(parsed)) {
                        timestamp = parsed;
                    }
                }
            }

            return {
                id: `history_${event.event_id}_${timestamp}`, // Add timestamp to ensure uniqueness
                content: messageContent,
                type: "narrative",
                timestamp,
                isHistorical: true,
                contentType,
            };
        } catch (error) {
            return null;
        }
    }, []);

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
            const params = new URLSearchParams();
            params.set("limit", limit.toString());

            if (sinceSeconds) {
                params.set("since_seconds", sinceSeconds.toString());
            }

            if (untilEvent) {
                params.set("until_event", untilEvent);
            }

            const url = `/api/history?${params}`;

            const response = await fetch(url, {
                method: "GET",
                headers: {
                    "X-Moor-Auth-Token": authToken,
                    "Content-Type": "application/json",
                },
            });

            if (!response.ok) {
                throw new Error(`History fetch failed: ${response.status} ${response.statusText}`);
            }

            const historyData: HistoryResponse = await response.json();

            // Convert events to narrative messages
            const narrativeMessages: NarrativeMessage[] = [];
            for (const event of historyData.events) {
                const message = convertHistoricalEvent(event);
                if (message) {
                    narrativeMessages.push(message);
                }
            }

            // Filter out MCP sequences before returning
            const filteredMessages = filterMCPSequences(narrativeMessages);

            // Update earliest event ID for pagination
            if (historyData.meta.earliest_event_id) {
                setEarliestHistoryEventId(historyData.meta.earliest_event_id);
            }

            return filteredMessages;
        } catch (error) {
            console.error("Failed to fetch more history:", error);
            throw error;
        } finally {
            setIsLoadingHistory(false);
        }
    }, [authToken, convertHistoricalEvent]);

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
    };
};
