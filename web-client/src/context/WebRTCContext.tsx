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
// ! WebRTC context provider - experimental alternative to WebSocketContext.
// ! Drop-in replacement that uses WebRTC data channels instead of WebSocket.

import React, { createContext, useContext } from "react";
import { Player } from "../hooks/useAuth";
import { useWebRTC } from "../hooks/useWebRTC";
import { EventMetadata } from "../lib/rpc-fb";
import { InputMetadata } from "../types/input";
import { PresentationData } from "../types/presentation";

// Compatible state interface matching WebSocketContext
interface ConnectionState {
    socket: WebSocket | null;
    isConnected: boolean;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
}

interface WebRTCContextType {
    wsState: ConnectionState;
    connect: (mode: "connect" | "create") => Promise<void>;
    disconnect: (reason?: string) => void;
    sendMessage: (message: string | Uint8Array | ArrayBuffer) => boolean;
    inputMetadata: InputMetadata | null;
    clearInputMetadata: () => void;
}

const WebRTCContext = createContext<WebRTCContextType | undefined>(undefined);

interface WebRTCProviderProps {
    children: React.ReactNode;
    player: Player | null;
    showMessage: (message: string, duration?: number) => void;
    setPlayerConnected: (connected: boolean) => void;
    setPlayerFlags: (flags: number) => void;
    handleNarrativeMessage: (
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
    ) => void;
    handlePresentMessage: (presentData: PresentationData) => void;
    handleUnpresentMessage: (id: string) => void;
}

export const WebRTCProvider: React.FC<WebRTCProviderProps> = ({
    children,
    player,
    showMessage,
    setPlayerConnected,
    setPlayerFlags,
    handleNarrativeMessage,
    handlePresentMessage,
    handleUnpresentMessage,
}) => {
    const webRTCHook = useWebRTC(
        player,
        showMessage,
        setPlayerConnected,
        setPlayerFlags,
        handleNarrativeMessage,
        handlePresentMessage,
        handleUnpresentMessage,
    );

    return (
        <WebRTCContext.Provider value={webRTCHook}>
            {children}
        </WebRTCContext.Provider>
    );
};

export const useWebRTCContext = (): WebRTCContextType => {
    const context = useContext(WebRTCContext);
    if (context === undefined) {
        throw new Error("useWebRTCContext must be used within a WebRTCProvider");
    }
    return context;
};

// Re-export as WebSocketContext alias for easy swapping
export { WebRTCContext as WebSocketContext };
export { WebRTCProvider as WebSocketProvider };
export { useWebRTCContext as useWebSocketContext };
