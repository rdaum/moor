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

export interface WebSocketMessage {
    kind?: string;
    message?: string;
    system_message?: string;
    present?: any;
    unpresent?: string;
    traceback?: any;
    server_time?: string;
}

export interface WebSocketState {
    socket: WebSocket | null;
    isConnected: boolean;
    lastMessage: WebSocketMessage | null;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
}

export const useWebSocket = (
    player: Player | null,
    onSystemMessage: (message: string, duration?: number) => void,
    onPlayerConnectedChange?: (connected: boolean) => void,
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
    ) => void,
    onPresentMessage?: (presentData: any) => void,
    onUnpresentMessage?: (id: string) => void,
    onMessage?: (message: WebSocketMessage) => void,
) => {
    const [wsState, setWsState] = useState<WebSocketState>({
        socket: null,
        isConnected: false,
        lastMessage: null,
        connectionStatus: "disconnected",
    });

    const socketRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);

    // Handle incoming WebSocket messages
    const handleMessage = useCallback((event: MessageEvent) => {
        try {
            const data: WebSocketMessage = JSON.parse(event.data);

            // Update state with last message
            setWsState(prev => ({ ...prev, lastMessage: data }));

            // Handle different message types
            if (typeof data !== "object" || data === null) {
                // Skip non-object messages like numbers
                return;
            } else if (data.system_message) {
                onSystemMessage(data.system_message, 5);
            } else if ("message" in data && data.message !== undefined) {
                // Narrative message - send to narrative display
                console.log("DEBUG: Raw WebSocket message data:", JSON.stringify(data, null, 2));

                let content: string | string[];
                let contentType: string | undefined;

                if (typeof data.message === "string") {
                    content = data.message;
                } else if (typeof data.message === "object" && data.message !== null) {
                    // For arrays or objects, use the message directly as content
                    content = data.message;
                } else {
                    content = JSON.stringify(data.message);
                }

                // Always check for content_type at the top level of the message
                contentType = (data as any).content_type;

                console.log("DEBUG: Extracted content:", content);
                console.log("DEBUG: Extracted contentType:", contentType);

                if (onNarrativeMessage) {
                    onNarrativeMessage(content, data.server_time, contentType, false); // WebSocket messages are always live (not historical)
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
                // Traceback message - for now just log it
            }

            // Call optional message handler
            if (onMessage) {
                onMessage(data);
            }
        } catch (error) {
            console.error("Failed to parse WebSocket message:", error, event.data);
        }
    }, [onSystemMessage, onNarrativeMessage, onPresentMessage, onUnpresentMessage, onMessage]);

    // Connect to WebSocket
    const connect = useCallback(async (mode: "connect" | "create") => {
        if (!player || !player.authToken) {
            console.error("Cannot connect: No player or auth token");
            return;
        }

        if (socketRef.current?.readyState === WebSocket.OPEN) {
            return;
        }

        try {
            setWsState(prev => ({ ...prev, connectionStatus: "connecting" }));
            onSystemMessage("Establishing connection...", 2);

            // Build WebSocket URL
            const baseUrl = window.location.host;
            const isSecure = window.location.protocol === "https:";
            const wsUrl = `${isSecure ? "wss://" : "ws://"}${baseUrl}/ws/attach/${mode}/${player.authToken}`;

            const ws = new WebSocket(wsUrl);
            socketRef.current = ws;

            // Set up event handlers
            ws.onopen = () => {
                setWsState(prev => ({
                    ...prev,
                    socket: ws,
                    isConnected: true,
                    connectionStatus: "connected",
                }));
                onSystemMessage("Connected!", 2);

                // Update player connection status
                if (onPlayerConnectedChange) {
                    onPlayerConnectedChange(true);
                }

                // Clear any reconnection timeout
                if (reconnectTimeoutRef.current) {
                    clearTimeout(reconnectTimeoutRef.current);
                    reconnectTimeoutRef.current = null;
                }
            };

            ws.onmessage = handleMessage;

            ws.onerror = (_error) => {
                setWsState(prev => ({ ...prev, connectionStatus: "error" }));
                onSystemMessage("Connection error", 5);
            };

            ws.onclose = (event) => {
                setWsState(prev => ({
                    ...prev,
                    socket: null,
                    isConnected: false,
                    connectionStatus: "disconnected",
                }));
                socketRef.current = null;

                // Update player connection status
                if (onPlayerConnectedChange) {
                    onPlayerConnectedChange(false);
                }

                if (event.code !== 1000) { // 1000 is normal closure
                    onSystemMessage(
                        `Connection closed: ${event.reason || "Server disconnected"}`,
                        5,
                    );

                    // Schedule reconnect for non-normal closures
                    scheduleReconnect(mode);
                }
            };
        } catch (error) {
            console.error("Failed to create WebSocket connection:", error);
            setWsState(prev => ({ ...prev, connectionStatus: "error" }));
            onSystemMessage(
                `Connection error: ${error instanceof Error ? error.message : "Unknown error"}`,
                5,
            );
        }
    }, [player, onSystemMessage, handleMessage]);

    // Schedule reconnection
    const scheduleReconnect = useCallback((mode: "connect" | "create") => {
        if (reconnectTimeoutRef.current) {
            return; // Already scheduled
        }

        const delay = 3000; // 3 seconds

        reconnectTimeoutRef.current = window.setTimeout(() => {
            reconnectTimeoutRef.current = null;
            if (wsState.connectionStatus !== "connected") {
                connect(mode);
            }
        }, delay);
    }, [connect, wsState.connectionStatus]);

    // Disconnect from WebSocket
    const disconnect = useCallback(() => {
        if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
            reconnectTimeoutRef.current = null;
        }

        if (socketRef.current) {
            socketRef.current.close(1000, "Manual disconnect");
        }
    }, []);

    // Send message
    const sendMessage = useCallback((message: string) => {
        if (socketRef.current?.readyState === WebSocket.OPEN) {
            socketRef.current.send(message);
            return true;
        } else {
            onSystemMessage("Not connected to server", 3);
            return false;
        }
    }, [onSystemMessage]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (reconnectTimeoutRef.current) {
                clearTimeout(reconnectTimeoutRef.current);
            }
            if (socketRef.current) {
                socketRef.current.close(1000, "Component unmounting");
            }
        };
    }, []);

    return {
        wsState,
        connect,
        disconnect,
        sendMessage,
    };
};
