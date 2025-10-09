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
import { EncryptionPasswordPrompt } from "./components/EncryptionPasswordPrompt";
import { EncryptionSetupPrompt } from "./components/EncryptionSetupPrompt";
import { Login, useWelcomeMessage } from "./components/Login";
import { MessageBoard, useSystemMessage } from "./components/MessageBoard";
import { Narrative, NarrativeRef } from "./components/Narrative";
import { PropertyEditor } from "./components/PropertyEditor";
import { SettingsPanel } from "./components/SettingsPanel";
import { ThemeProvider } from "./components/ThemeProvider";
import { TopNavBar } from "./components/TopNavBar";
import { VerbEditor } from "./components/VerbEditor";
import { AuthProvider, useAuthContext } from "./context/AuthContext";
import { EncryptionProvider, useEncryptionContext } from "./context/EncryptionContext";
import { PresentationProvider, usePresentationContext } from "./context/PresentationContext";
import { useWebSocketContext, WebSocketProvider } from "./context/WebSocketContext";
import { useHistory } from "./hooks/useHistory";
import { useMCPHandler } from "./hooks/useMCPHandler";
import { useMediaQuery } from "./hooks/useMediaQuery";
import { usePropertyEditor } from "./hooks/usePropertyEditor";
import { useTitle } from "./hooks/useTitle";
import { useVerbEditor } from "./hooks/useVerbEditor";
import { OAuth2UserInfo } from "./lib/oauth2";
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
    onPropertyEditorReady,
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
    onPropertyEditorReady?: (
        showPropertyEditor: (
            title: string,
            objectCurie: string,
            propertyName: string,
            content: string,
            uploadAction?: string,
        ) => void,
    ) => void;
}) {
    const { systemMessage, showMessage } = useSystemMessage();
    const { welcomeMessage, contentType, isServerReady } = useWelcomeMessage();
    const { authState, connect } = useAuthContext();
    const { encryptionState, setupEncryption, forgetKey, getKeyForHistoryRequest } = useEncryptionContext();
    const systemTitle = useTitle();
    const [loginMode, setLoginMode] = useState<"connect" | "create">("connect");
    const [historyLoaded, setHistoryLoaded] = useState(false);
    const [pendingHistoricalMessages, setPendingHistoricalMessages] = useState<any[]>([]);
    const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
    const [showEncryptionSetup, setShowEncryptionSetup] = useState(false);
    const [showPasswordPrompt, setShowPasswordPrompt] = useState(false);
    const [userSkippedEncryption, setUserSkippedEncryption] = useState(false);
    const [oauth2UserInfo, setOAuth2UserInfo] = useState<OAuth2UserInfo | null>(null);
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
        launchVerbEditor,
        closeEditor,
        showVerbEditor,
    } = useVerbEditor();

    // Property editor state
    const {
        propertyEditorSession,
        closePropertyEditor,
        showPropertyEditor,
    } = usePropertyEditor();

    // Notify parent about verb editor availability
    useEffect(() => {
        if (onVerbEditorReady) {
            onVerbEditorReady(showVerbEditor);
        }
    }, [onVerbEditorReady, showVerbEditor]);

    // Notify parent about property editor availability
    useEffect(() => {
        if (onPropertyEditorReady) {
            onPropertyEditorReady(showPropertyEditor);
        }
    }, [onPropertyEditorReady, showPropertyEditor]);

    // History management
    // Get encryption key reactively - will update when encryption state changes
    const encryptionKeyForHistory = getKeyForHistoryRequest();

    // Debug logging for encryption key state
    useEffect(() => {
        console.log("[EncryptionDebug] Encryption state:", {
            hasEncryption: encryptionState.hasEncryption,
            isChecking: encryptionState.isChecking,
            hasAgeIdentity: !!encryptionState.ageIdentity,
            ageIdentityLength: encryptionState.ageIdentity?.length,
            encryptionKeyForHistory: encryptionKeyForHistory?.substring(0, 30) + "...",
        });
    }, [
        encryptionState.hasEncryption,
        encryptionState.isChecking,
        encryptionState.ageIdentity,
        encryptionKeyForHistory,
    ]);

    const {
        setHistoryBoundaryNow,
        fetchInitialHistory,
        fetchMoreHistory,
        isLoadingHistory,
    } = useHistory(authState.player?.authToken || null, encryptionKeyForHistory);

    // Presentation management
    const {
        getLeftDockPresentations,
        getRightDockPresentations,
        getTopDockPresentations,
        getBottomDockPresentations,
        getVerbEditorPresentations,
        dismissPresentation,
        fetchCurrentPresentations,
    } = usePresentationContext();

    // Custom close handler for verb editor that also dismisses presentation
    const handleVerbEditorClose = useCallback(() => {
        // Find and dismiss any verb editor presentations
        const verbEditorPresentations = getVerbEditorPresentations();
        if (verbEditorPresentations.length > 0 && authState.player?.authToken) {
            verbEditorPresentations.forEach(presentation => {
                dismissPresentation(presentation.id, authState.player!.authToken);
            });
        }
        closeEditor();
    }, [getVerbEditorPresentations, dismissPresentation, authState.player?.authToken, closeEditor]);

    // Handle verb editor presentations from server
    useEffect(() => {
        const verbEditorPresentations = getVerbEditorPresentations();

        // If we have verb editor presentations and no current editor session, launch the first one
        if (verbEditorPresentations.length > 0 && !editorSession && authState.player?.authToken) {
            const presentation = verbEditorPresentations[0];

            // Parse presentation attributes to extract object and verb information
            const objectCurie = presentation.attrs.object || presentation.attrs.objectCurie;
            const verbName = presentation.attrs.verb || presentation.attrs.verbName;

            if (objectCurie && verbName) {
                // Use launchVerbEditor to fetch content via REST API
                launchVerbEditor(
                    presentation.title,
                    objectCurie,
                    verbName,
                    authState.player.authToken,
                );
            }
        }

        // If editor session exists but no presentations, close the editor
        // (This handles the case where the presentation was dismissed)
        // Only close if this was a presentation-triggered editor (no uploadAction means it came from a presentation)
        if (editorSession && verbEditorPresentations.length === 0 && !editorSession.uploadAction) {
            closeEditor();
        }
    }, [getVerbEditorPresentations, editorSession, launchVerbEditor, closeEditor, authState.player?.authToken]);

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

    // Handle OAuth2 callback from URL parameters
    useEffect(() => {
        const urlParams = new URLSearchParams(window.location.search);

        // Check for OAuth2 user info (new user flow)
        const oauth2UserInfoParam = urlParams.get("oauth2_user_info");
        if (oauth2UserInfoParam) {
            try {
                const userInfo: OAuth2UserInfo = JSON.parse(decodeURIComponent(oauth2UserInfoParam));
                setOAuth2UserInfo(userInfo);
                showMessage(`OAuth2 login successful! Please choose how to proceed.`, 5);
                window.history.replaceState({}, document.title, window.location.pathname);
            } catch (error) {
                console.error("Failed to parse OAuth2 user info:", error);
                showMessage("OAuth2 callback error. Please try again.", 5);
            }
        }

        // Check for auth token (existing user flow)
        const authToken = urlParams.get("auth_token");
        const playerOid = urlParams.get("player");
        if (authToken && playerOid) {
            // Clear URL parameters immediately
            window.history.replaceState({}, document.title, window.location.pathname);

            // Store in sessionStorage so useAuth can pick it up
            sessionStorage.setItem("oauth2_auth_token", authToken);
            sessionStorage.setItem("oauth2_player_oid", playerOid);

            showMessage("Logged in successfully via OAuth2!", 2);
        }

        // Check for OAuth2 errors
        const error = urlParams.get("error");
        if (error) {
            const details = urlParams.get("details");
            showMessage(`OAuth2 error: ${error}${details ? ` - ${details}` : ""}`, 5);
            window.history.replaceState({}, document.title, window.location.pathname);
        }
    }, [showMessage]);

    // Handle login and WebSocket connection
    const handleConnect = async (mode: "connect" | "create", username: string, password: string) => {
        setLoginMode(mode);
        await connect(mode, username, password);
    };

    // Handle OAuth2 account choice
    const handleOAuth2AccountChoice = async (choice: {
        mode: "oauth2_create" | "oauth2_connect";
        provider: string;
        external_id: string;
        email?: string;
        name?: string;
        username?: string;
        player_name?: string;
        existing_email?: string;
        existing_password?: string;
    }) => {
        try {
            const response = await fetch("/auth/oauth2/account", {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                },
                body: JSON.stringify({
                    mode: choice.mode,
                    provider: choice.provider,
                    external_id: choice.external_id,
                    email: choice.email,
                    name: choice.name,
                    username: choice.username,
                    player_name: choice.player_name,
                    existing_email: choice.existing_email,
                    existing_password: choice.existing_password,
                }),
            });

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({ error: "Unknown error" }));
                showMessage(`Failed: ${errorData.error || response.statusText}`, 5);
                return;
            }

            const result = await response.json();

            if (result.success && result.auth_token && result.player) {
                // Clear OAuth2 user info
                setOAuth2UserInfo(null);

                // Store credentials in sessionStorage for useAuth to pick up
                sessionStorage.setItem("oauth2_auth_token", result.auth_token);
                sessionStorage.setItem("oauth2_player_oid", result.player);

                showMessage(`Account ${choice.mode === "oauth2_create" ? "created" : "linked"}! Connecting...`, 2);

                // Reload to trigger auth flow
                window.location.reload();
            } else {
                // Show specific error message if available
                const errorMsg = result.error || "Failed to complete account setup. Please try again.";
                showMessage(errorMsg, 5);
            }
        } catch (error) {
            console.error("OAuth2 account choice failed:", error);
            showMessage(`Error: ${error instanceof Error ? error.message : String(error)}`, 5);
        }
    };

    // Check encryption setup after login
    useEffect(() => {
        if (authState.player && !encryptionState.isChecking && !userSkippedEncryption) {
            const hasLocalKey = !!encryptionState.ageIdentity;
            const backendHasPubkey = encryptionState.hasEncryption;

            // If no local key but backend has pubkey, prompt for existing password (NOT setup!)
            if (!hasLocalKey && backendHasPubkey) {
                console.log("Backend has pubkey but no local key - prompting for existing password");
                if (!showPasswordPrompt) {
                    setShowPasswordPrompt(true);
                }
                // Make sure setup screen is NOT showing
                if (showEncryptionSetup) {
                    setShowEncryptionSetup(false);
                }
            } // If no local key and backend has no pubkey, prompt for new setup (NOT password!)
            else if (!hasLocalKey && !backendHasPubkey) {
                console.log("No encryption key anywhere - prompting for new setup");
                if (!showEncryptionSetup) {
                    setShowEncryptionSetup(true);
                }
                // Make sure password prompt is NOT showing
                if (showPasswordPrompt) {
                    setShowPasswordPrompt(false);
                }
            } // If we have a local key but backend doesn't have our pubkey (DB was reset), clear and re-prompt
            else if (hasLocalKey && !backendHasPubkey) {
                console.log(
                    "Backend missing pubkey but localStorage has key - clearing stale key and prompting for fresh setup",
                );
                forgetKey();
                setUserSkippedEncryption(false);
                setShowEncryptionSetup(true);
                setShowPasswordPrompt(false);
            } // If we have both local key and backend has pubkey, we're good - hide prompts
            else if (hasLocalKey && backendHasPubkey) {
                setShowEncryptionSetup(false);
                setShowPasswordPrompt(false);
            }
        }
    }, [
        authState.player,
        encryptionState.hasEncryption,
        encryptionState.ageIdentity,
        encryptionState.isChecking,
        showEncryptionSetup,
        showPasswordPrompt,
        forgetKey,
        userSkippedEncryption,
    ]);

    // Load history and connect WebSocket after authentication
    useEffect(() => {
        // Load history when player is authenticated, encryption status has been checked at least once, and history not yet loaded
        if (authState.player && authState.player.authToken && !historyLoaded && encryptionState.hasCheckedOnce) {
            // Encryption key is ALWAYS required - events cannot be logged or retrieved without it
            if (!encryptionKeyForHistory) {
                console.error(
                    "[HistoryError] No encryption key available. Cannot load history. User must set up encryption.",
                );
                // Still need to connect WebSocket, but skip history loading
                setHistoryLoaded(true); // Mark as "loaded" to prevent retrying
                if (!wsState.isConnected) {
                    setTimeout(() => connectWS(loginMode), 100);
                }
                return;
            }

            console.log("[HistoryDebug] Loading history with encryption key");
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
                            await fetchCurrentPresentations(authState.player!.authToken, encryptionKeyForHistory);
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
                            await fetchCurrentPresentations(authState.player!.authToken, encryptionKeyForHistory);
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
        encryptionState.hasCheckedOnce,
        encryptionKeyForHistory,
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
    const hasActiveEditor = editorSession || propertyEditorSession;
    const isSplitMode = isConnected && hasActiveEditor && (isMobile || forceSplitMode);

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
                isServerReady={isServerReady}
                onConnect={handleConnect}
                oauth2UserInfo={oauth2UserInfo}
                onOAuth2AccountChoice={handleOAuth2AccountChoice}
                onOAuth2Cancel={() => setOAuth2UserInfo(null)}
            />

            {/* Top navigation bar - only show when connected */}
            {isConnected && <TopNavBar onSettingsToggle={() => setIsSettingsOpen(true)} />}

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
                    {isSplitMode && authState.player?.authToken && (
                        <div
                            style={{
                                height: `${(1 - splitRatio) * 100}%`,
                                display: "flex",
                                flexDirection: "column",
                                overflow: "hidden",
                            }}
                        >
                            {editorSession && (
                                <VerbEditor
                                    visible={true}
                                    onClose={handleVerbEditorClose}
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
                            )}
                            {propertyEditorSession && (
                                <PropertyEditor
                                    visible={true}
                                    onClose={closePropertyEditor}
                                    title={propertyEditorSession.title}
                                    objectCurie={propertyEditorSession.objectCurie}
                                    propertyName={propertyEditorSession.propertyName}
                                    initialContent={propertyEditorSession.content}
                                    authToken={authState.player.authToken}
                                    uploadAction={propertyEditorSession.uploadAction}
                                    onSendMessage={sendMessage}
                                    splitMode={true}
                                    onSplitDrag={handleSplitMouseDown}
                                    onSplitTouchStart={handleSplitTouchStart}
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                    contentType={propertyEditorSession.contentType}
                                />
                            )}
                        </div>
                    )}
                </main>
            )}

            {/* Editor Modals (fallback for non-split mode) */}
            {!isSplitMode && editorSession && authState.player?.authToken && (
                <VerbEditor
                    visible={true}
                    onClose={handleVerbEditorClose}
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
            {!isSplitMode && propertyEditorSession && authState.player?.authToken && (
                <PropertyEditor
                    visible={true}
                    onClose={closePropertyEditor}
                    title={propertyEditorSession.title}
                    objectCurie={propertyEditorSession.objectCurie}
                    propertyName={propertyEditorSession.propertyName}
                    initialContent={propertyEditorSession.content}
                    authToken={authState.player.authToken}
                    uploadAction={propertyEditorSession.uploadAction}
                    onSendMessage={sendMessage}
                    onToggleSplitMode={toggleSplitMode}
                    isInSplitMode={false}
                    contentType={propertyEditorSession.contentType}
                />
            )}

            {showPasswordPrompt && (
                <EncryptionPasswordPrompt
                    systemTitle={systemTitle}
                    onUnlock={async (password) => {
                        const result = await setupEncryption(password);
                        if (result.success) {
                            setShowPasswordPrompt(false);
                            setUserSkippedEncryption(false);
                        }
                        return result;
                    }}
                    onForgotPassword={() => {
                        setShowPasswordPrompt(false);
                        setShowEncryptionSetup(true);
                    }}
                    onSkip={() => {
                        setShowPasswordPrompt(false);
                        setUserSkippedEncryption(true);
                    }}
                />
            )}

            {showEncryptionSetup && (
                <EncryptionSetupPrompt
                    systemTitle={systemTitle}
                    onSetup={async (password) => {
                        const result = await setupEncryption(password);
                        if (result.success) {
                            setShowEncryptionSetup(false);
                            setUserSkippedEncryption(false);
                        }
                        return result;
                    }}
                    onSkip={() => {
                        setShowEncryptionSetup(false);
                        setUserSkippedEncryption(true);
                    }}
                />
            )}
        </div>
    );
}

function App() {
    const { showMessage } = useSystemMessage();

    return (
        <ThemeProvider>
            <AuthProvider showMessage={showMessage}>
                <PresentationProvider>
                    <EncryptionWrapper />
                </PresentationProvider>
            </AuthProvider>
        </ThemeProvider>
    );
}

function EncryptionWrapper() {
    const { authState } = useAuthContext();

    return (
        <EncryptionProvider
            authToken={authState.player?.authToken || null}
            playerOid={authState.player?.oid || null}
        >
            <AppWrapper />
        </EncryptionProvider>
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

    // Store property editor function from AppContent
    const showPropertyEditorRef = useRef<
        | ((title: string, objectCurie: string, propertyName: string, content: string, uploadAction?: string) => void)
        | null
    >(null);

    // MCP handler for parsing edit commands
    const { handleNarrativeMessage: mcpHandler } = useMCPHandler(
        (title, objectCurie, verbName, content, uploadAction) => {
            if (showVerbEditorRef.current) {
                showVerbEditorRef.current(title, objectCurie, verbName, content, uploadAction);
            }
        },
        (title, objectCurie, propertyName, content, uploadAction) => {
            if (showPropertyEditorRef.current) {
                showPropertyEditorRef.current(title, objectCurie, propertyName, content, uploadAction);
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
            noNewline?: boolean;
        }>
    >([]);

    const handleNarrativeMessage = useCallback((
        content: string | string[],
        _timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
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
                        noNewline,
                    );
                } else {
                    setPendingMessages(prev => [...prev, { content: filteredContent, contentType, noNewline }]);
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
                        noNewline,
                    );
                } else {
                    setPendingMessages(prev => [...prev, { content, contentType, noNewline }]);
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
            pendingMessages.forEach(({ content, contentType, noNewline }) => {
                node.addNarrativeContent(content, contentType as "text/plain" | "text/djot" | "text/html", noNewline);
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
                onPropertyEditorReady={(fn) => {
                    showPropertyEditorRef.current = fn;
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
