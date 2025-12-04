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

import React, { createContext, useContext } from "react";
import { Player } from "../hooks/useAuth";
import { useWebSocket, WebSocketState } from "../hooks/useWebSocket";
import { InputMetadata } from "../types/input";
import { PresentationData } from "../types/presentation";

interface WebSocketContextType {
    wsState: WebSocketState;
    connect: (mode: "connect" | "create") => Promise<void>;
    disconnect: (reason?: string) => void;
    sendMessage: (message: string | Uint8Array | ArrayBuffer) => boolean;
    inputMetadata: InputMetadata | null;
    clearInputMetadata: () => void;
}

const WebSocketContext = createContext<WebSocketContextType | undefined>(undefined);

interface WebSocketProviderProps {
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
    ) => void;
    handlePresentMessage: (presentData: PresentationData) => void;
    handleUnpresentMessage: (id: string) => void;
}

export const WebSocketProvider: React.FC<WebSocketProviderProps> = ({
    children,
    player,
    showMessage,
    setPlayerConnected,
    setPlayerFlags,
    handleNarrativeMessage,
    handlePresentMessage,
    handleUnpresentMessage,
}) => {
    const webSocketHook = useWebSocket(
        player,
        showMessage,
        setPlayerConnected,
        setPlayerFlags,
        handleNarrativeMessage,
        handlePresentMessage,
        handleUnpresentMessage,
    );

    return (
        <WebSocketContext.Provider value={webSocketHook}>
            {children}
        </WebSocketContext.Provider>
    );
};

export const useWebSocketContext = (): WebSocketContextType => {
    const context = useContext(WebSocketContext);
    if (context === undefined) {
        throw new Error("useWebSocketContext must be used within a WebSocketProvider");
    }
    return context;
};
