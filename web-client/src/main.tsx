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
import { SettingsPanel } from "./components/SettingsPanel";
import { TopNavBar } from "./components/TopNavBar";
import { VerbEditor } from "./components/VerbEditor";
import { AuthProvider, useAuthContext } from "./context/AuthContext";
import { PresentationProvider, usePresentationContext } from "./context/PresentationContext";
import { useWebSocketContext, WebSocketProvider } from "./context/WebSocketContext";
import { useHistory } from "./hooks/useHistory";
import { useMCPHandler } from "./hooks/useMCPHandler";
import { useMediaQuery } from "./hooks/useMediaQuery";
import { useVerbEditor } from "./hooks/useVerbEditor";
import { MoorRemoteObject } from "./lib/rpc";
import { oidRef } from "./lib/var";
import { PresentationData } from "./types/presentation";
import "./styles/main.css";

// Simple React App component to test the setup
function AppContent({
    narrativeRef,
    narrativeCallbackRef,
    onLinkClick,
    onVerbEditorReady,
}: {
    narrativeRef: React.RefObject<NarrativeRef>;
    narrativeCallbackRef: (node: NarrativeRef | null) => void;
    onLinkClick?: (url: string) => void;
    onVerbEditorReady?: (
        showVerbEditor: (
            title: string,
            objectCurie: string,
            verbName: string,
            content: string,
            uploadAction?: string,
        ) => void,
    ) => void;
}) {
    const { systemMessage, showMessage } = useSystemMessage();
    const { welcomeMessage, contentType } = useWelcomeMessage();
    const { authState, connect } = useAuthContext();
    const [loginMode, setLoginMode] = useState<"connect" | "create">("connect");
    const [historyLoaded, setHistoryLoaded] = useState(false);
    const [pendingHistoricalMessages, setPendingHistoricalMessages] = useState<any[]>([]);
    const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
    const [splitRatio, setSplitRatio] = useState(() => {
        // Load saved split ratio or default to 60% for room, 40% for editor
        const saved = localStorage.getItem("moor-split-ratio");
        return saved ? parseFloat(saved) : 0.6;
    });

    const splitRatioRef = useRef(splitRatio);
    splitRatioRef.current = splitRatio;

    const isMobile = useMediaQuery("(max-width: 768px)");
    const [forceSplitMode, setForceSplitMode] = useState(false);

    const toggleSplitMode = useCallback(() => {
        setForceSplitMode(prev => !prev);
    }, []);

    // Verb editor state (only used in this component for the modal)
    const {
        editorSession,
        closeEditor,
        showVerbEditor,
    } = useVerbEditor();

    // Notify parent about verb editor availability
    useEffect(() => {
        if (onVerbEditorReady) {
            onVerbEditorReady(showVerbEditor);
        }
    }, [onVerbEditorReady, showVerbEditor]);

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

    // MCP handler for parsing edit commands - passed from parent
    // (We receive the handler instead of creating it here)

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

    // Track if we were previously connected to distinguish reconnection from initial connection
    const wasConnectedRef = useRef(false);

    // Reset history loaded flag when WebSocket disconnects to ensure history is refetched on reconnection
    // Only reset if we were previously connected (not during initial connection flow)
    useEffect(() => {
        if (wsState.connectionStatus === "connected") {
            wasConnectedRef.current = true;
        } else if (wsState.connectionStatus === "disconnected" && wasConnectedRef.current && historyLoaded) {
            setHistoryLoaded(false);
            wasConnectedRef.current = false;
        }
    }, [wsState.connectionStatus, historyLoaded]);

    // Handle split divider dragging
    const [isDraggingSplit, setIsDraggingSplit] = useState(false);

    const handleSplitMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        setIsDraggingSplit(true);
        e.preventDefault();
        e.stopPropagation(); // Prevent other drag handlers from interfering
    }, []);

    const handleSplitTouchStart = useCallback((e: React.TouchEvent) => {
        setIsDraggingSplit(true);
        e.preventDefault();
        e.stopPropagation();
    }, []);

    // Add global mouse event listeners for split dragging
    useEffect(() => {
        if (!isDraggingSplit) return;

        const updateSplitRatio = (clientY: number) => {
            // Get the main app layout element to calculate relative position
            const mainElement = document.querySelector(".app_layout") as HTMLElement;
            if (!mainElement) return;

            const rect = mainElement.getBoundingClientRect();
            const relativeY = clientY - rect.top;
            const newRatio = relativeY / rect.height;
            const clampedRatio = Math.max(0.2, Math.min(0.8, newRatio)); // Keep between 20% and 80%

            setSplitRatio(clampedRatio);
        };

        const handleMouseMove = (e: MouseEvent) => {
            updateSplitRatio(e.clientY);
        };

        const handleTouchMove = (e: TouchEvent) => {
            if (e.touches.length === 0) return; // Safety check
            e.preventDefault(); // Prevent scrolling
            e.stopPropagation(); // Prevent other handlers from processing this
            updateSplitRatio(e.touches[0].clientY);
        };

        const endDrag = () => {
            setIsDraggingSplit(false);
            // Save the split ratio to localStorage
            localStorage.setItem("moor-split-ratio", splitRatioRef.current.toString());
        };

        document.addEventListener("mousemove", handleMouseMove);
        document.addEventListener("mouseup", endDrag);
        document.addEventListener("touchmove", handleTouchMove, { passive: false, capture: true });
        document.addEventListener("touchend", endDrag, { capture: true });
        document.body.style.cursor = "row-resize";
        document.body.style.userSelect = "none";

        return () => {
            document.removeEventListener("mousemove", handleMouseMove);
            document.removeEventListener("mouseup", endDrag);
            document.removeEventListener("touchmove", handleTouchMove, { capture: true } as any);
            document.removeEventListener("touchend", endDrag, { capture: true } as any);
            document.body.style.cursor = "";
            document.body.style.userSelect = "";
        };
    }, [isDraggingSplit]);

    const isConnected = authState.player?.connected || false;
    const isSplitMode = isConnected && editorSession && (isMobile || forceSplitMode);

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
            {/* Screen reader heading for main application */}
            <h1 className="sr-only">mooR Web Client</h1>

            {/* Main container (primarily for styling) */}
            <div className="main" />

            {/* System message notifications area (toast-style) */}
            <MessageBoard
                message={systemMessage.message}
                visible={systemMessage.visible}
            />

            {/* Login component (shows/hides based on connection state) */}
            <Login
                visible={!isConnected}
                welcomeMessage={welcomeMessage}
                contentType={contentType}
                onConnect={handleConnect}
            />

            {/* Top navigation bar */}
            <TopNavBar onSettingsToggle={() => setIsSettingsOpen(true)} />

            {/* Settings panel */}
            <SettingsPanel
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
            />

            {/* Main app layout with narrative interface */}
            {isConnected && (
                <main
                    className="app_layout"
                    role="main"
                    style={{
                        display: "flex",
                        flexDirection: "column",
                        flex: 1,
                        overflow: "hidden",
                    }}
                >
                    {/* Room/Narrative Section */}
                    <div
                        style={{
                            height: isSplitMode ? `${splitRatio * 100}%` : "100%",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                        }}
                    >
                        {/* Top dock */}
                        <aside role="complementary" aria-label="Top dock panels">
                            <TopDock
                                presentations={getTopDockPresentations()}
                                onClosePresentation={handleClosePresentation}
                                onLinkClick={onLinkClick}
                            />
                        </aside>

                        {/* Middle section with left dock, narrative, right dock */}
                        <div className="middle_section">
                            <aside role="complementary" aria-label="Left dock panels">
                                <LeftDock
                                    presentations={getLeftDockPresentations()}
                                    onClosePresentation={handleClosePresentation}
                                    onLinkClick={onLinkClick}
                                />
                            </aside>

                            {/* Main narrative interface - takes up full space */}
                            <section role="main" aria-label="Game narrative and input">
                                <Narrative
                                    ref={narrativeCallbackRef}
                                    visible={isConnected}
                                    connected={isConnected}
                                    onSendMessage={sendMessage}
                                    onLoadMoreHistory={handleLoadMoreHistory}
                                    isLoadingHistory={isLoadingHistory}
                                    onLinkClick={onLinkClick}
                                />
                            </section>

                            <aside role="complementary" aria-label="Right dock panels">
                                <RightDock
                                    presentations={getRightDockPresentations()}
                                    onClosePresentation={handleClosePresentation}
                                    onLinkClick={onLinkClick}
                                />
                            </aside>
                        </div>

                        {/* Bottom dock */}
                        <aside role="complementary" aria-label="Bottom dock panels">
                            <BottomDock
                                presentations={getBottomDockPresentations()}
                                onClosePresentation={handleClosePresentation}
                                onLinkClick={onLinkClick}
                            />
                        </aside>
                    </div>

                    {/* Editor Section (in split mode) */}
                    {isSplitMode && editorSession && authState.player?.authToken && (
                        <div
                            style={{
                                height: `${(1 - splitRatio) * 100}%`,
                                display: "flex",
                                flexDirection: "column",
                                overflow: "hidden",
                            }}
                        >
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
                                splitMode={true}
                                onSplitDrag={handleSplitMouseDown}
                                onSplitTouchStart={handleSplitTouchStart}
                                onToggleSplitMode={toggleSplitMode}
                                isInSplitMode={true}
                            />
                        </div>
                    )}
                </main>
            )}

            {/* Verb Editor Modal (fallback for non-split mode) */}
            {!isSplitMode && editorSession && authState.player?.authToken && (
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
                    onToggleSplitMode={toggleSplitMode}
                    isInSplitMode={false}
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

    // Store verb editor function from AppContent
    const showVerbEditorRef = useRef<
        ((title: string, objectCurie: string, verbName: string, content: string, uploadAction?: string) => void) | null
    >(null);

    // MCP handler for parsing edit commands
    const { handleNarrativeMessage: mcpHandler } = useMCPHandler(
        (title, objectCurie, verbName, content, uploadAction) => {
            if (showVerbEditorRef.current) {
                showVerbEditorRef.current(title, objectCurie, verbName, content, uploadAction);
            }
        },
    );

    // Handle MOO link clicks
    const handleLinkClick = useCallback(async (url: string) => {
        if (!authState.player?.authToken) {
            console.warn("Cannot handle link click: No auth token available");
            return;
        }

        try {
            const sysobj = new MoorRemoteObject(oidRef(0), authState.player.authToken);
            await sysobj.callVerb("handle_client_url", [url]);
            // The result comes through WebSocket narrative, so we don't need to handle the return value
        } catch (error) {
            console.error("Failed to handle link click:", error);
            showMessage(`Failed to handle link: ${error instanceof Error ? error.message : String(error)}`, 5);
        }
    }, [authState.player?.authToken, showMessage]);
    const [pendingMessages, setPendingMessages] = useState<
        Array<{
            content: string | string[];
            contentType?: string;
        }>
    >([]);

    const handleNarrativeMessage = useCallback((
        content: string | string[],
        _timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
    ) => {
        // Handle array content by processing each line
        if (Array.isArray(content)) {
            const filteredContent: string[] = [];
            for (const line of content) {
                // Check if this line is an MCP message that should be filtered
                if (!mcpHandler(line, isHistorical || false)) {
                    // If mcpHandler returns false, the line was not MCP-related and should be shown
                    filteredContent.push(line);
                }
            }

            // Only add content if there are non-MCP lines
            if (filteredContent.length > 0) {
                if (narrativeRef.current) {
                    narrativeRef.current.addNarrativeContent(
                        filteredContent,
                        contentType as "text/plain" | "text/djot" | "text/html",
                    );
                } else {
                    setPendingMessages(prev => [...prev, { content: filteredContent, contentType }]);
                }
            }
        } else {
            // Handle single string content
            if (!mcpHandler(content, isHistorical || false)) {
                // If mcpHandler returns false, the content was not MCP-related and should be shown
                if (narrativeRef.current) {
                    narrativeRef.current.addNarrativeContent(
                        content,
                        contentType as "text/plain" | "text/djot" | "text/html",
                    );
                } else {
                    setPendingMessages(prev => [...prev, { content, contentType }]);
                }
            }
        }
    }, [mcpHandler]);

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
            <AppContent
                narrativeRef={narrativeRef}
                narrativeCallbackRef={narrativeCallbackRef}
                onLinkClick={handleLinkClick}
                onVerbEditorReady={(fn) => {
                    showVerbEditorRef.current = fn;
                }}
            />
        </WebSocketProvider>
    );
}

const rootElement = document.getElementById("root")!;

// Prevent duplicate rendering
if (!rootElement.hasChildNodes()) {
    const root = ReactDOM.createRoot(rootElement);
    root.render(<App />);
}
