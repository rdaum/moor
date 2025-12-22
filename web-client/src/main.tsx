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
import { PropertyValueEditorWindow } from "./components/PropertyValueEditorWindow";
import { SettingsPanel } from "./components/SettingsPanel";
import { TextEditor } from "./components/TextEditor";
import { ThemeProvider } from "./components/ThemeProvider";
import { TopNavBar } from "./components/TopNavBar";
import { VerbEditor } from "./components/VerbEditor";
import { AuthProvider, useAuthContext } from "./context/AuthContext";
import { EncryptionProvider, useEncryptionContext } from "./context/EncryptionContext";
import { PresentationProvider, usePresentationContext } from "./context/PresentationContext";
import { useWebSocketContext, WebSocketProvider } from "./context/WebSocketContext";
import { useHistory } from "./hooks/useHistory";
import { useMCPHandler } from "./hooks/useMCPHandler";
import { usePersistentState } from "./hooks/usePersistentState";
import { usePropertyEditor } from "./hooks/usePropertyEditor";
import { usePropertyValueEditor } from "./hooks/usePropertyValueEditor";
import { useTextEditor } from "./hooks/useTextEditor";
import { useTitle } from "./hooks/useTitle";
import { useTouchDevice } from "./hooks/useTouchDevice";
import { useVerbEditor } from "./hooks/useVerbEditor";
import { MoorVar } from "./lib/MoorVar";
import { OAuth2UserInfo } from "./lib/oauth2";
import { fetchServerFeatures, invokeVerbFlatBuffer } from "./lib/rpc-fb";
import { stringToCurie } from "./lib/var";
import { PresentationData } from "./types/presentation";
import "./styles/main.css";

// ObjFlag enum values (must match server-side ObjFlag::Programmer = 1)
const OBJFLAG_PROGRAMMER = 1 << 1; // Bit position 1 -> value 2

const serializeNumber = (value: number) => value.toString();
const deserializeNumber = (raw: string): number | null => {
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : null;
};

const clampNarrativeFontSize = (size: number) => Math.min(24, Math.max(10, size));
const serializeNarrativeFontSize = (value: number) => clampNarrativeFontSize(value).toString();
const deserializeNarrativeFontSize = (raw: string): number | null => {
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? clampNarrativeFontSize(parsed) : null;
};

