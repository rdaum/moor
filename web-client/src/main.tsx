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
import { AccountMenu } from "./components/AccountMenu";
import { BottomDock } from "./components/docks/BottomDock";
import { LeftDock } from "./components/docks/LeftDock";
import { RightDock } from "./components/docks/RightDock";
import { TopDock } from "./components/docks/TopDock";
import { EncryptionPasswordPrompt } from "./components/EncryptionPasswordPrompt";
import { EncryptionSetupPrompt } from "./components/EncryptionSetupPrompt";
import { EvalPanel } from "./components/EvalPanel";
import { Login, useWelcomeMessage } from "./components/Login";
import { MessageBoard, useSystemMessage } from "./components/MessageBoard";
import { Narrative, NarrativeMessage, NarrativeRef } from "./components/Narrative";
import { ObjectBrowser } from "./components/ObjectBrowser";
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
import { usePropertyEditor } from "./hooks/usePropertyEditor";
import { useTitle } from "./hooks/useTitle";
import { useTouchDevice } from "./hooks/useTouchDevice";
import { useVerbEditor } from "./hooks/useVerbEditor";
import { OAuth2UserInfo } from "./lib/oauth2";
import { MoorRemoteObject } from "./lib/rpc";
import { fetchServerFeatures } from "./lib/rpc-fb";
import { oidRef } from "./lib/var";
import { PresentationData } from "./types/presentation";
import "./styles/main.css";

