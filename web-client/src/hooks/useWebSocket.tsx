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
import { EventMetadata, handleClientEventFlatBuffer, LinkPreview } from "../lib/rpc-fb";
import { InputMetadata } from "../types/input";
import { PresentationData } from "../types/presentation";
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
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
        linkPreview?: LinkPreview,
        eventMetadata?: EventMetadata,
    ) => void,
    onPresentMessage?: (presentData: PresentationData) => void,
    onUnpresentMessage?: (id: string) => void,
    onAuthFailure?: () => void,
    onInitialAttachComplete?: () => void,
) => {
    const [wsState, setWsState] = useState<WebSocketState>({
        socket: null,
        isConnected: false,
        connectionStatus: "disconnected",
    });

    const [inputMetadata, setInputMetadata] = useState<InputMetadata | null>(null);

    const socketRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);
    const lastEventTimestampRef = useRef<bigint | null>(null);
    const processingRef = useRef<Promise<void>>(Promise.resolve());
    const isDisconnectingRef = useRef(false);
    const connectionStatusRef = useRef<WebSocketState["connectionStatus"]>("disconnected");
    const hasEverConnectedRef = useRef(false);
    // Ref to current connect function - used by reconnect timeout to avoid stale closures
    const connectRef = useRef<((mode: "connect" | "create") => Promise<void>) | null>(null);

    useEffect(() => {
        connectionStatusRef.current = wsState.connectionStatus;
    }, [wsState.connectionStatus]);

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
                        setInputMetadata,
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

            // Get client tokens from localStorage for reconnection
            const clientToken = localStorage.getItem("client_token");
            const clientId = localStorage.getItem("client_id");
            const sessionActive = localStorage.getItem("client_session_active") === "true";

            const wsUrl = `${isSecure ? "wss://" : "ws://"}${baseUrl}/ws/attach/${mode}`;
            const wsProtocols = ["moor", `paseto.${player.authToken}`];

            if (player.isInitialAttach) {
                wsProtocols.push("initial_attach.true");
                console.log("[WebSocket] Initial attach - will trigger user_connected");
            }

            if (sessionActive && clientToken && clientId) {
                wsProtocols.push(`client_id.${clientId}`);
                wsProtocols.push(`client_token.${clientToken}`);
                console.log("[WebSocket] Reconnecting with existing client_id:", clientId);
            } else {
                console.log("[WebSocket] New connection (no stored tokens)");
            }

            console.log("[WebSocket] Creating new WebSocket to:", wsUrl);
            const ws = new WebSocket(wsUrl, wsProtocols);
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
                localStorage.setItem("client_session_active", "true");
                hasEverConnectedRef.current = true;

                // Update player connection status
                if (onPlayerConnectedChange) {
                    onPlayerConnectedChange(true);
                }

                // Notify parent to update isInitialAttach based on history encryption
                if (player?.isInitialAttach && onInitialAttachComplete) {
                    onInitialAttachComplete();
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

                if (event.reason === "LOGOUT") {
                    localStorage.setItem("client_session_active", "false");
                }

                // Update player connection status
                if (onPlayerConnectedChange) {
                    onPlayerConnectedChange(false);
                }

                if (event.code !== 1000) { // 1000 is normal closure
                    // If we've never successfully connected, this is likely an auth failure
                    if (!hasEverConnectedRef.current) {
                        console.log("[WebSocket] Connection failed on initial attempt - likely auth failure");
                        onSystemMessage("Authentication failed - please log in again", 5);
                        if (onAuthFailure) {
                            onAuthFailure();
                        }
                        return;
                    }

                    onSystemMessage(
                        `Connection closed: ${event.reason || "Server disconnected"}`,
                        5,
                    );

                    // Schedule reconnect for non-normal closures (only if we've connected before)
                    // Uses connectRef to get current connect function, avoiding stale closure issues
                    const delay = 3000;
                    if (!reconnectTimeoutRef.current) {
                        reconnectTimeoutRef.current = window.setTimeout(() => {
                            reconnectTimeoutRef.current = null;
                            if (connectionStatusRef.current !== "connected" && connectRef.current) {
                                connectRef.current(mode);
                            }
                        }, delay);
                    }
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
    }, [handleMessage, onPlayerConnectedChange, onSystemMessage, player, onInitialAttachComplete]);

    // Keep connectRef updated so reconnect timeouts use current function
    useEffect(() => {
        connectRef.current = connect;
    }, [connect]);

    // Disconnect from WebSocket
    const disconnect = useCallback((reason?: string) => {
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
            oldSocket.close(1000, reason ?? "Manual disconnect");

            // Immediately clear state
            setWsState({
                socket: null,
                isConnected: false,
                connectionStatus: "disconnected",
            });
        }

        if (reason === "LOGOUT") {
            localStorage.setItem("client_session_active", "false");
        }

        // Allow reconnect after a short delay
        setTimeout(() => {
            isDisconnectingRef.current = false;
        }, 100);
    }, []);

    // Send message (text string or binary data)
    const sendMessage = useCallback((message: string | Uint8Array | ArrayBuffer) => {
        if (socketRef.current?.readyState === WebSocket.OPEN) {
            socketRef.current.send(message);
            return true;
        } else {
            onSystemMessage("Not connected to server", 3);
            return false;
        }
    }, [onSystemMessage]);

    // Clear input metadata
    const clearInputMetadata = useCallback(() => {
        setInputMetadata(null);
    }, []);

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
            hasEverConnectedRef.current = false;
        }
    }, [player]);

    return {
        wsState,
        connect,
        disconnect,
        sendMessage,
        inputMetadata,
        clearInputMetadata,
    };
};
