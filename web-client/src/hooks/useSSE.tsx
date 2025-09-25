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

import { useCallback, useEffect, useRef, useState } from "react";
import { Player } from "./useAuth";

export interface SSEMessage {
    kind?: string;
    message?: string;
    system_message?: string;
    present?: any;
    unpresent?: string;
    traceback?: any;
    server_time?: string;
    no_newline?: boolean;
    event_id?: string;
    is_historical?: boolean;
}

export interface SSEState {
    eventSource: EventSource | null;
    isConnected: boolean;
    lastMessage: SSEMessage | null;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
    lastEventId: string | null;
}

export const useSSE = (
    player: Player | null,
    onSystemMessage: (message: string, duration?: number) => void,
    onPlayerConnectedChange?: (connected: boolean) => void,
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
    ) => void,
    onPresentMessage?: (presentData: any) => void,
    onUnpresentMessage?: (id: string) => void,
    onMessage?: (message: SSEMessage) => void,
) => {
    const [sseState, setSseState] = useState<SSEState>({
        eventSource: null,
        isConnected: false,
        lastMessage: null,
        connectionStatus: "disconnected",
        lastEventId: null,
    });

    const eventSourceRef = useRef<EventSource | null>(null);

    // Handle incoming SSE messages
    const handleMessage = useCallback((event: MessageEvent) => {
        try {
            const data: SSEMessage = JSON.parse(event.data);

            // Store the event ID for reconnection
            if (event.lastEventId) {
                setSseState(prev => ({ ...prev, lastEventId: event.lastEventId }));
            }

            // Update state with last message
            setSseState(prev => ({ ...prev, lastMessage: data }));

            // Handle different message types - same logic as WebSocket
            if (typeof data !== "object" || data === null) {
                // Skip non-object messages like numbers
                return;
            } else if (data.system_message) {
                onSystemMessage(data.system_message, 5);
            } else if ("message" in data && data.message !== undefined) {
                // Narrative message - send to narrative display

                let content: string | string[];

                if (typeof data.message === "string") {
                    content = data.message;
                } else if (typeof data.message === "object" && data.message !== null) {
                    // For arrays or objects, use the message directly as content
                    content = data.message;
                } else {
                    content = JSON.stringify(data.message);
                }

                // Always check for content_type at the top level of the message
                const contentType = (data as any).content_type;

                if (onNarrativeMessage) {
                    onNarrativeMessage(content, data.server_time, contentType, data.is_historical, data.no_newline);
                }
            } else if (data.present) {
                // Presentation message - handle present
                if (onPresentMessage) {
                    onPresentMessage(data.present);
                }
            } else if (data.unpresent) {
                // Unpresent message - handle unpresent
                if (onUnpresentMessage) {
                    onUnpresentMessage(data.unpresent);
                }
            } else if (data.traceback) {
                // Traceback message - log to console and show as narrative
                console.error("MOO Traceback:", data.traceback);
                if (onNarrativeMessage) {
                    const tracebackText = `${data.traceback.error}\n${data.traceback.traceback.join("\n")}`;
                    onNarrativeMessage(tracebackText, data.server_time, "text/traceback", false, false);
                }
            }

            // Call optional message handler
            if (onMessage) {
                onMessage(data);
            }
        } catch (error) {
            console.error("Failed to parse SSE message:", error, event.data);
        }
    }, [onSystemMessage, onNarrativeMessage, onPresentMessage, onUnpresentMessage, onMessage]);

    // Handle connection events
    const handleOpen = useCallback(() => {
        setSseState(prev => ({
            ...prev,
            isConnected: true,
            connectionStatus: "connected",
        }));
        onSystemMessage("Connected!", 2);

        // Update player connection status
        if (onPlayerConnectedChange) {
            onPlayerConnectedChange(true);
        }
    }, [onSystemMessage, onPlayerConnectedChange]);

    const handleError = useCallback((_error: Event) => {
        setSseState(prev => ({ ...prev, connectionStatus: "error" }));
        onSystemMessage("Connection lost, reconnecting...", 3);

        // Update player connection status
        if (onPlayerConnectedChange) {
            onPlayerConnectedChange(false);
        }
    }, [onSystemMessage, onPlayerConnectedChange]);

    // Connect to SSE
    const connect = useCallback(async (_mode: "connect" | "create") => {
        if (!player || !player.authToken) {
            return;
        }

        if (eventSourceRef.current?.readyState === EventSource.OPEN) {
            return;
        }

        try {
            setSseState(prev => ({ ...prev, connectionStatus: "connecting" }));
            onSystemMessage("Establishing connection...", 2);

            const baseUrl = window.location.host;
            const sseUrl = `${window.location.protocol}//${baseUrl}/sse/events?token=${
                encodeURIComponent(player.authToken)
            }`;

            const eventSource = new EventSource(sseUrl);
            eventSourceRef.current = eventSource;

            eventSource.onopen = handleOpen;
            eventSource.onmessage = handleMessage;
            eventSource.onerror = handleError;

            // Handle specific event types
            eventSource.addEventListener("narrative", handleMessage);
            eventSource.addEventListener("system_message", handleMessage);
            eventSource.addEventListener("error", handleMessage);
            eventSource.addEventListener("disconnect", () => disconnect());

            setSseState(prev => ({ ...prev, eventSource }));
        } catch (error) {
            setSseState(prev => ({ ...prev, connectionStatus: "error" }));
            onSystemMessage(
                `Connection error: ${error instanceof Error ? error.message : "Unknown error"}`,
                5,
            );
        }
    }, [player, onSystemMessage, handleMessage, handleOpen, handleError]);

    // Disconnect from SSE
    const disconnect = useCallback(() => {
        if (eventSourceRef.current) {
            eventSourceRef.current.close();
            eventSourceRef.current = null;
        }

        setSseState(prev => ({
            ...prev,
            eventSource: null,
            isConnected: false,
            connectionStatus: "disconnected",
        }));

        // Update player connection status
        if (onPlayerConnectedChange) {
            onPlayerConnectedChange(false);
        }
    }, [onPlayerConnectedChange]);

    // Send message via HTTP POST to command endpoint (since SSE is receive-only)
    const sendMessage = useCallback(async (message: string) => {
        if (!player?.authToken) {
            onSystemMessage("Not authenticated", 3);
            return false;
        }

        try {
            const response = await fetch("/command", {
                method: "POST",
                headers: {
                    "X-Moor-Auth-Token": player.authToken,
                    "Content-Type": "text/plain",
                },
                body: message,
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            return true;
        } catch (error) {
            onSystemMessage(
                `Failed to send message: ${error instanceof Error ? error.message : "Unknown error"}`,
                5,
            );
            return false;
        }
    }, [player?.authToken, onSystemMessage]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (eventSourceRef.current) {
                eventSourceRef.current.close();
            }
        };
    }, []);

    return {
        sseState,
        connect,
        disconnect,
        sendMessage,
    };
};
