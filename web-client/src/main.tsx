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
import ReactDOM from "react-dom/client";
import { BottomDock } from "./components/docks/BottomDock";
import { LeftDock } from "./components/docks/LeftDock";
import { RightDock } from "./components/docks/RightDock";
import { TopDock } from "./components/docks/TopDock";
import { Login, useWelcomeMessage } from "./components/Login";
import { MessageBoard, useSystemMessage } from "./components/MessageBoard";
import { Narrative, NarrativeRef } from "./components/Narrative";
import { ThemeToggle } from "./components/ThemeToggle";
import { VerbEditor } from "./components/VerbEditor";
import { AuthProvider, useAuthContext } from "./context/AuthContext";
import { PresentationProvider, usePresentationContext } from "./context/PresentationContext";
import { useWebSocketContext, WebSocketProvider } from "./context/WebSocketContext";
import { useHistory } from "./hooks/useHistory";
import { useMCPHandler } from "./hooks/useMCPHandler";
import { useVerbEditor } from "./hooks/useVerbEditor";
import { PresentationData } from "./types/presentation";
import "./styles/main.css";

// Simple React App component to test the setup
function AppContent({
    narrativeRef,
    narrativeCallbackRef,
}: {
    narrativeRef: React.RefObject<NarrativeRef>;
    narrativeCallbackRef: (node: NarrativeRef | null) => void;
}) {
    const { systemMessage, showMessage } = useSystemMessage();
    const welcomeMessage = useWelcomeMessage();
    const { authState, connect } = useAuthContext();
    const [loginMode, setLoginMode] = useState<"connect" | "create">("connect");
    const [historyLoaded, setHistoryLoaded] = useState(false);
    const [pendingHistoricalMessages, setPendingHistoricalMessages] = useState<any[]>([]);

    // History management
    const {
        setHistoryBoundaryNow,
        fetchInitialHistory,
        fetchMoreHistory,
        isLoadingHistory,
    } = useHistory(authState.player?.authToken || null);

    // Presentation management
    const {
        getLeftDockPresentations,
        getRightDockPresentations,
        getTopDockPresentations,
        getBottomDockPresentations,
        dismissPresentation,
        fetchCurrentPresentations,
    } = usePresentationContext();

    // Verb editor management
    const {
        editorSession,
        showVerbEditor,
        closeEditor,
    } = useVerbEditor();

    // MCP handler for parsing edit commands
    useMCPHandler(showVerbEditor);

    // Handle closing presentations
    const handleClosePresentation = useCallback((id: string) => {
        if (authState.player?.authToken) {
            dismissPresentation(id, authState.player.authToken);
        }
    }, [dismissPresentation, authState.player?.authToken]);

    // WebSocket integration
    const { wsState, connect: connectWS, sendMessage } = useWebSocketContext();

    // Handle login and WebSocket connection
    const handleConnect = async (mode: "connect" | "create", username: string, password: string) => {
        setLoginMode(mode);
        await connect(mode, username, password);
    };

    // Load history and connect WebSocket after authentication
    useEffect(() => {
        // Load history when player is authenticated and history not yet loaded
        if (authState.player && authState.player.authToken && !historyLoaded) {
            setHistoryLoaded(true);

            // Set history boundary before fetching
            setHistoryBoundaryNow();

            // Load initial history with dynamic sizing
            setTimeout(() => {
                // Wait a moment for the component to render and get actual size
                fetchInitialHistory()
                    .then(async (historicalMessages) => {
                        // Store historical messages to add later when narrative component is available
                        setPendingHistoricalMessages(historicalMessages);

                        showMessage("History loaded successfully", 2);

                        // Fetch current presentations BEFORE connecting WebSocket
                        try {
                            await fetchCurrentPresentations(authState.player!.authToken);
                        } catch (_error) {
                            // Continue even if presentations fail to load
                        }

                        // Connect WebSocket after history and presentations are loaded (if not already connected)
                        if (!wsState.isConnected) {
                            connectWS(loginMode);
                        }
                    })
                    .catch(async (_error) => {
                        showMessage("Failed to load history, continuing anyway...", 3);

                        // Still try to fetch presentations even if history fails
                        try {
                            await fetchCurrentPresentations(authState.player!.authToken);
                        } catch (_error) {
                            // Continue even if presentations fail to load
                        }

                        // Connect WebSocket even if history fails (if not already connected)
                        if (!wsState.isConnected) {
                            connectWS(loginMode);
                        }
                    });
            }, 100); // Wait 100ms for component to render
        }
    }, [
        authState.player?.authToken,
        historyLoaded,
        wsState.isConnected,
        connectWS,
        loginMode,
        setHistoryBoundaryNow,
        fetchInitialHistory,
        fetchCurrentPresentations,
        showMessage,
    ]);

    const isConnected = authState.player?.connected || false;

    // Add pending historical messages when narrative component becomes available
    useEffect(() => {
        if (narrativeRef.current && pendingHistoricalMessages.length > 0) {
            narrativeRef.current.addHistoricalMessages(pendingHistoricalMessages);
            setPendingHistoricalMessages([]); // Clear pending messages
        }
    }, [isConnected, pendingHistoricalMessages]);

    // Handle loading more history for infinite scroll
    const handleLoadMoreHistory = useCallback(async () => {
        if (!authState.player?.authToken || isLoadingHistory) {
            return;
        }

        try {
            const moreHistoricalMessages = await fetchMoreHistory();

            if (moreHistoricalMessages.length > 0) {
                narrativeRef.current?.prependHistoricalMessages(moreHistoricalMessages);
            }
        } catch (error) {
            // History loading failed, but we can continue without it
            console.warn("Failed to load more history:", error);
        }
    }, [authState.player?.authToken, isLoadingHistory, fetchMoreHistory]);

    return (
        <div style={{ height: "100vh", display: "flex", flexDirection: "column", overflow: "hidden" }}>
            {/* Main container (primarily for styling) */}
            <div className="main" />

            {/* System message notifications area (toast-style) */}
            <MessageBoard
                message={systemMessage.message}
                visible={systemMessage.visible}
            />

            {/* Theme toggle component */}
            <ThemeToggle />

            {/* Login component (shows/hides based on connection state) */}
            <Login
                visible={!isConnected}
                welcomeMessage={welcomeMessage}
                onConnect={handleConnect}
            />

            {/* Main app layout with narrative interface */}
            {isConnected && (
                <div className="app_layout">
                    {/* Top dock */}
                    <TopDock
                        presentations={getTopDockPresentations()}
                        onClosePresentation={handleClosePresentation}
                    />

                    {/* Middle section with left dock, narrative, right dock */}
                    <div className="middle_section">
                        <LeftDock
                            presentations={getLeftDockPresentations()}
                            onClosePresentation={handleClosePresentation}
                        />

                        {/* Main narrative interface - takes up full space */}
                        <Narrative
                            ref={narrativeCallbackRef}
                            visible={isConnected}
                            connected={isConnected}
                            onSendMessage={sendMessage}
                            onLoadMoreHistory={handleLoadMoreHistory}
                            isLoadingHistory={isLoadingHistory}
                        />

                        <RightDock
                            presentations={getRightDockPresentations()}
                            onClosePresentation={handleClosePresentation}
                        />
                    </div>

                    {/* Bottom dock */}
                    <BottomDock
                        presentations={getBottomDockPresentations()}
                        onClosePresentation={handleClosePresentation}
                    />
                </div>
            )}

            {/* Verb Editor Modal */}
            {editorSession && authState.player?.authToken && (
                <VerbEditor
                    visible={true}
                    onClose={closeEditor}
                    title={editorSession.title}
                    objectCurie={editorSession.objectCurie}
                    verbName={editorSession.verbName}
                    initialContent={editorSession.content}
                    authToken={authState.player.authToken}
                    uploadAction={editorSession.uploadAction}
                    onSendMessage={sendMessage}
                />
            )}
        </div>
    );
}