// ObjFlag enum values (must match server-side ObjFlag::Programmer = 1)
const OBJFLAG_PROGRAMMER = 1 << 1; // Bit position 1 -> value 2

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
    const { authState, connect, disconnect } = useAuthContext();
    const { encryptionState, setupEncryption, forgetKey, getKeyForHistoryRequest } = useEncryptionContext();
    const systemTitle = useTitle();
    const [loginMode, setLoginMode] = useState<"connect" | "create">("connect");
    const [historyLoaded, setHistoryLoaded] = useState(false);
    const [pendingHistoricalMessages, setPendingHistoricalMessages] = useState<NarrativeMessage[]>([]);
    const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
    const [isAccountMenuOpen, setIsAccountMenuOpen] = useState<boolean>(false);
    const [isObjectBrowserOpen, setIsObjectBrowserOpen] = useState<boolean>(false);
    const [isEvalPanelOpen, setIsEvalPanelOpen] = useState<boolean>(false);
    const [showEncryptionSetup, setShowEncryptionSetup] = useState(false);
    const [showPasswordPrompt, setShowPasswordPrompt] = useState(false);
    const [userSkippedEncryption, setUserSkippedEncryption] = useState(false);
    const [oauth2UserInfo, setOAuth2UserInfo] = useState<OAuth2UserInfo | null>(null);
    const [splitRatio, setSplitRatio] = useState(() => {
        // Load saved split ratio or default to 60% for room, 40% for editor
        const saved = localStorage.getItem("moor-split-ratio");
        return saved ? parseFloat(saved) : 0.6;
    });
    const [unseenCount, setUnseenCount] = useState(0);
    const [eventLogEnabled, setEventLogEnabled] = useState<boolean | null>(null);
    const [hasShownHistoryUnavailable, setHasShownHistoryUnavailable] = useState(false);

    const splitRatioRef = useRef(splitRatio);
    splitRatioRef.current = splitRatio;

    const isTouchDevice = useTouchDevice();
    const [forceSplitMode, setForceSplitMode] = useState(false);
    const [isObjectBrowserDocked, setIsObjectBrowserDocked] = useState(() => isTouchDevice);
    const [isEvalPanelDocked, setIsEvalPanelDocked] = useState(() => isTouchDevice);
    const [narrativeFontSize, setNarrativeFontSize] = useState(() => {
        const fallback = 14;
        if (typeof window === "undefined") {
            return fallback;
        }
        const stored = window.localStorage.getItem("moor-narrative-font-size");
        if (!stored) {
            return fallback;
        }
        const parsed = parseInt(stored, 10);
        if (!Number.isFinite(parsed)) {
            return fallback;
        }
        return Math.min(24, Math.max(10, parsed));
    });

    const toggleSplitMode = useCallback(() => {
        setForceSplitMode(prev => !prev);
    }, []);

    const toggleObjectBrowserDock = useCallback(() => {
        if (isTouchDevice) {
            return;
        }
        setIsObjectBrowserDocked(prev => !prev);
    }, [isTouchDevice]);

    const toggleEvalPanelDock = useCallback(() => {
        if (isTouchDevice) {
            return;
        }
        setIsEvalPanelDocked(prev => !prev);
    }, [isTouchDevice]);

    const decreaseNarrativeFontSize = useCallback(() => {
        setNarrativeFontSize(prev => Math.max(10, prev - 1));
    }, []);

    const increaseNarrativeFontSize = useCallback(() => {
        setNarrativeFontSize(prev => Math.min(24, prev + 1));
    }, []);

    useEffect(() => {
        if (isTouchDevice && !isObjectBrowserDocked) {
            setIsObjectBrowserDocked(true);
        }
    }, [isTouchDevice, isObjectBrowserDocked]);

    useEffect(() => {
        if (isTouchDevice && !isEvalPanelDocked) {
            setIsEvalPanelDocked(true);
        }
    }, [isTouchDevice, isEvalPanelDocked]);

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem("moor-narrative-font-size", narrativeFontSize.toString());
        }
    }, [narrativeFontSize]);

    const handleMessageAppended = useCallback((message: NarrativeMessage) => {
        if (typeof document === "undefined") {
            return;
        }

        if (message.isHistorical) {
            return;
        }

        const documentHasFocus = typeof document.hasFocus === "function" ? document.hasFocus() : true;

        if (!document.hidden && documentHasFocus) {
            return;
        }

        setUnseenCount(prev => Math.min(prev + 1, 99));
    }, []);

    useEffect(() => {
        if (typeof document === "undefined") {
            return;
        }

        const baseTitle = systemTitle || "mooR";
        const title = unseenCount > 0 ? `(${unseenCount}) ${baseTitle}` : baseTitle;
        document.title = title;
    }, [systemTitle, unseenCount]);

    useEffect(() => {
        if (typeof document === "undefined") {
            return;
        }

        const resetUnseen = () => {
            setUnseenCount(0);
        };

        const handleVisibilityChange = () => {
            if (!document.hidden) {
                resetUnseen();
            }
        };

        document.addEventListener("visibilitychange", handleVisibilityChange);
        window.addEventListener("focus", resetUnseen);

        return () => {
            document.removeEventListener("visibilitychange", handleVisibilityChange);
            window.removeEventListener("focus", resetUnseen);
        };
    }, []);

    useEffect(() => {
        let cancelled = false;
        fetchServerFeatures()
            .then((features) => {
                if (cancelled) {
                    return;
                }
                setEventLogEnabled(features.enableEventlog);
            })
            .catch((error) => {
                console.error("Failed to fetch server features:", error);
                if (!cancelled) {
                    setEventLogEnabled(true);
                }
            });

        return () => {
            cancelled = true;
        };
    }, []);

    useEffect(() => {
        if (!authState.player) {
            setHasShownHistoryUnavailable(false);
            return;
        }

        if (eventLogEnabled === false && !hasShownHistoryUnavailable) {
            showMessage("Message history is not available on this server", 4);
            setHasShownHistoryUnavailable(true);
        }
    }, [authState.player, eventLogEnabled, hasShownHistoryUnavailable, showMessage]);

    useEffect(() => {
        if (eventLogEnabled === false) {
            setShowEncryptionSetup(false);
            setShowPasswordPrompt(false);
        }
    }, [eventLogEnabled]);

    // Verb editor state (only used in this component for the modal)
    const {
        editorSession,
        editorSessions,
        activeSessionIndex,
        launchVerbEditor,
        closeEditor,
        showVerbEditor,
        previousSession,
        nextSession,
    } = useVerbEditor();

    // Property editor state
    const {
        propertyEditorSession,
        closePropertyEditor,
        showPropertyEditor,
    } = usePropertyEditor();

    const handleOpenObjectBrowser = useCallback(() => {
        if (isTouchDevice) {
            closeEditor();
            closePropertyEditor();
            if (!isObjectBrowserDocked) {
                setIsObjectBrowserDocked(true);
            }
        }
        setIsObjectBrowserOpen(true);
    }, [isTouchDevice, closeEditor, closePropertyEditor, isObjectBrowserDocked]);

    const handleOpenEvalPanel = useCallback(() => {
        if (isTouchDevice) {
            closeEditor();
            closePropertyEditor();
            if (!isEvalPanelDocked) {
                setIsEvalPanelDocked(true);
            }
        }
        setIsEvalPanelOpen(true);
    }, [isTouchDevice, closeEditor, closePropertyEditor, isEvalPanelDocked]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (isObjectBrowserOpen) {
            closeEditor();
            closePropertyEditor();
        }
    }, [isTouchDevice, isObjectBrowserOpen, closeEditor, closePropertyEditor]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if ((editorSession || propertyEditorSession) && isObjectBrowserOpen) {
            setIsObjectBrowserOpen(false);
        }
    }, [isTouchDevice, editorSession, propertyEditorSession, isObjectBrowserOpen]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (isEvalPanelOpen) {
            closeEditor();
            closePropertyEditor();
        }
    }, [isTouchDevice, isEvalPanelOpen, closeEditor, closePropertyEditor]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if ((editorSession || propertyEditorSession) && isEvalPanelOpen) {
            setIsEvalPanelOpen(false);
        }
    }, [isTouchDevice, editorSession, propertyEditorSession, isEvalPanelOpen]);

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
        clearAll: clearAllPresentations,
    } = usePresentationContext();

    // Custom close handler for verb editor that also dismisses presentation
    const handleVerbEditorClose = useCallback(() => {
        // If there are multiple sessions, close only the current one
        if (editorSessions.length > 1 && editorSession) {
            // Dismiss the presentation for the current session
            if (editorSession.presentationId && authState.player?.authToken) {
                dismissPresentation(editorSession.presentationId, authState.player.authToken);
            }
            // Close just this session
            closeEditor(editorSession.id);
        } else {
            // Last session - dismiss all presentations and close all
            const verbEditorPresentations = getVerbEditorPresentations();
            if (verbEditorPresentations.length > 0 && authState.player?.authToken) {
                verbEditorPresentations.forEach(presentation => {
                    dismissPresentation(presentation.id, authState.player!.authToken);
                });
            }
            closeEditor();
        }
    }, [
        editorSessions.length,
        editorSession,
        getVerbEditorPresentations,
        dismissPresentation,
        authState.player?.authToken,
        closeEditor,
    ]);

    // Handle verb editor presentations from server
    useEffect(() => {
        const verbEditorPresentations = getVerbEditorPresentations();

        // Process each presentation and launch editors for ones we don't have sessions for
        if (verbEditorPresentations.length > 0 && authState.player?.authToken) {
            for (const presentation of verbEditorPresentations) {
                // Check if we already have a session for this presentation
                const existingSession = editorSessions.find(s => s.presentationId === presentation.id);

                if (!existingSession) {
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
                            presentation.id,
                        ).catch((error) => {
                            // Show error message
                            const errorMsg = `Failed to open verb editor: ${error.message}`;
                            console.log("[VerbEditor] Showing error:", errorMsg);
                            showMessage(errorMsg, 5);
                            // Clean up the presentation
                            if (authState.player?.authToken) {
                                dismissPresentation(presentation.id, authState.player.authToken);
                            }
                        });
                    }
                }
            }
        }

        // Close any sessions that no longer have a corresponding presentation
        // Only close if this was a presentation-triggered editor (no uploadAction means it came from a presentation)
        for (const session of editorSessions) {
            if (session.presentationId && !session.uploadAction) {
                const hasPresentation = verbEditorPresentations.some(p => p.id === session.presentationId);
                if (!hasPresentation) {
                    closeEditor(session.id);
                }
            }
        }
    }, [
        getVerbEditorPresentations,
        editorSessions,
        launchVerbEditor,
        closeEditor,
        authState.player?.authToken,
        dismissPresentation,
        showMessage,
    ]);

    // MCP handler for parsing edit commands - passed from parent
    // (We receive the handler instead of creating it here)

    // Handle closing presentations
    const handleClosePresentation = useCallback((id: string) => {
        if (authState.player?.authToken) {
            dismissPresentation(id, authState.player.authToken);
        }
    }, [dismissPresentation, authState.player?.authToken]);

    // WebSocket integration
    const { wsState, connect: connectWS, disconnect: disconnectWS, sendMessage, inputMetadata, clearInputMetadata } =
        useWebSocketContext();

    // Track previous player OID to detect logout
    const previousPlayerOidRef = useRef<string | null>(null);

    // Clean up all user-specific state when player logs out OR changes
    useEffect(() => {
        const currentPlayerOid = authState.player?.oid || null;

        // Detect logout OR user switch: had a player, now different/none
        if (previousPlayerOidRef.current && previousPlayerOidRef.current !== currentPlayerOid) {
            console.log("[Cleanup] Player changed from", previousPlayerOidRef.current, "to", currentPlayerOid);
            console.log("[Cleanup] WebSocket state before disconnect:", wsState);

            // Disconnect WebSocket
            console.log("[Cleanup] Calling disconnectWS()");
            disconnectWS();
            console.log("[Cleanup] After disconnectWS(), wsState:", wsState);

            // Close any open editors
            closeEditor();
            closePropertyEditor();

            // Clear all presentations
            clearAllPresentations();

            // Clear narrative messages
            if (narrativeRef.current) {
                narrativeRef.current.clearAll();
            }

            // Reset local state
            setHistoryLoaded(false);
            setPendingHistoricalMessages([]);
            setShowEncryptionSetup(false);
            setShowPasswordPrompt(false);
            setUserSkippedEncryption(false);
            setOAuth2UserInfo(null);
        }

        previousPlayerOidRef.current = currentPlayerOid;
    }, [
        authState.player?.oid,
        disconnectWS,
        closeEditor,
        closePropertyEditor,
        clearAllPresentations,
    ]);

    // Comprehensive logout handler
    const handleLogout = useCallback(() => {
        if (narrativeRef.current) {
            narrativeRef.current.clearAll();
        }
        setUnseenCount(0);
        // Just disconnect from auth - the useEffect above will handle all cleanup
        disconnect();
    }, [disconnect, narrativeRef]);

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
        const flagsParam = urlParams.get("flags");
        if (authToken && playerOid) {
            // Clear URL parameters immediately
            window.history.replaceState({}, document.title, window.location.pathname);

            // Store in localStorage so useAuth can pick it up (persists across reloads)
            localStorage.setItem("oauth2_auth_token", authToken);
            localStorage.setItem("oauth2_player_oid", playerOid);
            if (flagsParam) {
                localStorage.setItem("oauth2_player_flags", flagsParam);
            }

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

                // Store credentials in localStorage for useAuth to pick it up (persists across reloads)
                localStorage.setItem("oauth2_auth_token", result.auth_token);
                localStorage.setItem("oauth2_player_oid", result.player);
                if (result.player_flags !== undefined) {
                    localStorage.setItem("oauth2_player_flags", result.player_flags.toString());
                }

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
        if (eventLogEnabled === false) {
            return;
        }

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
        eventLogEnabled,
    ]);

    // Load history and connect WebSocket after authentication
    useEffect(() => {
        if (!authState.player || !authState.player.authToken) {
            return;
        }

        if (eventLogEnabled === null) {
            return;
        }

        if (!historyLoaded && eventLogEnabled === false) {
            setHistoryLoaded(true);
            if (!wsState.isConnected) {
                setTimeout(() => connectWS(loginMode), 100);
            }
            return;
        }

        if (historyLoaded || !encryptionState.hasCheckedOnce) {
            return;
        }

        // Load history when player is authenticated, encryption status has been checked at least once, and history not yet loaded
        if (eventLogEnabled && authState.player.authToken && !historyLoaded) {
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
        eventLogEnabled,
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

        const touchMoveOptions: AddEventListenerOptions = { passive: false, capture: true };
        const touchEndOptions: EventListenerOptions = { capture: true };

        document.addEventListener("mousemove", handleMouseMove);
        document.addEventListener("mouseup", endDrag);
        document.addEventListener("touchmove", handleTouchMove, touchMoveOptions);
        document.addEventListener("touchend", endDrag, touchEndOptions);
        document.body.style.cursor = "row-resize";
        document.body.style.userSelect = "none";

        return () => {
            document.removeEventListener("mousemove", handleMouseMove);
            document.removeEventListener("mouseup", endDrag);
            document.removeEventListener("touchmove", handleTouchMove, touchMoveOptions);
            document.removeEventListener("touchend", endDrag, touchEndOptions);
            document.body.style.cursor = "";
            document.body.style.userSelect = "";
        };
    }, [isDraggingSplit]);

    const isConnected = authState.player?.connected || false;
    const verbEditorDocked = !!editorSession && (isTouchDevice || forceSplitMode);
    const propertyEditorDocked = !!propertyEditorSession && (isTouchDevice || forceSplitMode);
    const objectBrowserDocked = isObjectBrowserOpen && isObjectBrowserDocked;
    const evalPanelDocked = isEvalPanelOpen && isEvalPanelDocked;
    const isSplitMode = isConnected
        && (verbEditorDocked || propertyEditorDocked || objectBrowserDocked || evalPanelDocked);
    const canUseObjectBrowser = Boolean(
        isConnected
            && authState.player?.authToken
            && authState.player?.flags !== undefined
            && (authState.player.flags & OBJFLAG_PROGRAMMER),
    );

    useEffect(() => {
        if (!isConnected) {
            setUnseenCount(0);
        }
    }, [isConnected]);

    // Add pending historical messages when narrative component becomes available
    useEffect(() => {
        if (narrativeRef.current && pendingHistoricalMessages.length > 0) {
            narrativeRef.current.addHistoricalMessages(pendingHistoricalMessages);
            setPendingHistoricalMessages([]); // Clear pending messages
        }
    }, [isConnected, pendingHistoricalMessages]);

    // Handle loading more history for infinite scroll
    const handleLoadMoreHistory = useCallback(async () => {
        if (!authState.player?.authToken || isLoadingHistory || eventLogEnabled === false) {
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
    }, [authState.player?.authToken, isLoadingHistory, fetchMoreHistory, eventLogEnabled]);

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
            {isConnected && (
                <>
                    <TopNavBar
                        onSettingsToggle={() => setIsSettingsOpen(true)}
                        onAccountToggle={() => setIsAccountMenuOpen(true)}
                        onBrowserToggle={authState.player?.flags && (authState.player.flags & OBJFLAG_PROGRAMMER)
                            ? handleOpenObjectBrowser
                            : undefined}
                        onEvalToggle={authState.player?.flags && (authState.player.flags & OBJFLAG_PROGRAMMER)
                            ? handleOpenEvalPanel
                            : undefined}
                    />
                </>
            )}

            {/* Settings panel */}
            <SettingsPanel
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
                narrativeFontSize={narrativeFontSize}
                onDecreaseNarrativeFontSize={decreaseNarrativeFontSize}
                onIncreaseNarrativeFontSize={increaseNarrativeFontSize}
            />

            {/* Account menu */}
            <AccountMenu
                isOpen={isAccountMenuOpen}
                onClose={() => setIsAccountMenuOpen(false)}
                onLogout={handleLogout}
                historyAvailable={eventLogEnabled !== false}
                authToken={authState.player?.authToken ?? null}
                playerOid={authState.player?.oid ?? null}
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
                            flex: isSplitMode ? splitRatio : 1,
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                            minHeight: 0,
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
                                    onLoadMoreHistory={eventLogEnabled === false ? undefined : handleLoadMoreHistory}
                                    isLoadingHistory={eventLogEnabled === false ? false : isLoadingHistory}
                                    onLinkClick={onLinkClick}
                                    playerOid={authState.player?.oid}
                                    onMessageAppended={handleMessageAppended}
                                    fontSize={narrativeFontSize}
                                    inputMetadata={inputMetadata}
                                    onClearInputMetadata={clearInputMetadata}
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

                    {/* Split handle between narrative and editors */}
                    {isSplitMode && (
                        <div
                            role="separator"
                            aria-orientation="horizontal"
                            aria-label="Resize editor split"
                            onMouseDown={handleSplitMouseDown}
                            onTouchStart={handleSplitTouchStart}
                            style={{
                                height: "8px",
                                flex: "0 0 auto",
                                cursor: "row-resize",
                                background: "var(--color-border-medium)",
                                display: "flex",
                                alignItems: "center",
                                justifyContent: "center",
                                touchAction: "none",
                                borderTop: "1px solid var(--color-border-light)",
                                borderBottom: "1px solid var(--color-border-light)",
                            }}
                        >
                            <div
                                aria-hidden="true"
                                style={{
                                    width: "40px",
                                    height: "2px",
                                    borderRadius: "2px",
                                    backgroundColor: "var(--color-border-dark)",
                                }}
                            />
                        </div>
                    )}

                    {/* Editor Section (in split mode) */}
                    {isSplitMode && authState.player?.authToken && (
                        <div
                            style={{
                                flex: isSplitMode ? (1 - splitRatio) : 0,
                                display: "flex",
                                flexDirection: "column",
                                overflow: "hidden",
                                minHeight: 0,
                            }}
                        >
                            {verbEditorDocked && editorSession && (
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
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                    onPreviousEditor={previousSession}
                                    onNextEditor={nextSession}
                                    editorCount={editorSessions.length}
                                    currentEditorIndex={activeSessionIndex}
                                />
                            )}
                            {propertyEditorDocked && propertyEditorSession && (
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
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                    contentType={propertyEditorSession.contentType}
                                />
                            )}
                            {objectBrowserDocked && canUseObjectBrowser && (
                                <ObjectBrowser
                                    visible={isObjectBrowserOpen}
                                    onClose={() => setIsObjectBrowserOpen(false)}
                                    authToken={authState.player.authToken}
                                    splitMode={true}
                                    onToggleSplitMode={toggleObjectBrowserDock}
                                    isInSplitMode={true}
                                />
                            )}
                            {evalPanelDocked && canUseObjectBrowser && (
                                <EvalPanel
                                    visible={isEvalPanelOpen}
                                    onClose={() => setIsEvalPanelOpen(false)}
                                    authToken={authState.player.authToken}
                                    splitMode={true}
                                    onToggleSplitMode={toggleEvalPanelDock}
                                    isInSplitMode={true}
                                />
                            )}
                        </div>
                    )}
                </main>
            )}

            {/* Editor Modals (floating mode) - render all non-docked sessions */}
            {authState.player?.authToken && !verbEditorDocked && editorSessions.map((session) => {
                const authToken = authState.player!.authToken;
                return (
                    <VerbEditor
                        key={session.id}
                        visible={true}
                        onClose={() => {
                            // Close this specific session
                            closeEditor(session.id);
                            // Also dismiss presentation if it exists
                            if (session.presentationId && authState.player?.authToken) {
                                const verbEditorPresentations = getVerbEditorPresentations();
                                const presentation = verbEditorPresentations.find(p => p.id === session.presentationId);
                                if (presentation) {
                                    dismissPresentation(presentation.id, authState.player.authToken);
                                }
                            }
                        }}
                        title={session.title}
                        objectCurie={session.objectCurie}
                        verbName={session.verbName}
                        initialContent={session.content}
                        authToken={authToken}
                        uploadAction={session.uploadAction}
                        onSendMessage={sendMessage}
                        onToggleSplitMode={toggleSplitMode}
                        isInSplitMode={false}
                    />
                );
            })}
            {propertyEditorSession && authState.player?.authToken && !propertyEditorDocked && (
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

            {eventLogEnabled !== false && showPasswordPrompt && (
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

            {eventLogEnabled !== false && showEncryptionSetup && (
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

            {/* Object Browser - floating mode */}
            {isObjectBrowserOpen && !objectBrowserDocked && canUseObjectBrowser && authState.player?.authToken && (
                <ObjectBrowser
                    visible={true}
                    onClose={() => setIsObjectBrowserOpen(false)}
                    authToken={authState.player.authToken}
                    onToggleSplitMode={toggleObjectBrowserDock}
                    isInSplitMode={false}
                />
            )}

            {/* Eval Panel - floating mode */}
            {isEvalPanelOpen && !evalPanelDocked && canUseObjectBrowser && authState.player?.authToken && (
                <EvalPanel
                    visible={true}
                    onClose={() => setIsEvalPanelOpen(false)}
                    authToken={authState.player.authToken}
                    onToggleSplitMode={toggleEvalPanelDock}
                    isInSplitMode={false}
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
    const { authState, setPlayerConnected, setPlayerFlags } = useAuthContext();
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
            // TODO: Need to convert URL to FlatBuffer Var and pass as argument
            // For now, calling without arguments - the verb may not work correctly
            await sysobj.callVerb("handle_client_url");
            console.warn(`Link click handling called without URL argument: ${url}`);
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
            presentationHint?: string;
            thumbnail?: { contentType: string; data: string };
        }>
    >([]);

    const handleNarrativeMessage = useCallback((
        content: string | string[],
        _timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
        presentationHint?: string,
        thumbnail?: { contentType: string; data: string },
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
                        presentationHint,
                        thumbnail,
                    );
                } else {
                    setPendingMessages(
                        prev => [...prev, {
                            content: filteredContent,
                            contentType,
                            noNewline,
                            presentationHint,
                            thumbnail,
                        }],
                    );
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
                        presentationHint,
                        thumbnail,
                    );
                } else {
                    setPendingMessages(
                        prev => [...prev, { content, contentType, noNewline, presentationHint, thumbnail }],
                    );
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
            pendingMessages.forEach(({ content, contentType, noNewline, presentationHint, thumbnail }) => {
                node.addNarrativeContent(
                    content,
                    contentType as "text/plain" | "text/djot" | "text/html",
                    noNewline,
                    presentationHint,
                    thumbnail,
                );
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
            setPlayerFlags={setPlayerFlags}
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