// Simple React App component to test the setup
function AppContent({
    narrativeRef,
    narrativeCallbackRef,
    onVerbEditorReady,
    onPropertyEditorReady,
}: {
    narrativeRef: React.RefObject<NarrativeRef>;
    narrativeCallbackRef: (node: NarrativeRef | null) => void;
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
    const player = authState.player;
    const authToken = player?.authToken ?? null;
    const playerOid = player?.oid ?? null;
    const playerFlags = player?.flags;
    const isPlayerConnected = Boolean(player?.connected);
    const hasProgrammerAccess = Boolean(
        authToken && playerFlags !== undefined && (playerFlags & OBJFLAG_PROGRAMMER),
    );
    const [historyLoaded, setHistoryLoaded] = useState(false);
    const [pendingHistoricalMessages, setPendingHistoricalMessages] = useState<NarrativeMessage[]>([]);
    const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
    const [isAccountMenuOpen, setIsAccountMenuOpen] = useState<boolean>(false);
    const [isObjectBrowserOpen, setIsObjectBrowserOpen] = useState<boolean>(false);
    const [objectBrowserPresentationIds, setObjectBrowserPresentationIds] = useState<string[]>([]);
    const [objectBrowserLinkedToPresentation, setObjectBrowserLinkedToPresentation] = useState(false);
    const [objectBrowserFocusedObjectCurie, setObjectBrowserFocusedObjectCurie] = useState<string | undefined>();
    const [isEvalPanelOpen, setIsEvalPanelOpen] = useState<boolean>(false);
    const [showEncryptionSetup, setShowEncryptionSetup] = useState(false);
    const [showPasswordPrompt, setShowPasswordPrompt] = useState(false);
    const [userSkippedEncryption, setUserSkippedEncryption] = usePersistentState<boolean>(
        "moor-skip-encryption-setup",
        false,
    );
    const [oauth2UserInfo, setOAuth2UserInfo] = useState<OAuth2UserInfo | null>(null);
    const [splitRatio, setSplitRatio] = usePersistentState<number>(
        "moor-split-ratio",
        0.6,
        {
            serialize: serializeNumber,
            deserialize: deserializeNumber,
        },
    );
    const [unseenCount, setUnseenCount] = useState(0);
    const [eventLogEnabled, setEventLogEnabled] = useState<boolean | null>(null);
    const [hasShownHistoryUnavailable, setHasShownHistoryUnavailable] = useState(false);

    const splitRatioRef = useRef(splitRatio);
    splitRatioRef.current = splitRatio;

    const isTouchDevice = useTouchDevice();
    const [forceSplitMode, setForceSplitMode] = useState(false);
    const [isObjectBrowserDocked, setIsObjectBrowserDocked] = useState(() => isTouchDevice);
    const [isEvalPanelDocked, setIsEvalPanelDocked] = useState(() => isTouchDevice);
    const [narrativeFontSize, setNarrativeFontSize] = usePersistentState<number>(
        "moor-narrative-font-size",
        () => 14,
        {
            serialize: serializeNarrativeFontSize,
            deserialize: deserializeNarrativeFontSize,
        },
    );

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
        setNarrativeFontSize(prev => clampNarrativeFontSize(prev - 1));
    }, [setNarrativeFontSize]);

    const increaseNarrativeFontSize = useCallback(() => {
        setNarrativeFontSize(prev => clampNarrativeFontSize(prev + 1));
    }, [setNarrativeFontSize]);

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
        launchPropertyEditor,
        closePropertyEditor,
        showPropertyEditor,
    } = usePropertyEditor();
    const {
        propertyValueEditorSession,
        launchPropertyValueEditor,
        refreshPropertyValueEditor,
        closePropertyValueEditor,
    } = usePropertyValueEditor();

    const {
        textEditorSession,
        showTextEditor,
        closeTextEditor,
    } = useTextEditor();

    // Presentation management (needs to be declared before handlers that reference it)
    const {
        getLeftDockPresentations,
        getRightDockPresentations,
        getTopDockPresentations,
        getBottomDockPresentations,
        getVerbEditorPresentations,
        getPropertyEditorPresentations,
        getPropertyValueEditorPresentations,
        getObjectBrowserPresentations,
        getTextEditorPresentations,
        dismissPresentation,
        fetchCurrentPresentations,
        clearAll: clearAllPresentations,
    } = usePresentationContext();

    // Computed values (must be declared before any effects/callbacks that use them)
    const isConnected = isPlayerConnected;
    const hasPlayer = Boolean(player);
    const canUseObjectBrowser = Boolean(isConnected && hasProgrammerAccess);
    const verbEditorDocked = !!editorSession && (isTouchDevice || forceSplitMode);
    const propertyEditorDocked = !!propertyEditorSession && (isTouchDevice || forceSplitMode);
    const propertyValueEditorDocked = !!propertyValueEditorSession && (isTouchDevice || forceSplitMode);
    const textEditorDocked = !!textEditorSession && (isTouchDevice || forceSplitMode);
    const objectBrowserDocked = isObjectBrowserOpen && isObjectBrowserDocked;
    const evalPanelDocked = isEvalPanelOpen && isEvalPanelDocked;
    const isSplitMode = isConnected
        && (verbEditorDocked || propertyEditorDocked || propertyValueEditorDocked || textEditorDocked
            || objectBrowserDocked || evalPanelDocked);

    const handleOpenObjectBrowser = useCallback(() => {
        if (isTouchDevice) {
            closeEditor();
            closePropertyEditor();
            closePropertyValueEditor();
            closeTextEditor();
            if (!isObjectBrowserDocked) {
                setIsObjectBrowserDocked(true);
            }
        }
        setIsObjectBrowserOpen(true);
        setObjectBrowserLinkedToPresentation(false);
    }, [
        isTouchDevice,
        closeEditor,
        closePropertyEditor,
        closePropertyValueEditor,
        closeTextEditor,
        isObjectBrowserDocked,
    ]);

    const handleCloseObjectBrowser = useCallback(() => {
        if (authToken) {
            objectBrowserPresentationIds.forEach(id => dismissPresentation(id, authToken));
        }
        setIsObjectBrowserOpen(false);
        setObjectBrowserLinkedToPresentation(false);
    }, [authToken, dismissPresentation, objectBrowserPresentationIds]);

    const handleOpenEvalPanel = useCallback(() => {
        if (isTouchDevice) {
            closeEditor();
            closePropertyEditor();
            closePropertyValueEditor();
            closeTextEditor();
            if (!isEvalPanelDocked) {
                setIsEvalPanelDocked(true);
            }
        }
        setIsEvalPanelOpen(true);
    }, [isTouchDevice, closeEditor, closePropertyEditor, closePropertyValueEditor, closeTextEditor, isEvalPanelDocked]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (isObjectBrowserOpen) {
            closeEditor();
            closePropertyEditor();
            closePropertyValueEditor();
            closeTextEditor();
        }
    }, [
        isTouchDevice,
        isObjectBrowserOpen,
        closeEditor,
        closePropertyEditor,
        closePropertyValueEditor,
        closeTextEditor,
    ]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (
            (editorSession || propertyEditorSession || propertyValueEditorSession || textEditorSession)
            && isObjectBrowserOpen
        ) {
            setIsObjectBrowserOpen(false);
        }
    }, [
        isTouchDevice,
        editorSession,
        propertyEditorSession,
        propertyValueEditorSession,
        textEditorSession,
        isObjectBrowserOpen,
    ]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (isEvalPanelOpen) {
            closeEditor();
            closePropertyEditor();
            closePropertyValueEditor();
            closeTextEditor();
        }
    }, [isTouchDevice, isEvalPanelOpen, closeEditor, closePropertyEditor, closePropertyValueEditor, closeTextEditor]);

    useEffect(() => {
        if (!isTouchDevice) {
            return;
        }
        if (
            (editorSession || propertyEditorSession || propertyValueEditorSession || textEditorSession)
            && isEvalPanelOpen
        ) {
            setIsEvalPanelOpen(false);
        }
    }, [
        isTouchDevice,
        editorSession,
        propertyEditorSession,
        propertyValueEditorSession,
        textEditorSession,
        isEvalPanelOpen,
    ]);

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
        shouldShowDisconnectDivider,
    } = useHistory(authToken, encryptionKeyForHistory);

    // Custom close handler for verb editor that also dismisses presentation
    const handleVerbEditorClose = useCallback(() => {
        // If there are multiple sessions, close only the current one
        if (editorSessions.length > 1 && editorSession) {
            // Dismiss the presentation for the current session
            if (editorSession.presentationId && authToken) {
                dismissPresentation(editorSession.presentationId, authToken);
            }
            // Close just this session
            closeEditor(editorSession.id);
        } else {
            // Last session - dismiss all presentations and close all
            const verbEditorPresentations = getVerbEditorPresentations();
            if (verbEditorPresentations.length > 0 && authToken) {
                verbEditorPresentations.forEach(presentation => {
                    dismissPresentation(presentation.id, authToken);
                });
            }
            closeEditor();
        }
    }, [authToken, closeEditor, dismissPresentation, editorSession, editorSessions.length, getVerbEditorPresentations]);

    const propertyEditorPresentationId = propertyEditorSession?.presentationId;

    const handlePropertyEditorClose = useCallback(() => {
        if (propertyEditorPresentationId && authToken) {
            dismissPresentation(propertyEditorPresentationId, authToken);
        }
        closePropertyEditor();
    }, [authToken, closePropertyEditor, dismissPresentation, propertyEditorPresentationId]);

    const propertyValueEditorPresentationId = propertyValueEditorSession?.presentationId;

    const handlePropertyValueEditorClose = useCallback(() => {
        if (propertyValueEditorPresentationId && authToken) {
            dismissPresentation(propertyValueEditorPresentationId, authToken);
        }
        closePropertyValueEditor();
    }, [authToken, closePropertyValueEditor, dismissPresentation, propertyValueEditorPresentationId]);

    const textEditorPresentationId = textEditorSession?.presentationId;
    const closedTextEditorPresentationsRef = useRef<Set<string>>(new Set());

    const handleTextEditorClose = useCallback(() => {
        // Track this presentation as closed to prevent the effect from reopening it
        if (textEditorPresentationId) {
            closedTextEditorPresentationsRef.current.add(textEditorPresentationId);
        }

        // Notify the server by calling the verb with 'close symbol
        if (textEditorSession && authToken) {
            const closeArgs = MoorVar.buildTextEditorCloseArgs(textEditorSession.sessionId);
            invokeVerbFlatBuffer(
                authToken,
                textEditorSession.objectCurie,
                textEditorSession.verbName,
                closeArgs,
            ).catch(err => console.error("Failed to send close notification:", err));
        }

        closeTextEditor();
        if (textEditorPresentationId && authToken) {
            dismissPresentation(textEditorPresentationId, authToken);
        }
    }, [authToken, closeTextEditor, dismissPresentation, textEditorPresentationId, textEditorSession]);

    // Handle verb editor presentations from server
    useEffect(() => {
        const verbEditorPresentations = getVerbEditorPresentations();

        if (verbEditorPresentations.length > 0 && authToken) {
            for (const presentation of verbEditorPresentations) {
                const existingSession = editorSessions.find(s => s.presentationId === presentation.id);

                if (!existingSession) {
                    const rawObjectId = presentation.attrs.object || presentation.attrs.objectCurie;
                    const verbName = presentation.attrs.verb || presentation.attrs.verbName;

                    if (rawObjectId && verbName) {
                        const objectCurie = stringToCurie(rawObjectId);

                        launchVerbEditor(
                            presentation.title,
                            objectCurie,
                            verbName,
                            authToken,
                            presentation.id,
                        ).catch((error) => {
                            const errorMsg = `Failed to open verb editor: ${error.message}`;
                            console.log("[VerbEditor] Showing error:", errorMsg);
                            showMessage(errorMsg, 5);
                            if (authToken) {
                                dismissPresentation(presentation.id, authToken);
                            }
                        });
                    }
                }
            }
        }

        for (const session of editorSessions) {
            if (session.presentationId && !session.uploadAction) {
                const hasPresentation = verbEditorPresentations.some(p => p.id === session.presentationId);
                if (!hasPresentation) {
                    closeEditor(session.id);
                }
            }
        }
    }, [
        authToken,
        closeEditor,
        dismissPresentation,
        editorSessions,
        getVerbEditorPresentations,
        launchVerbEditor,
        showMessage,
    ]);

    // Handle property editor presentations from server
    useEffect(() => {
        if (!authToken) {
            return;
        }

        const propertyPresentations = getPropertyEditorPresentations();

        for (const presentation of propertyPresentations) {
            if (propertyEditorSession?.presentationId === presentation.id) {
                continue;
            }

            const rawObjectId = presentation.attrs.object || presentation.attrs.objectCurie;
            const propertyName = presentation.attrs.property || presentation.attrs.propertyName;

            if (!rawObjectId || !propertyName) {
                showMessage("Property editor presentation missing object/property metadata", 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            const objectCurie = stringToCurie(rawObjectId);
            if (!objectCurie) {
                showMessage(`Cannot parse object reference ${rawObjectId} for property editor`, 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            launchPropertyEditor(
                presentation.title || `${objectCurie}.${propertyName}`,
                objectCurie,
                propertyName,
                authToken,
                presentation.id,
            ).catch((error: unknown) => {
                const message = error instanceof Error ? error.message : String(error);
                const errorMsg = `Failed to open property editor: ${message}`;
                console.log("[PropertyEditor] Showing error:", errorMsg);
                showMessage(errorMsg, 5);
                dismissPresentation(presentation.id, authToken);
            });

            break;
        }

        if (propertyEditorSession?.presentationId) {
            const hasPresentation = propertyPresentations.some(
                presentation => presentation.id === propertyEditorSession.presentationId,
            );
            if (!hasPresentation) {
                closePropertyEditor();
            }
        }
    }, [
        authToken,
        closePropertyEditor,
        dismissPresentation,
        getPropertyEditorPresentations,
        launchPropertyEditor,
        propertyEditorSession,
        showMessage,
    ]);

    // Handle property value editor presentations from server
    useEffect(() => {
        if (!authToken) {
            return;
        }

        const valuePresentations = getPropertyValueEditorPresentations();

        for (const presentation of valuePresentations) {
            if (propertyValueEditorSession?.presentationId === presentation.id) {
                continue;
            }

            const rawObjectId = presentation.attrs.object || presentation.attrs.objectCurie;
            const propertyName = presentation.attrs.property || presentation.attrs.propertyName;

            if (!rawObjectId || !propertyName) {
                showMessage("Property value editor presentation missing object/property metadata", 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            const objectCurie = stringToCurie(rawObjectId);
            if (!objectCurie) {
                showMessage(`Cannot parse object reference ${rawObjectId} for property value editor`, 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            launchPropertyValueEditor(
                presentation.title || `${objectCurie}.${propertyName}`,
                objectCurie,
                propertyName,
                authToken,
                presentation.id,
            ).catch((error: unknown) => {
                const message = error instanceof Error ? error.message : String(error);
                const errorMsg = `Failed to open property value editor: ${message}`;
                console.log("[PropertyValueEditor] Showing error:", errorMsg);
                showMessage(errorMsg, 5);
                dismissPresentation(presentation.id, authToken);
            });

            break;
        }

        if (propertyValueEditorSession?.presentationId) {
            const hasPresentation = valuePresentations.some(
                presentation => presentation.id === propertyValueEditorSession.presentationId,
            );
            if (!hasPresentation) {
                closePropertyValueEditor();
            }
        }
    }, [
        authToken,
        closePropertyValueEditor,
        dismissPresentation,
        getPropertyValueEditorPresentations,
        launchPropertyValueEditor,
        propertyValueEditorSession,
        showMessage,
    ]);

    // Handle object browser presentations from server
    useEffect(() => {
        const objectPresentations = getObjectBrowserPresentations();
        setObjectBrowserPresentationIds(objectPresentations.map(presentation => presentation.id));

        if (objectPresentations.length === 0) {
            if (objectBrowserLinkedToPresentation) {
                setIsObjectBrowserOpen(false);
                setObjectBrowserLinkedToPresentation(false);
                setObjectBrowserFocusedObjectCurie(undefined);
            }
            return;
        }

        if (!canUseObjectBrowser) {
            objectPresentations.forEach(presentation => {
                showMessage("Object browser is unavailable for this account", 5);
                if (authToken) {
                    dismissPresentation(presentation.id, authToken);
                }
            });
            return;
        }

        // Use the most recent presentation and dismiss old ones
        const latestPresentation = objectPresentations[objectPresentations.length - 1];
        const objectCurie = latestPresentation.attrs.object || latestPresentation.attrs.objectCurie;
        if (objectCurie) {
            setObjectBrowserFocusedObjectCurie(objectCurie);
        }

        // Dismiss superseded presentations
        if (objectPresentations.length > 1 && authToken) {
            for (let i = 0; i < objectPresentations.length - 1; i++) {
                dismissPresentation(objectPresentations[i].id, authToken);
            }
        }

        // Open browser if not already open
        if (!isObjectBrowserOpen) {
            handleOpenObjectBrowser();
            setObjectBrowserLinkedToPresentation(true);
        }
    }, [
        authToken,
        canUseObjectBrowser,
        dismissPresentation,
        getObjectBrowserPresentations,
        handleOpenObjectBrowser,
        isObjectBrowserOpen,
        objectBrowserLinkedToPresentation,
        showMessage,
    ]);

    // Handle text editor presentations from server
    useEffect(() => {
        if (!authToken) {
            return;
        }

        const textPresentations = getTextEditorPresentations();

        for (const presentation of textPresentations) {
            if (textEditorSession?.presentationId === presentation.id) {
                continue;
            }

            // Skip presentations that were recently closed (prevents reopening race)
            if (closedTextEditorPresentationsRef.current.has(presentation.id)) {
                // Clean up once we've skipped it
                closedTextEditorPresentationsRef.current.delete(presentation.id);
                continue;
            }

            const rawObjectId = presentation.attrs.object || presentation.attrs.objectCurie;
            const verbName = presentation.attrs.verb || presentation.attrs.verbName;

            if (!rawObjectId || !verbName) {
                showMessage("Text editor presentation missing object/verb metadata", 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            const objectCurie = stringToCurie(rawObjectId);
            if (!objectCurie) {
                showMessage(`Cannot parse object reference ${rawObjectId} for text editor`, 5);
                dismissPresentation(presentation.id, authToken);
                continue;
            }

            // Get optional session ID
            const sessionId = presentation.attrs.session_id || undefined;

            // Get content type (default to text/plain)
            const contentType = presentation.attrs.content_type === "text/djot" ? "text/djot" : "text/plain";

            // Get text mode (default to list)
            const textMode = presentation.attrs.text_mode === "string" ? "string" : "list";

            // Get description (optional)
            const description = presentation.attrs.description || "";

            // Convert content to string (may be string or string[])
            const content = Array.isArray(presentation.content)
                ? presentation.content.join("\n")
                : (presentation.content || "");

            // Show the text editor with content from the presentation
            showTextEditor(
                presentation.id,
                presentation.title || "Edit Text",
                description,
                objectCurie,
                verbName,
                sessionId,
                content,
                contentType,
                textMode,
                presentation.id,
            );

            break;
        }

        if (textEditorSession?.presentationId) {
            const hasPresentation = textPresentations.some(
                presentation => presentation.id === textEditorSession.presentationId,
            );
            if (!hasPresentation) {
                closeTextEditor();
            }
        }
    }, [
        authToken,
        closeTextEditor,
        dismissPresentation,
        getTextEditorPresentations,
        showTextEditor,
        showMessage,
        textEditorSession,
    ]);

    // MCP handler for parsing edit commands - passed from parent
    // (We receive the handler instead of creating it here)

    // Handle closing presentations
    const handleClosePresentation = useCallback((id: string) => {
        if (authToken) {
            dismissPresentation(id, authToken);
        }
    }, [authToken, dismissPresentation]);

    // WebSocket integration
    const { wsState, connect: connectWS, disconnect: disconnectWS, sendMessage, inputMetadata, clearInputMetadata } =
        useWebSocketContext();

    // Handle MOO link clicks based on URL scheme
    const handleLinkClick = useCallback((url: string) => {
        if (url.startsWith("moo://cmd/")) {
            // Command link: send as if typed
            const command = decodeURIComponent(url.slice(10));
            sendMessage(command);
        } else if (url.startsWith("moo://inspect/")) {
            // Inspect link: show object info in popover (TODO)
            const oref = url.slice(14);
            console.log("Inspect link clicked:", oref);
            showMessage("Inspect not yet implemented", 2);
        } else if (url.startsWith("moo://help/")) {
            // Help link: show help in panel (TODO)
            const topic = decodeURIComponent(url.slice(11));
            console.log("Help link clicked:", topic);
            showMessage("Help links not yet implemented", 2);
        } else if (url.startsWith("http://") || url.startsWith("https://")) {
            // External link: open in new tab
            window.open(url, "_blank", "noopener,noreferrer");
        } else {
            console.warn("Unknown link scheme:", url);
        }
    }, [sendMessage, showMessage]);

    // Track previous player OID to detect logout
    const previousPlayerOidRef = useRef<string | null>(null);

    // Clean up all user-specific state when player logs out OR changes
    useEffect(() => {
        const currentPlayerOid = playerOid;

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
            closePropertyValueEditor();
            closeTextEditor();

            // Clear all presentations
            clearAllPresentations();

            // Clear narrative messages
            narrativeRef.current?.clearAll();

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
        clearAllPresentations,
        closeEditor,
        closePropertyEditor,
        closePropertyValueEditor,
        closeTextEditor,
        disconnectWS,
        narrativeRef,
        playerOid,
        wsState,
    ]);

    // Comprehensive logout handler
    const handleLogout = useCallback(() => {
        if (narrativeRef.current) {
            narrativeRef.current.clearAll();
        }
        setUnseenCount(0);
        disconnectWS("LOGOUT");
        // Reset the "skip encryption setup" flag so they see the prompt again if they log back in
        setUserSkippedEncryption(false);
        // Notify server of explicit logout (triggers user_disconnected if last connection)
        if (authState.player?.clientToken && authState.player?.clientId) {
            fetch("/auth/logout", {
                method: "POST",
                headers: {
                    "X-Moor-Auth-Token": authState.player.authToken,
                    "X-Moor-Client-Token": authState.player.clientToken,
                    "X-Moor-Client-Id": authState.player.clientId,
                },
            }).catch((e) => console.error("Failed to send logout notification:", e));
        }
        // Just disconnect from auth - the useEffect above will handle all cleanup
        disconnect();
    }, [disconnect, disconnectWS, narrativeRef, setUserSkippedEncryption, authState.player]);

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
        const authTokenParam = urlParams.get("auth_token");
        const playerOidParam = urlParams.get("player");
        const flagsParam = urlParams.get("flags");
        const clientTokenParam = urlParams.get("client_token");
        const clientIdParam = urlParams.get("client_id");
        if (authTokenParam && playerOidParam) {
            // Clear URL parameters immediately
            window.history.replaceState({}, document.title, window.location.pathname);

            // Store in localStorage so useAuth can pick it up (persists across reloads)
            localStorage.setItem("oauth2_auth_token", authTokenParam);
            localStorage.setItem("oauth2_player_oid", playerOidParam);
            if (flagsParam) {
                localStorage.setItem("oauth2_player_flags", flagsParam);
            }
            // Store client credentials for transparent reconnection
            if (clientTokenParam) {
                localStorage.setItem("client_token", clientTokenParam);
            }
            if (clientIdParam) {
                localStorage.setItem("client_id", clientIdParam);
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
                // Store client credentials for transparent reconnection
                if (result.client_token) {
                    localStorage.setItem("client_token", result.client_token);
                }
                if (result.client_id) {
                    localStorage.setItem("client_id", result.client_id);
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
        if (!authToken || eventLogEnabled === null) {
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

        if (eventLogEnabled && !historyLoaded) {
            if (!encryptionKeyForHistory) {
                console.error(
                    "[HistoryError] No encryption key available. Cannot load history. User must set up encryption.",
                );
                setHistoryLoaded(true);
                if (!wsState.isConnected) {
                    setTimeout(() => connectWS(loginMode), 100);
                }
                return;
            }

            console.log("[HistoryDebug] Loading history with encryption key");
            setHistoryLoaded(true);
            const lastMsgTimestamp = narrativeRef.current?.getLastMessageTimestamp() || 0;
            setHistoryBoundaryNow(lastMsgTimestamp);

            setTimeout(() => {
                fetchInitialHistory()
                    .then(async (historicalMessages) => {
                        setPendingHistoricalMessages(historicalMessages);

                        if (historicalMessages.length > 0) {
                            showMessage("History loaded successfully", 2);
                        }

                        try {
                            await fetchCurrentPresentations(authToken, encryptionKeyForHistory);
                        } catch {
                            // ignore
                        }

                        if (!wsState.isConnected) {
                            connectWS(loginMode);
                        }
                    })
                    .catch(async (_error) => {
                        showMessage("Failed to load history, continuing anyway...", 3);

                        try {
                            await fetchCurrentPresentations(authToken, encryptionKeyForHistory);
                        } catch {
                            // ignore
                        }

                        if (!wsState.isConnected) {
                            connectWS(loginMode);
                        }
                    });
            }, 100);
        }
    }, [
        authToken,
        connectWS,
        encryptionKeyForHistory,
        encryptionState.hasCheckedOnce,
        eventLogEnabled,
        fetchCurrentPresentations,
        fetchInitialHistory,
        historyLoaded,
        loginMode,
        setHistoryBoundaryNow,
        showMessage,
        wsState.isConnected,
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
    }, [isDraggingSplit, setSplitRatio]);

    useEffect(() => {
        if (!isConnected) {
            setUnseenCount(0);
        }
    }, [isConnected]);

    // Add pending historical messages when narrative component becomes available
    useEffect(() => {
        if (narrativeRef.current && pendingHistoricalMessages.length > 0) {
            narrativeRef.current.addHistoricalMessages(pendingHistoricalMessages);
            setPendingHistoricalMessages([]);
        }
    }, [isConnected, narrativeRef, pendingHistoricalMessages]);

    // Handle loading more history for infinite scroll
    const handleLoadMoreHistory = useCallback(async () => {
        if (!authToken || isLoadingHistory || eventLogEnabled === false) {
            return;
        }

        try {
            const moreHistoricalMessages = await fetchMoreHistory();

            if (moreHistoricalMessages.length > 0) {
                narrativeRef.current?.prependHistoricalMessages(moreHistoricalMessages);
            }
        } catch (error) {
            console.warn("Failed to load more history:", error);
        }
    }, [authToken, eventLogEnabled, fetchMoreHistory, isLoadingHistory, narrativeRef]);

    return (
        <div className="app-root">
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
                visible={!player}
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
                        onBrowserToggle={hasProgrammerAccess ? handleOpenObjectBrowser : undefined}
                        onEvalToggle={hasProgrammerAccess ? handleOpenEvalPanel : undefined}
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
                authToken={authToken}
                playerOid={playerOid}
            />

            {/* Main app layout with narrative interface */}
            {hasPlayer && (
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
                                onLinkClick={handleLinkClick}
                            />
                        </aside>

                        {/* Middle section with left dock, narrative, right dock */}
                        <div className="middle_section">
                            <aside role="complementary" aria-label="Left dock panels">
                                <LeftDock
                                    presentations={getLeftDockPresentations()}
                                    onClosePresentation={handleClosePresentation}
                                    onLinkClick={handleLinkClick}
                                />
                            </aside>

                            {/* Main narrative interface - takes up full space */}
                            <section aria-label="Game narrative">
                                <Narrative
                                    ref={narrativeCallbackRef}
                                    visible={hasPlayer}
                                    connectionStatus={wsState.connectionStatus}
                                    onSendMessage={sendMessage}
                                    onLoadMoreHistory={eventLogEnabled === false ? undefined : handleLoadMoreHistory}
                                    isLoadingHistory={eventLogEnabled === false ? false : isLoadingHistory}
                                    onLinkClick={handleLinkClick}
                                    playerOid={playerOid}
                                    onMessageAppended={handleMessageAppended}
                                    fontSize={narrativeFontSize}
                                    inputMetadata={inputMetadata}
                                    onClearInputMetadata={clearInputMetadata}
                                    shouldShowDisconnectDivider={shouldShowDisconnectDivider}
                                />
                            </section>

                            <aside role="complementary" aria-label="Right dock panels">
                                <RightDock
                                    presentations={getRightDockPresentations()}
                                    onClosePresentation={handleClosePresentation}
                                    onLinkClick={handleLinkClick}
                                />
                            </aside>
                        </div>

                        {/* Bottom dock */}
                        <aside role="complementary" aria-label="Bottom dock panels">
                            <BottomDock
                                presentations={getBottomDockPresentations()}
                                onClosePresentation={handleClosePresentation}
                                onLinkClick={handleLinkClick}
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
                    {isSplitMode && authToken && (
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
                                    authToken={authToken}
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
                                    onClose={handlePropertyEditorClose}
                                    title={propertyEditorSession.title}
                                    objectCurie={propertyEditorSession.objectCurie}
                                    propertyName={propertyEditorSession.propertyName}
                                    initialContent={propertyEditorSession.content}
                                    authToken={authToken}
                                    uploadAction={propertyEditorSession.uploadAction}
                                    onSendMessage={sendMessage}
                                    splitMode={true}
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                    contentType={propertyEditorSession.contentType}
                                />
                            )}
                            {propertyValueEditorDocked && propertyValueEditorSession && (
                                <PropertyValueEditorWindow
                                    visible={true}
                                    authToken={authToken}
                                    session={propertyValueEditorSession}
                                    onClose={handlePropertyValueEditorClose}
                                    onRefresh={() => refreshPropertyValueEditor(authToken)}
                                    splitMode={true}
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                />
                            )}
                            {textEditorDocked && textEditorSession && (
                                <TextEditor
                                    visible={true}
                                    onClose={handleTextEditorClose}
                                    title={textEditorSession.title}
                                    description={textEditorSession.description}
                                    objectCurie={textEditorSession.objectCurie}
                                    verbName={textEditorSession.verbName}
                                    sessionId={textEditorSession.sessionId}
                                    initialContent={textEditorSession.content}
                                    authToken={authToken}
                                    contentType={textEditorSession.contentType}
                                    textMode={textEditorSession.textMode}
                                    splitMode={true}
                                    onToggleSplitMode={toggleSplitMode}
                                    isInSplitMode={true}
                                />
                            )}
                            {isObjectBrowserOpen && objectBrowserDocked && canUseObjectBrowser && (
                                <ObjectBrowser
                                    key="object-browser-instance"
                                    visible={true}
                                    onClose={handleCloseObjectBrowser}
                                    authToken={authToken}
                                    splitMode={true}
                                    onToggleSplitMode={toggleObjectBrowserDock}
                                    isInSplitMode={true}
                                    focusedObjectCurie={objectBrowserFocusedObjectCurie}
                                    onOpenVerbInEditor={showVerbEditor}
                                />
                            )}
                            {evalPanelDocked && canUseObjectBrowser && (
                                <EvalPanel
                                    visible={isEvalPanelOpen}
                                    onClose={() => setIsEvalPanelOpen(false)}
                                    authToken={authToken}
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
            {authToken && !verbEditorDocked && editorSessions.map((session) => (
                <VerbEditor
                    key={session.id}
                    visible={true}
                    onClose={() => {
                        closeEditor(session.id);
                        if (session.presentationId && authToken) {
                            const verbEditorPresentations = getVerbEditorPresentations();
                            const presentation = verbEditorPresentations.find(p => p.id === session.presentationId);
                            if (presentation) {
                                dismissPresentation(presentation.id, authToken);
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
            ))}
            {propertyEditorSession && authToken && !propertyEditorDocked && (
                <PropertyEditor
                    visible={true}
                    onClose={handlePropertyEditorClose}
                    title={propertyEditorSession.title}
                    objectCurie={propertyEditorSession.objectCurie}
                    propertyName={propertyEditorSession.propertyName}
                    initialContent={propertyEditorSession.content}
                    authToken={authToken}
                    uploadAction={propertyEditorSession.uploadAction}
                    onSendMessage={sendMessage}
                    onToggleSplitMode={toggleSplitMode}
                    isInSplitMode={false}
                    contentType={propertyEditorSession.contentType}
                />
            )}
            {propertyValueEditorSession && authToken && !propertyValueEditorDocked && (
                <PropertyValueEditorWindow
                    visible={true}
                    authToken={authToken}
                    session={propertyValueEditorSession}
                    onClose={handlePropertyValueEditorClose}
                    onRefresh={() => refreshPropertyValueEditor(authToken)}
                    onToggleSplitMode={toggleSplitMode}
                    isInSplitMode={false}
                />
            )}
            {textEditorSession && authToken && !textEditorDocked && (
                <TextEditor
                    visible={true}
                    onClose={handleTextEditorClose}
                    title={textEditorSession.title}
                    description={textEditorSession.description}
                    objectCurie={textEditorSession.objectCurie}
                    verbName={textEditorSession.verbName}
                    sessionId={textEditorSession.sessionId}
                    initialContent={textEditorSession.content}
                    authToken={authToken}
                    contentType={textEditorSession.contentType}
                    textMode={textEditorSession.textMode}
                    onToggleSplitMode={toggleSplitMode}
                    isInSplitMode={false}
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
                            setHistoryLoaded(false);
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
                            setHistoryLoaded(false);
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
            {isObjectBrowserOpen && !objectBrowserDocked && canUseObjectBrowser && authToken && (
                <ObjectBrowser
                    key="object-browser-instance"
                    visible={true}
                    onClose={handleCloseObjectBrowser}
                    authToken={authToken}
                    splitMode={false}
                    onToggleSplitMode={toggleObjectBrowserDock}
                    isInSplitMode={false}
                    focusedObjectCurie={objectBrowserFocusedObjectCurie}
                    onOpenVerbInEditor={showVerbEditor}
                />
            )}

            {/* Eval Panel - floating mode */}
            {isEvalPanelOpen && !evalPanelDocked && canUseObjectBrowser && authToken && (
                <EvalPanel
                    visible={true}
                    onClose={() => setIsEvalPanelOpen(false)}
                    authToken={authToken}
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
    const narrativeRef = useRef<NarrativeRef | null>(null);

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

    const [pendingMessages, setPendingMessages] = useState<
        Array<{
            content: string | string[];
            contentType?: string;
            noNewline?: boolean;
            presentationHint?: string;
            groupId?: string;
            ttsText?: string;
            thumbnail?: { contentType: string; data: string };
            linkPreview?: import("./lib/rpc-fb").LinkPreview;
            eventMetadata?: import("./lib/rpc-fb").EventMetadata;
        }>
    >([]);

    const handleNarrativeMessage = useCallback((
        content: string | string[],
        _timestamp?: string,
        contentType?: string,
        isHistorical?: boolean,
        noNewline?: boolean,
        presentationHint?: string,
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
        linkPreview?: import("./lib/rpc-fb").LinkPreview,
        eventMetadata?: import("./lib/rpc-fb").EventMetadata,
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
                        groupId,
                        ttsText,
                        thumbnail,
                        linkPreview,
                        eventMetadata,
                    );
                } else {
                    setPendingMessages(
                        prev => [...prev, {
                            content: filteredContent,
                            contentType,
                            noNewline,
                            presentationHint,
                            groupId,
                            ttsText,
                            thumbnail,
                            linkPreview,
                            eventMetadata,
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
                        groupId,
                        ttsText,
                        thumbnail,
                        linkPreview,
                        eventMetadata,
                    );
                } else {
                    setPendingMessages(
                        prev => [...prev, {
                            content,
                            contentType,
                            noNewline,
                            presentationHint,
                            groupId,
                            ttsText,
                            thumbnail,
                            linkPreview,
                            eventMetadata,
                        }],
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
            pendingMessages.forEach(
                (
                    {
                        content,
                        contentType,
                        noNewline,
                        presentationHint,
                        groupId,
                        ttsText,
                        thumbnail,
                        linkPreview,
                        eventMetadata,
                    },
                ) => {
                    node.addNarrativeContent(
                        content,
                        contentType as "text/plain" | "text/djot" | "text/html",
                        noNewline,
                        presentationHint,
                        groupId,
                        ttsText,
                        thumbnail,
                        linkPreview,
                        eventMetadata,
                    );
                },
            );
            if (pendingMessages.length > 0) {
                setPendingMessages([]);
            }
        }
        narrativeRef.current = node;
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
