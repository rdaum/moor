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
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
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
                        lastEventTimestampRef,
                    );
                } else {
                    console.error("Unexpected non-binary WebSocket message:", event.data);
                }
            } catch (error) {
                console.error("Failed to parse WebSocket message:", error);
            }
        });
    }, [onSystemMessage, onNarrativeMessage, onPresentMessage, onUnpresentMessage]);

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
