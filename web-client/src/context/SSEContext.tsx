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
import { SSEState, useSSE } from "../hooks/useSSE";
import { PresentationData } from "../types/presentation";

interface SSEContextType {
    sseState: SSEState;
    connect: (mode: "connect" | "create") => Promise<void>;
    disconnect: () => void;
    sendMessage: (message: string) => boolean;
    sendMessageAsync: (message: string) => Promise<boolean>;
}

const SSEContext = createContext<SSEContextType | undefined>(undefined);

interface SSEProviderProps {
    children: React.ReactNode;
    player: Player | null;
    showMessage: (message: string, duration?: number) => void;
    setPlayerConnected: (connected: boolean) => void;
    handleNarrativeMessage: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
    ) => void;
    handlePresentMessage: (presentData: PresentationData) => void;
    handleUnpresentMessage: (id: string) => void;
}

export const SSEProvider: React.FC<SSEProviderProps> = ({
    children,
    player,
    showMessage,
    setPlayerConnected,
    handleNarrativeMessage,
    handlePresentMessage,
    handleUnpresentMessage,
}) => {
    const sseHook = useSSE(
        player,
        showMessage,
        setPlayerConnected,
        handleNarrativeMessage,
        handlePresentMessage,
        handleUnpresentMessage,
    );

    // Create a sync wrapper for sendMessage that fires and forgets
    const sendMessage = (message: string): boolean => {
        sseHook.sendMessage(message).catch(error => {
            console.error("Failed to send message:", error);
        });
        return true; // Always return true for compatibility
    };

    const contextValue = {
        ...sseHook,
        sendMessage,
        sendMessageAsync: sseHook.sendMessage,
    };

    return (
        <SSEContext.Provider value={contextValue}>
            {children}
        </SSEContext.Provider>
    );
};

export const useSSEContext = (): SSEContextType => {
    const context = useContext(SSEContext);
    if (context === undefined) {
        throw new Error("useSSEContext must be used within an SSEProvider");
    }
    return context;
};
