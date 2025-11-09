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
import { handleClientEventFlatBuffer } from "../lib/rpc-fb";
import { Player } from "./useAuth";

export interface WebSocketState {
    socket: WebSocket | null;
    isConnected: boolean;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
}

export const useWebSocket = (
    player: Player | null,
    onSystemMessage: (message: string, duration?: number) => void,
    onPlayerConnectedChange?: (connected: boolean) => void,
    onPlayerFlagsChange?: (flags: number) => void,
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
        presentationHint?: string,
        thumbnail?: { contentType: string; data: string },
    ) => void,
    onPresentMessage?: (presentData: any) => void,
    onUnpresentMessage?: (id: string) => void,
) => {
    const [wsState, setWsState] = useState<WebSocketState>({
        socket: null,
        isConnected: false,
        connectionStatus: "disconnected",
    });

    const socketRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);
    const lastEventTimestampRef = useRef<bigint | null>(null);
    const processingRef = useRef<Promise<void>>(Promise.resolve());
    const isDisconnectingRef = useRef(false);

    // Handle incoming WebSocket messages
    const handleMessage = useCallback(async (event: MessageEvent) => {
        // Queue message processing to ensure sequential handling
        // This prevents race conditions when async processing causes reordering
        processingRef.current = processingRef.current.then(async () => {
            try {
                // All messages are now binary FlatBuffer format
                if (event.data instanceof ArrayBuffer || event.data instanceof Blob) {
                    // Convert Blob to ArrayBuffer if needed
                    const arrayBuffer = event.data instanceof Blob
                        ? await event.data.arrayBuffer()
                        : event.data;

                    handleClientEventFlatBuffer(
                        new Uint8Array(arrayBuffer),
                        onSystemMessage,
                        onNarrativeMessage,
                        onPresentMessage,
                        onUnpresentMessage,
                        onPlayerFlagsChange,
                        lastEventTimestampRef,
                    );
                } else {
                    console.error("Unexpected non-binary WebSocket message:", event.data);
                }
            } catch (error) {
                console.error("Failed to parse WebSocket message:", error);
            }
        });
    }, [onSystemMessage, onNarrativeMessage, onPresentMessage, onUnpresentMessage, onPlayerFlagsChange]);

    // Connect to WebSocket
    const connect = useCallback(async (mode: "connect" | "create") => {
        if (!player || !player.authToken) {
            console.error("[WebSocket] Cannot connect: No player or auth token");
            return;
        }

        if (isDisconnectingRef.current) {
            console.warn("[WebSocket] Cannot connect: Disconnect in progress");
            return;
        }

        if (socketRef.current?.readyState === WebSocket.OPEN) {
            console.log("[WebSocket] Already connected, skipping");
            return;
        }

        console.log("[WebSocket] Starting connection for player:", player.oid);

        // If there's an existing socket that's not closed, close it first
        if (socketRef.current && socketRef.current.readyState !== WebSocket.CLOSED) {
            console.warn("[WebSocket] Found existing socket, closing it first. State:", socketRef.current.readyState);
            const oldSocket = socketRef.current;
            socketRef.current = null;
            oldSocket.onopen = null;
            oldSocket.onmessage = null;
            oldSocket.onerror = null;
            oldSocket.onclose = null;
            oldSocket.close(1000, "Replacing with new connection");
        }

        try {
            setWsState(prev => ({ ...prev, connectionStatus: "connecting" }));
            onSystemMessage("Establishing connection...", 2);

            // Build WebSocket URL
            const baseUrl = window.location.host;
            const isSecure = window.location.protocol === "https:";
            const wsUrl = `${isSecure ? "wss://" : "ws://"}${baseUrl}/ws/attach/${mode}/${player.authToken}`;

            console.log("[WebSocket] Creating new WebSocket to:", wsUrl);
            const ws = new WebSocket(wsUrl);
            socketRef.current = ws;
            console.log("[WebSocket] Socket created, readyState:", ws.readyState);

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
        isDisconnectingRef.current = true;

        if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
            reconnectTimeoutRef.current = null;
        }

        if (socketRef.current) {
            const oldSocket = socketRef.current;
            socketRef.current = null;

            // Remove event handlers to prevent them from firing
            oldSocket.onopen = null;
            oldSocket.onmessage = null;
            oldSocket.onerror = null;
            oldSocket.onclose = null;

            // Close the socket
            oldSocket.close(1000, "Manual disconnect");

            // Immediately clear state
            setWsState({
                socket: null,
                isConnected: false,
                connectionStatus: "disconnected",
            });
        }

        // Allow reconnect after a short delay
        setTimeout(() => {
            isDisconnectingRef.current = false;
        }, 100);
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

    // Reset state when player becomes null (logout)
    useEffect(() => {
        if (!player) {
            // Clear WebSocket state for new login
            setWsState({
                socket: null,
                isConnected: false,
                connectionStatus: "disconnected",
            });
            lastEventTimestampRef.current = null;
        }
    }, [player]);

    return {
        wsState,
        connect,
        disconnect,
        sendMessage,
    };
};