function App() {
    const { showMessage } = useSystemMessage();

    return (
        <AuthProvider showMessage={showMessage}>
            <PresentationProvider>
                <AppWrapper />
            </PresentationProvider>
        </AuthProvider>
    );
}

function AppWrapper() {
    const { authState, setPlayerConnected } = useAuthContext();
    const { addPresentation, removePresentation } = usePresentationContext();
    const { showMessage } = useSystemMessage();
    const narrativeRef = useRef<NarrativeRef>(null);
    const [pendingMessages, setPendingMessages] = useState<
        Array<{
            content: string | string[];
            contentType?: string;
        }>
    >([]);

    const handleNarrativeMessage = (
        content: string | string[],
        _timestamp?: string,
        contentType?: string,
        _isHistorical?: boolean,
    ) => {
        if (narrativeRef.current) {
            narrativeRef.current.addNarrativeContent(content, contentType as "text/plain" | "text/djot" | "text/html");
        } else {
            // Queue message if narrative ref is not available yet
            setPendingMessages(prev => [...prev, { content, contentType }]);
        }
    };

    const handlePresentMessage = (presentData: PresentationData) => {
        addPresentation(presentData);
    };

    const handleUnpresentMessage = (id: string) => {
        removePresentation(id);
    };

    // Process pending messages when narrative ref becomes available
    const narrativeCallbackRef = useCallback((node: NarrativeRef | null) => {
        if (node) {
            // Process any pending messages
            pendingMessages.forEach(({ content, contentType }) => {
                node.addNarrativeContent(content, contentType as "text/plain" | "text/djot" | "text/html");
            });
            if (pendingMessages.length > 0) {
                setPendingMessages([]);
            }
        }
        // Also store in the ref for other uses
        (narrativeRef as any).current = node;
    }, [pendingMessages]);

    return (
        <WebSocketProvider
            player={authState.player}
            showMessage={showMessage}
            setPlayerConnected={setPlayerConnected}
            handleNarrativeMessage={handleNarrativeMessage}
            handlePresentMessage={handlePresentMessage}
            handleUnpresentMessage={handleUnpresentMessage}
        >
            <AppContent narrativeRef={narrativeRef} narrativeCallbackRef={narrativeCallbackRef} />
        </WebSocketProvider>
    );
}

const rootElement = document.getElementById("root")!;

// Prevent duplicate rendering
if (!rootElement.hasChildNodes()) {
    const root = ReactDOM.createRoot(rootElement);
    root.render(<App />);
}
