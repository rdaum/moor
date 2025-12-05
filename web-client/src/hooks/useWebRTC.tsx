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
// ! WebRTC Data Channel transport - experimental alternative to WebSocket.
// ! Provides the same interface as useWebSocket but uses WebRTC data channels.

import { useCallback, useEffect, useRef, useState } from "react";
import { EventMetadata, handleClientEventFlatBuffer } from "../lib/rpc-fb";
import { InputMetadata } from "../types/input";
import { PresentationData } from "../types/presentation";
import { Player } from "./useAuth";

export interface WebRTCState {
    peerConnection: RTCPeerConnection | null;
    dataChannel: RTCDataChannel | null;
    isConnected: boolean;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
}

// Re-export with same name for compatibility
export type WebSocketState = WebRTCState;

interface RtcOfferResponse {
    sdp: string;
    client_id: string;
    client_token: string;
}

export const useWebRTC = (
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
        eventMetadata?: EventMetadata,
    ) => void,
    onPresentMessage?: (presentData: PresentationData) => void,
    onUnpresentMessage?: (id: string) => void,
    onAuthFailure?: () => void,
    onInitialAttachComplete?: () => void,
) => {
    const [rtcState, setRtcState] = useState<WebRTCState>({
        peerConnection: null,
        dataChannel: null,
        isConnected: false,
        connectionStatus: "disconnected",
    });

    const [inputMetadata, setInputMetadata] = useState<InputMetadata | null>(null);

    const peerConnectionRef = useRef<RTCPeerConnection | null>(null);
    const dataChannelRef = useRef<RTCDataChannel | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);
    const lastEventTimestampRef = useRef<bigint | null>(null);
    const processingRef = useRef<Promise<void>>(Promise.resolve());
    const isDisconnectingRef = useRef(false);
    const connectionStatusRef = useRef<WebRTCState["connectionStatus"]>("disconnected");
    const hasEverConnectedRef = useRef(false);
    const connectRef = useRef<((mode: "connect" | "create") => Promise<void>) | null>(null);

    useEffect(() => {
        connectionStatusRef.current = rtcState.connectionStatus;
    }, [rtcState.connectionStatus]);

    // Handle incoming data channel messages
    const handleMessage = useCallback(async (event: MessageEvent) => {
        processingRef.current = processingRef.current.then(async () => {
            try {
                // Data channel messages are ArrayBuffer or Blob
                if (event.data instanceof ArrayBuffer || event.data instanceof Blob) {
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
                } else if (typeof event.data === "string") {
                    // Data channels can also receive strings
                    const encoder = new TextEncoder();
                    const bytes = encoder.encode(event.data);
                    handleClientEventFlatBuffer(
                        bytes,
                        onSystemMessage,
                        onNarrativeMessage,
                        onPresentMessage,
                        onUnpresentMessage,
                        onPlayerFlagsChange,
                        lastEventTimestampRef,
                        setInputMetadata,
                    );
                } else {
                    console.error("Unexpected data channel message type:", typeof event.data);
                }
            } catch (error) {
                console.error("Failed to parse data channel message:", error);
            }
        });
    }, [onSystemMessage, onNarrativeMessage, onPresentMessage, onUnpresentMessage, onPlayerFlagsChange]);

    // Connect via WebRTC
    const connect = useCallback(async (mode: "connect" | "create") => {
        if (!player || !player.authToken) {
            console.error("[WebRTC] Cannot connect: No player or auth token");
            return;
        }

        if (isDisconnectingRef.current) {
            console.warn("[WebRTC] Cannot connect: Disconnect in progress");
            return;
        }

        if (dataChannelRef.current?.readyState === "open") {
            console.log("[WebRTC] Already connected, skipping");
            return;
        }

        console.log("[WebRTC] Starting connection for player:", player.oid);

        // Close existing connection if any
        if (peerConnectionRef.current) {
            console.warn("[WebRTC] Found existing connection, closing it first");
            peerConnectionRef.current.close();
            peerConnectionRef.current = null;
            dataChannelRef.current = null;
        }

        try {
            setRtcState(prev => ({ ...prev, connectionStatus: "connecting" }));
            onSystemMessage("Establishing WebRTC connection...", 2);

            // Create peer connection
            // STUN helps with NAT traversal; for localhost we mainly need it to trigger gathering
            const config: RTCConfiguration = {
                iceServers: [
                    { urls: "stun:stun.l.google.com:19302" },
                ],
            };
            const pc = new RTCPeerConnection(config);
            peerConnectionRef.current = pc;

            // Create data channel BEFORE creating offer
            const dc = pc.createDataChannel("moor-events", {
                ordered: true, // Reliable, ordered delivery
            });
            dataChannelRef.current = dc;

            // Set up data channel handlers
            dc.onopen = () => {
                console.log("[WebRTC] Data channel opened, sending READY signal");

                // Send READY signal to tell server we're ready to receive events
                // This prevents race condition where server sends before client is ready
                dc.send("READY");

                setRtcState(prev => ({
                    ...prev,
                    peerConnection: pc,
                    dataChannel: dc,
                    isConnected: true,
                    connectionStatus: "connected",
                }));
                onSystemMessage("Connected via WebRTC!", 2);
                localStorage.setItem("client_session_active", "true");
                hasEverConnectedRef.current = true;

                if (onPlayerConnectedChange) {
                    onPlayerConnectedChange(true);
                }

                if (player?.isInitialAttach && onInitialAttachComplete) {
                    onInitialAttachComplete();
                }

                if (reconnectTimeoutRef.current) {
                    clearTimeout(reconnectTimeoutRef.current);
                    reconnectTimeoutRef.current = null;
                }
            };

            dc.onmessage = handleMessage;

            dc.onerror = (error) => {
                console.error("[WebRTC] Data channel error:", error);
                setRtcState(prev => ({ ...prev, connectionStatus: "error" }));
                onSystemMessage("WebRTC data channel error", 5);
            };

            dc.onclose = () => {
                console.log("[WebRTC] Data channel closed");
            };

            // NOTE: onconnectionstatechange is set up AFTER SDP exchange to avoid
            // interfering with ICE gathering (Firefox fires "failed" early sometimes)

            // Log ICE state changes for debugging
            pc.onicegatheringstatechange = () => {
                console.log("[WebRTC] ICE gathering state:", pc.iceGatheringState);
            };
            pc.oniceconnectionstatechange = () => {
                console.log("[WebRTC] ICE connection state:", pc.iceConnectionState);
            };

            // Collect ICE candidates - must set handler BEFORE setLocalDescription
            // because Firefox fires events immediately
            const candidates: RTCIceCandidate[] = [];
            let gatheringResolve: (() => void) | null = null;

            const gatheringComplete = new Promise<void>((resolve) => {
                gatheringResolve = resolve;
            });

            let resolved = false;
            const done = () => {
                if (!resolved) {
                    resolved = true;
                    gatheringResolve?.();
                }
            };

            pc.onicecandidate = (event) => {
                if (event.candidate) {
                    console.log("[WebRTC] ICE candidate:", event.candidate.candidate.substring(0, 50) + "...");
                    candidates.push(event.candidate);
                } else {
                    // null candidate means gathering is complete
                    console.log("[WebRTC] ICE gathering complete, collected", candidates.length, "candidates");
                    done();
                }
            };

            // Create offer and set local description - this triggers ICE gathering
            const offer = await pc.createOffer();
            await pc.setLocalDescription(offer);

            // Timeout fallback
            const timeout = setTimeout(() => {
                console.log("[WebRTC] ICE gathering timeout, collected", candidates.length, "candidates");
                done();
            }, 3000);

            await gatheringComplete;
            clearTimeout(timeout);

            // Build SDP with candidates included
            // Firefox's localDescription may not include candidates, so we add them manually
            let sdp = pc.localDescription?.sdp;
            if (!sdp) {
                throw new Error("No local description after ICE gathering");
            }

            // If no candidates in SDP, append them
            const existingCandidates = (sdp.match(/a=candidate:/g) || []).length;
            if (existingCandidates === 0 && candidates.length > 0) {
                console.log("[WebRTC] Appending", candidates.length, "candidates to SDP");
                // Find the m= line and append candidates after it
                const lines = sdp.split("\r\n");
                const newLines: string[] = [];
                for (const line of lines) {
                    newLines.push(line);
                    // After sctp-port line, add candidates
                    if (line.startsWith("a=sctp-port:")) {
                        for (const candidate of candidates) {
                            newLines.push(`a=${candidate.candidate}`);
                        }
                    }
                }
                sdp = newLines.join("\r\n");
            }

            const candidateCount = (sdp.match(/a=candidate:/g) || []).length;
            console.log("[WebRTC] Sending offer to server with", candidateCount, "ICE candidates");

            // Send offer to server
            const baseUrl = window.location.origin;
            const response = await fetch(`${baseUrl}/rtc/offer`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    "X-Moor-Auth-Token": player.authToken,
                },
                body: JSON.stringify({ sdp }),
            });

            if (!response.ok) {
                if (response.status === 401 || response.status === 403) {
                    console.log("[WebRTC] Auth failure");
                    if (onAuthFailure) {
                        onAuthFailure();
                    }
                    throw new Error("Authentication failed");
                }
                throw new Error(`Server returned ${response.status}`);
            }

            const answerData: RtcOfferResponse = await response.json();
            console.log("[WebRTC] Received answer from server, client_id:", answerData.client_id);

            // Store client credentials for reconnection
            localStorage.setItem("client_token", answerData.client_token);
            localStorage.setItem("client_id", answerData.client_id);

            // Normalize line endings and filter problematic SDP lines
            // Server might use \n, but SDP spec requires \r\n
            const normalizedSdp = answerData.sdp.replace(/\r\n/g, "\n").replace(/\n/g, "\r\n");

            let filteredAnswerSdp = normalizedSdp
                .split("\r\n")
                .filter(line => {
                    if (line.startsWith("a=end-of-candidates")) return false;
                    // Filter IPv6 candidates (contain :: in the address)
                    if (line.startsWith("a=candidate:") && line.includes("::")) return false;
                    return true;
                })
                .join("\r\n");

            // Ensure SDP ends with CRLF (required by RFC 4566)
            if (!filteredAnswerSdp.endsWith("\r\n")) {
                filteredAnswerSdp += "\r\n";
            }

            // Set remote description
            await pc.setRemoteDescription({
                type: "answer",
                sdp: filteredAnswerSdp,
            });

            console.log("[WebRTC] Remote description set, waiting for data channel to open");

            // Set up connection state handler AFTER SDP exchange to avoid interfering with ICE gathering
            pc.onconnectionstatechange = () => {
                console.log("[WebRTC] Connection state:", pc.connectionState);
                switch (pc.connectionState) {
                    case "disconnected":
                    case "failed":
                    case "closed":
                        setRtcState(prev => ({
                            ...prev,
                            peerConnection: null,
                            dataChannel: null,
                            isConnected: false,
                            connectionStatus: "disconnected",
                        }));
                        peerConnectionRef.current = null;
                        dataChannelRef.current = null;

                        if (onPlayerConnectedChange) {
                            onPlayerConnectedChange(false);
                        }

                        // Schedule reconnect if we were previously connected
                        if (hasEverConnectedRef.current && !reconnectTimeoutRef.current) {
                            onSystemMessage("Connection lost, reconnecting...", 3);
                            reconnectTimeoutRef.current = window.setTimeout(() => {
                                reconnectTimeoutRef.current = null;
                                if (connectionStatusRef.current !== "connected" && connectRef.current) {
                                    connectRef.current(mode);
                                }
                            }, 3000);
                        }
                        break;
                }
            };
        } catch (error) {
            console.error("[WebRTC] Connection failed:", error);
            setRtcState(prev => ({ ...prev, connectionStatus: "error" }));
            onSystemMessage(
                `WebRTC connection error: ${error instanceof Error ? error.message : "Unknown error"}`,
                5,
            );

            // Cleanup on failure
            if (peerConnectionRef.current) {
                peerConnectionRef.current.close();
                peerConnectionRef.current = null;
                dataChannelRef.current = null;
            }
        }
    }, [handleMessage, onPlayerConnectedChange, onSystemMessage, player, onInitialAttachComplete, onAuthFailure]);

    // Keep connectRef updated
    useEffect(() => {
        connectRef.current = connect;
    }, [connect]);

    // Disconnect
    const disconnect = useCallback((reason?: string) => {
        isDisconnectingRef.current = true;

        if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
            reconnectTimeoutRef.current = null;
        }

        if (peerConnectionRef.current) {
            peerConnectionRef.current.close();
            peerConnectionRef.current = null;
            dataChannelRef.current = null;

            setRtcState({
                peerConnection: null,
                dataChannel: null,
                isConnected: false,
                connectionStatus: "disconnected",
            });
        }

        if (reason === "LOGOUT") {
            localStorage.setItem("client_session_active", "false");
        }

        setTimeout(() => {
            isDisconnectingRef.current = false;
        }, 100);
    }, []);

    // Send message
    const sendMessage = useCallback((message: string | Uint8Array | ArrayBuffer) => {
        if (dataChannelRef.current?.readyState === "open") {
            if (typeof message === "string") {
                dataChannelRef.current.send(message);
            } else if (message instanceof ArrayBuffer) {
                dataChannelRef.current.send(message);
            } else {
                // Uint8Array - copy to a new ArrayBuffer to satisfy TypeScript
                const buffer = message.buffer.slice(
                    message.byteOffset,
                    message.byteOffset + message.byteLength,
                ) as ArrayBuffer;
                dataChannelRef.current.send(buffer);
            }
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
            if (peerConnectionRef.current) {
                peerConnectionRef.current.close();
            }
        };
    }, []);

    // Reset state when player becomes null (logout)
    useEffect(() => {
        if (!player) {
            setRtcState({
                peerConnection: null,
                dataChannel: null,
                isConnected: false,
                connectionStatus: "disconnected",
            });
            lastEventTimestampRef.current = null;
            hasEverConnectedRef.current = false;
        }
    }, [player]);

    // Return with wsState alias for compatibility
    return {
        wsState: {
            socket: null, // Compatibility - not used with WebRTC
            isConnected: rtcState.isConnected,
            connectionStatus: rtcState.connectionStatus,
        },
        connect,
        disconnect,
        sendMessage,
        inputMetadata,
        clearInputMetadata,
    };
};
