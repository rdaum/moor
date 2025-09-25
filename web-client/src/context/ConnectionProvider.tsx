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

import { createContext, ReactNode, useContext, useEffect, useState } from "react";
import { Player } from "../hooks/useAuth";
import { SSEProvider, useSSEContext } from "./SSEContext";
import { useWebSocketContext, WebSocketProvider } from "./WebSocketContext";

type ConnectionMode = "sse" | "websocket" | "testing";

interface ConnectionContextType {
    sendMessage: (message: string) => Promise<boolean>;
    connect: (mode: "connect" | "create") => Promise<void>;
    disconnect: () => void;
    connectionMode: ConnectionMode;
    isConnected: boolean;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
}

const ConnectionContext = createContext<ConnectionContextType | undefined>(undefined);

interface ConnectionProviderProps {
    children: ReactNode;
    player: Player | null;
    onSystemMessage: (message: string, duration?: number) => void;
    onPlayerConnectedChange?: (connected: boolean) => void;
    onNarrativeMessage?: (
        content: string | string[],
        timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
    ) => void;
    onPresentMessage?: (presentData: any) => void;
    onUnpresentMessage?: (id: string) => void;
    onMessage?: (message: any) => void;
    forcedConnectionMode?: "sse" | "websocket";
}

function ConnectionModeDetector({ children, ...props }: ConnectionProviderProps) {
    const [connectionMode, setConnectionMode] = useState<ConnectionMode>("testing");

    useEffect(() => {
        // If forced mode is specified, use it immediately
        if (props.forcedConnectionMode) {
            props.onSystemMessage(`Using forced ${props.forcedConnectionMode.toUpperCase()} connection`, 2);
            setConnectionMode(props.forcedConnectionMode);
            return;
        }

        // Auto-detect: Test SSE availability by checking if endpoint exists
        const testSSE = async () => {
            try {
                const baseUrl = window.location.host;
                const sseUrl = `${window.location.protocol}//${baseUrl}/sse/events`;

                // Just test if the endpoint responds (it should give 401/403 without auth, not 404)
                const response = await fetch(sseUrl, { method: "HEAD" });

                // If we get any response (even 401), SSE endpoint exists
                if (response.status === 404 || response.status === 500) {
                    props.onSystemMessage("SSE not available, using WebSocket", 3);
                    setConnectionMode("websocket");
                } else {
                    props.onSystemMessage("Auto-detected SSE connection", 2);
                    setConnectionMode("sse");
                }
            } catch (error) {
                // Network error or endpoint doesn't exist
                props.onSystemMessage("SSE not available, using WebSocket", 3);
                setConnectionMode("websocket");
            }
        };

        testSSE();
    }, [props.onSystemMessage, props.forcedConnectionMode]);

    if (connectionMode === "testing") {
        return <div>Detecting connection method...</div>;
    }

    if (connectionMode === "sse") {
        return (
            <SSEProvider
                player={props.player}
                showMessage={props.onSystemMessage}
                setPlayerConnected={props.onPlayerConnectedChange || (() => {})}
                handleNarrativeMessage={props.onNarrativeMessage || (() => {})}
                handlePresentMessage={props.onPresentMessage || (() => {})}
                handleUnpresentMessage={props.onUnpresentMessage || (() => {})}
            >
                <SSEConnectionWrapper connectionMode={connectionMode}>
                    {children}
                </SSEConnectionWrapper>
            </SSEProvider>
        );
    }

    return (
        <WebSocketProvider
            player={props.player}
            showMessage={props.onSystemMessage}
            setPlayerConnected={props.onPlayerConnectedChange || (() => {})}
            handleNarrativeMessage={props.onNarrativeMessage || (() => {})}
            handlePresentMessage={props.onPresentMessage || (() => {})}
            handleUnpresentMessage={props.onUnpresentMessage || (() => {})}
        >
            <WebSocketConnectionWrapper connectionMode={connectionMode}>
                {children}
            </WebSocketConnectionWrapper>
        </WebSocketProvider>
    );
}

function SSEConnectionWrapper({ children, connectionMode }: { children: ReactNode; connectionMode: ConnectionMode }) {
    const sseContext = useSSEContext();

    const contextValue: ConnectionContextType = {
        sendMessage: sseContext.sendMessageAsync,
        connect: sseContext.connect,
        disconnect: sseContext.disconnect,
        connectionMode,
        isConnected: sseContext.sseState.isConnected,
        connectionStatus: sseContext.sseState.connectionStatus,
    };

    return (
        <ConnectionContext.Provider value={contextValue}>
            {children}
        </ConnectionContext.Provider>
    );
}

function WebSocketConnectionWrapper(
    { children, connectionMode }: { children: ReactNode; connectionMode: ConnectionMode },
) {
    const webSocketContext = useWebSocketContext();

    const contextValue: ConnectionContextType = {
        sendMessage: async (message: string) => {
            return webSocketContext.sendMessage(message);
        },
        connect: webSocketContext.connect,
        disconnect: webSocketContext.disconnect,
        connectionMode,
        isConnected: webSocketContext.wsState.isConnected,
        connectionStatus: webSocketContext.wsState.connectionStatus,
    };

    return (
        <ConnectionContext.Provider value={contextValue}>
            {children}
        </ConnectionContext.Provider>
    );
}

export function ConnectionProvider(props: ConnectionProviderProps) {
    return <ConnectionModeDetector {...props} />;
}

export function useConnectionContext(): ConnectionContextType {
    const context = useContext(ConnectionContext);
    if (!context) {
        throw new Error("useConnectionContext must be used within a ConnectionProvider");
    }
    return context;
}
