// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

import { decodeAnnotations, SemanticAnnotation } from "@moor/web-sdk";
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AccountMenu } from "../components/AccountMenu";
import { AnnotationCommandReview } from "../components/AnnotationCommandReview";
import { DialogSheet } from "../components/DialogSheet";
import { BottomDock } from "../components/docks/BottomDock";
import { LeftDock } from "../components/docks/LeftDock";
import { RightDock } from "../components/docks/RightDock";
import { TopDock } from "../components/docks/TopDock";
import { EncryptionPasswordPrompt } from "../components/EncryptionPasswordPrompt";
import { EncryptionSetupPrompt } from "../components/EncryptionSetupPrompt";
import { InspectPopover } from "../components/InspectPopover";
import {
    ChangeReview,
    ChangeWorkspace,
    EvalPanel,
    ObjectBrowser,
    PropertyEditor,
    PropertyValueEditorWindow,
    TextEditor,
    VerbEditor,
} from "../components/lazyEditors";
import { Login, useWelcomeMessage } from "../components/Login";
import { MessageBoard, useSystemMessage } from "../components/MessageBoard";
import { NarrativeRef } from "../components/Narrative";
import { Narrative } from "../components/Narrative";
import { MemberNavigation } from "../components/object-browser/types";
import { ProfileSetupPanel } from "../components/ProfileSetupPanel";
import { SettingsPanel } from "../components/SettingsPanel";
import { TopNavBar } from "../components/TopNavBar";
import { AnnotationActivation, AnnotationContext } from "../context/AnnotationContext";
import { ArgumentContext, ArgumentCoordinator } from "../context/ArgumentContext";
import { useAuthContext } from "../context/AuthContext";
import { useEncryptionContext } from "../context/EncryptionContext";
import { usePresentationContext } from "../context/PresentationContext";
import { useWebSocketContext } from "../context/WebSocketContext";
import { useInspectPopover } from "../hooks/useInspectPopover";
import { useNarrativeLinks } from "../hooks/useNarrativeLinks";
import { usePersistentState } from "../hooks/usePersistentState";
import { useServerFeatures } from "../hooks/useServerFeatures";
import { useSplitDrag } from "../hooks/useSplitDrag";
import { useTitle } from "../hooks/useTitle";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { useUnseenTitle } from "../hooks/useUnseenTitle";
import { type ChangeServiceClient, discoverChangeService } from "../lib/change-service";
import { MoorVar } from "../lib/MoorVar";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";
import { useNarrativePipelineContext } from "../session/SessionCoordinator";
import { usePlayerSwitch } from "../session/usePlayerSwitch";
import { useSessionControls } from "../session/useSessionControls";
import { useEncryptionReadiness } from "./useEncryptionReadiness";
import { useHistoryCoordinator } from "./useHistoryCoordinator";
import { usePresentationRouting } from "./usePresentationRouting";
import { useRoomHud } from "./useRoomHud";

// ObjFlag enum values (must match server-side ObjFlag::Programmer = 1)
const OBJFLAG_PROGRAMMER = 1 << 1; // Bit position 1 -> value 2
const OBJFLAG_WIZARD = 1 << 2;

const clampNarrativeFontSize = (size: number) => Math.min(24, Math.max(10, size));
const serializeNarrativeFontSize = (value: number) => clampNarrativeFontSize(value).toString();
const deserializeNarrativeFontSize = (raw: string): number | null => {
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? clampNarrativeFontSize(parsed) : null;
};

/**
 * The connected application surface. Composes the extracted owners (session,
 * history, encryption readiness, presentation routing, room HUD, navigation,
 * inspection) and renders the top-level layout: login, chrome, docks,
 * transcript, split editors, and floating windows.
 */
export const MainSurface: React.FC = () => {
    const { systemMessage, showMessage } = useSystemMessage();
    const { welcomeMessage, contentType, isServerReady } = useWelcomeMessage();
    const { authState, establishSession } = useAuthContext();
    const { encryptionState, getKeyForHistoryRequest } = useEncryptionContext();
    const { stateRevision, wsState, disconnect: disconnectWS, sendMessage, inputMetadata, clearInputMetadata } =
        useWebSocketContext();
    const { narrativeRef, narrativeCallbackRef } = useNarrativePipelineContext();

    const systemTitle = useTitle();
    const { eventLogEnabled } = useServerFeatures();
    const isTouchDevice = useTouchDevice();

    const player = authState.player;
    const authToken = player?.authToken ?? null;
    const playerOid = player?.oid ?? null;
    const historyPlayerOid = player?.historyOid ?? null;
    const playerFlags = player?.flags;
    const isConnected = Boolean(player?.connected);
    const hasProgrammerAccess = Boolean(
        authToken && playerFlags !== undefined && (playerFlags & OBJFLAG_PROGRAMMER),
    );
    const canUseObjectBrowser = Boolean(isConnected && hasProgrammerAccess);
    const hasPlayer = Boolean(player);

    // Presentation getters used directly by the dock layout
    const {
        dismissPresentation,
        addPresentation,
        getLeftDockPresentations,
        getRightDockPresentations,
        getBottomDockPresentations,
        getVerbEditorPresentations,
    } = usePresentationContext();

    // UI shell state
    const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
    const [isAccountMenuOpen, setIsAccountMenuOpen] = useState<boolean>(false);
    const [isObjectBrowserOpen, setIsObjectBrowserOpen] = useState<boolean>(false);
    const [isEvalPanelOpen, setIsEvalPanelOpen] = useState<boolean>(false);
    const [isChangesOpen, setIsChangesOpen] = useState(false);
    const [changeService, setChangeService] = useState<ChangeServiceClient | null>(null);
    const [changeConnectionRevision, setChangeConnectionRevision] = useState(0);
    useEffect(() => {
        setChangeService(null);
        setIsChangesOpen(false);
    }, [authToken]);
    useEffect(() => {
        if (!authToken || !isConnected) return;
        setChangeConnectionRevision(previous => previous + 1);
        let cancelled = false;
        void discoverChangeService(authToken).then(service => {
            if (!cancelled) setChangeService(service);
        }).catch(() => {
            // Cores may omit this service or deny access to the current player.
            // Retain an open workspace during transient failures so unsaved text survives.
        });
        return () => {
            cancelled = true;
        };
    }, [authToken, isConnected]);
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
    const [loginMode, setLoginMode] = useState<"connect" | "create">("connect");

    // History lifecycle
    const encryptionKeyForHistory = getKeyForHistoryRequest();
    const {
        isLoadingHistory,
        hasMoreHistory,
        handleLoadMoreHistory,
        markHistoryForReload,
        noteLiveActivity,
    } = useHistoryCoordinator({
        authToken,
        historyAuthToken: player?.historyAuthToken ?? null,
        encryptionKeyForHistory,
        encryptionHasCheckedOnce: encryptionState.hasCheckedOnce,
        encryptionStatusError: encryptionState.statusError,
        eventLogEnabled,
        loginMode,
        narrativeRef,
        showMessage,
    });

    // Unread badge + window title; live activity throttles history resyncs
    const { handleMessageAppended, resetUnseen } = useUnseenTitle(systemTitle, noteLiveActivity);

    useEffect(() => {
        if (!isConnected) {
            resetUnseen();
        }
    }, [isConnected, resetUnseen]);

    // Server-feature notices
    const [hasShownHistoryUnavailable, setHasShownHistoryUnavailable] = useState(false);

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

    // Encryption readiness prompts (needs history reload trigger)
    const {
        showEncryptionSetup,
        showPasswordPrompt,
        handleUnlock,
        handleSetup,
        handleForgotPassword,
        skipUnlock,
        skipSetup,
        resetForIdentityChange,
    } = useEncryptionReadiness(eventLogEnabled, markHistoryForReload);

    // Session entry points
    const {
        oauth2UserInfo,
        clearOAuth2UserInfo,
        handleOAuth2AccountChoice,
        handleConnect,
        handleLogout,
    } = useSessionControls(
        establishSession,
        showMessage,
        narrativeRef as React.RefObject<NarrativeRef | null>,
        resetForIdentityChange,
        loginMode,
        setLoginMode,
    );

    // Room HUD dock interpretation
    const {
        roomHudEnabled,
        setRoomHudEnabled,
        currentRoomLookKey,
        handleActiveRoomLookVisibilityChange,
        topDockPresentations,
        resetLatching,
    } = useRoomHud();

    // Object-browser open/close requests flow between presentation routing and
    // this surface through stable refs to avoid render-phase coupling
    const openObjectBrowserRef = useRef<() => void>(() => {});
    const closeObjectBrowserRef = useRef<() => void>(() => {});

    // Editor sessions and presentation interpretation
    const {
        editorSession,
        editorSessions,
        activeSessionIndex,
        previousSession,
        nextSession,
        showVerbEditor,
        handleVerbEditorClose,
        closeEditor,
        propertyEditorSession,
        handlePropertyEditorClose,
        propertyValueEditorSession,
        refreshPropertyValueEditor,
        handlePropertyValueEditorClose,
        textEditorSession,
        handleTextEditorClose,
        objectBrowserFocusedObjectCurie,
        dismissAllObjectBrowserPresentations,
        profileSetupPresentation,
        profileRefreshKey,
        handleProfileSetupComplete,
        handleProfileSetupSkip,
        resetForPlayerSwitch,
    } = usePresentationRouting({
        authToken,
        showMessage,
        canUseObjectBrowser,
        isObjectBrowserOpen,
        onOpenObjectBrowserRequested: useCallback(() => openObjectBrowserRef.current(), []),
        onObjectBrowserPresentationsCleared: useCallback(() => closeObjectBrowserRef.current(), []),
    });

    // Inspection popover + link routing
    const [argumentCoordinator] = useState(() => new ArgumentCoordinator());
    useEffect(() => {
        argumentCoordinator.configure(authToken, Boolean(inputMetadata));
    }, [argumentCoordinator, authToken, inputMetadata, stateRevision]);
    useEffect(() => () => argumentCoordinator.dispose(), [argumentCoordinator]);
    const inspect = useInspectPopover({ authToken, showMessage, refreshKey: stateRevision });
    const submitInspectionCommand = useCallback((command: string) => {
        if (inputMetadata) throw new Error("Finish the current prompt before running a command.");
        return narrativeRef.current?.submitCommand(command) ?? false;
    }, [inputMetadata, narrativeRef]);
    const { closeInspectPopover, inspectObject } = inspect;
    const { handleLinkClick } = useNarrativeLinks();

    const [memberReference, setMemberReference] = useState<string | null>(null);
    const [changeReview, setChangeReview] = useState<Extract<SemanticAnnotation, { kind: "change" }> | null>(null);
    const [memberCopyNotice, setMemberCopyNotice] = useState("");
    const [memberNavigation, setMemberNavigation] = useState<MemberNavigation | undefined>();
    const navigationSequence = useRef(0);
    const [annotationCommand, setAnnotationCommand] = useState<
        {
            annotation: Extract<SemanticAnnotation, { kind: "command" }>;
            position: { x: number; y: number };
        } | null
    >(null);
    useEffect(() => {
        setChangeReview(null);
        setAnnotationCommand(null);
        setMemberNavigation(undefined);
        setMemberReference(null);
    }, [authToken]);
    useEffect(() => {
        setMemberNavigation(undefined);
    }, [objectBrowserFocusedObjectCurie]);
    const helpRequest = useRef(0);
    useEffect(() => {
        helpRequest.current += 1;
    }, [authToken]);
    const handleAnnotationActivation = useCallback(({ annotation, position }: AnnotationActivation) => {
        if (annotation.kind === "change") {
            setChangeReview(annotation);
            return;
        }
        if (annotation.kind === "object" && !argumentCoordinator.select(annotation.ref)) {
            void inspectObject(annotation.ref, position);
        }
        if (annotation.kind === "command") {
            setAnnotationCommand({ annotation, position: { x: position.x, y: position.y + 8 } });
        }
        if (annotation.kind === "verb" || annotation.kind === "property") {
            if (!canUseObjectBrowser) {
                setMemberCopyNotice("");
                const object = annotation.kind === "verb" ? annotation.receiver : annotation.object;
                setMemberReference(
                    object.replace(/^(oid|uuid):/, "#") + (annotation.kind === "verb" ? ":" : ".") + annotation.name,
                );
                return;
            }
            setMemberNavigation({
                requestId: ++navigationSequence.current,
                object: annotation.kind === "verb" ? annotation.receiver : annotation.object,
                kind: annotation.kind,
                name: annotation.name,
                definer: annotation.kind === "verb" ? annotation.definer : undefined,
            });
            openObjectBrowserRef.current();
        }
        if (annotation.kind === "help" && authToken) {
            const request = ++helpRequest.current;
            void invokeVerbFlatBuffer(
                authToken,
                annotation.provider,
                "help_topic",
                MoorVar.buildInvokeArgs([annotation.topic]),
            ).then(({ result }) => {
                if (request !== helpRequest.current) return;
                const topic = result as { title?: string; content?: string[]; annotations?: unknown };
                if (!topic || typeof topic.title !== "string" || !Array.isArray(topic.content)) {
                    throw new Error("Help topic is unavailable.");
                }
                addPresentation({
                    id: "help-topic",
                    target: "help",
                    content_type: "text/djot",
                    content: topic.content.join("\n"),
                    annotations: decodeAnnotations(topic.annotations),
                    attributes: [["title", topic.title]],
                });
            }).catch(error => {
                if (request === helpRequest.current) {
                    showMessage(error instanceof Error ? error.message : "Help topic is unavailable.", 4);
                }
            });
        }
    }, [inspectObject, authToken, addPresentation, showMessage, canUseObjectBrowser, argumentCoordinator]);

    // Split layout
    const { splitRatio, handleSplitMouseDown, handleSplitTouchStart } = useSplitDrag();

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
            setIsObjectBrowserDocked(true);
            setIsEvalPanelOpen(false);
        }
        setIsObjectBrowserOpen(true);
    }, [isTouchDevice]);

    openObjectBrowserRef.current = handleOpenObjectBrowser;

    const handleCloseObjectBrowser = useCallback(() => {
        dismissAllObjectBrowserPresentations();
        setIsObjectBrowserOpen(false);
    }, [dismissAllObjectBrowserPresentations]);

    closeObjectBrowserRef.current = handleCloseObjectBrowser;

    const handleOpenEvalPanel = useCallback(() => {
        if (isTouchDevice) {
            setIsEvalPanelDocked(true);
            setIsObjectBrowserOpen(false);
        }
        setIsEvalPanelOpen(true);
    }, [isTouchDevice]);

    // A newly opened editor takes the touch dock. Tool navigation itself leaves
    // existing editors mounted so their unsaved content survives the switch.
    useEffect(() => {
        if (!isTouchDevice) return;
        setIsObjectBrowserOpen(false);
        setIsEvalPanelOpen(false);
    }, [
        isTouchDevice,
        editorSession?.id,
        editorSession?.presentationId,
        propertyEditorSession?.id,
        propertyEditorSession?.presentationId,
        propertyValueEditorSession?.id,
        propertyValueEditorSession?.presentationId,
        textEditorSession?.id,
    ]);

    const editorsCovered = isTouchDevice && (isObjectBrowserOpen || isEvalPanelOpen);

    // Handle closing presentations
    const handleClosePresentation = useCallback((id: string) => {
        if (authToken) {
            dismissPresentation(id, authToken);
        }
    }, [authToken, dismissPresentation]);

    // Player-switch orchestration across owners
    const switchHandlers = useMemo(() => ({
        onAuthorityReset: () => {
            resetForPlayerSwitch();
            resetLatching();
            // Programmer surfaces are authority-scoped: a manually opened
            // browser or eval panel must not survive an identity change with
            // data fetched under the previous player's token
            setIsObjectBrowserOpen(false);
            setIsEvalPanelOpen(false);
            setChangeReview(null);
            closeInspectPopover();
        },
        onSessionEnded: () => {
            disconnectWS();
            narrativeRef.current?.clearAll();
            clearOAuth2UserInfo();
        },
        onHistoryIdentityChanged: () => {
            markHistoryForReload();
            resetForIdentityChange();
        },
    }), [
        clearOAuth2UserInfo,
        closeInspectPopover,
        disconnectWS,
        markHistoryForReload,
        narrativeRef,
        resetForIdentityChange,
        resetForPlayerSwitch,
        resetLatching,
    ]);

    usePlayerSwitch(playerOid, historyPlayerOid, switchHandlers);

    return (
        <ArgumentContext.Provider value={argumentCoordinator}>
            <AnnotationContext.Provider value={handleAnnotationActivation}>
                <div className="app-root">
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
                        eventLogEnabled={eventLogEnabled}
                        onConnect={handleConnect}
                        oauth2UserInfo={oauth2UserInfo}
                        onOAuth2AccountChoice={handleOAuth2AccountChoice}
                        onOAuth2Cancel={clearOAuth2UserInfo}
                    />

                    {/* Top navigation bar - only show when connected */}
                    {isConnected && (
                        <>
                            <TopNavBar
                                onSettingsToggle={() => setIsSettingsOpen(true)}
                                onAccountToggle={() => setIsAccountMenuOpen(true)}
                                onBrowserToggle={hasProgrammerAccess ? handleOpenObjectBrowser : undefined}
                                onEvalToggle={hasProgrammerAccess ? handleOpenEvalPanel : undefined}
                                onChangesToggle={changeService ? () => setIsChangesOpen(true) : undefined}
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
                        roomHudEnabled={roomHudEnabled}
                        onToggleRoomHud={() => setRoomHudEnabled((prev) => !prev)}
                    />

                    {/* Account menu */}
                    <AccountMenu
                        isOpen={isAccountMenuOpen}
                        onClose={() => setIsAccountMenuOpen(false)}
                        onLogout={handleLogout}
                        historyAvailable={eventLogEnabled !== false}
                        authToken={authToken}
                        playerOid={playerOid}
                        refreshKey={profileRefreshKey}
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
                                        presentations={topDockPresentations}
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
                                            connectionError={wsState.connectionError}
                                            onSendMessage={sendMessage}
                                            onLoadMoreHistory={eventLogEnabled === false || !hasMoreHistory
                                                ? undefined
                                                : handleLoadMoreHistory}
                                            isLoadingHistory={eventLogEnabled === false ? false : isLoadingHistory}
                                            onLinkClick={handleLinkClick}
                                            playerOid={playerOid}
                                            onMessageAppended={handleMessageAppended}
                                            currentRoomLookKey={currentRoomLookKey}
                                            onActiveRoomLookVisibilityChange={handleActiveRoomLookVisibilityChange}
                                            fontSize={narrativeFontSize}
                                            inputMetadata={inputMetadata}
                                            onClearInputMetadata={clearInputMetadata}
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
                                    <div style={{ display: editorsCovered ? "none" : "contents" }}>
                                        {verbEditorDocked && editorSession && (
                                            <VerbEditor
                                                visible={!editorsCovered}
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
                                    </div>
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
                                            memberNavigation={memberNavigation}
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
                                    const presentation = verbEditorPresentations.find(p =>
                                        p.id === session.presentationId
                                    );
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
                            onUnlock={handleUnlock}
                            onForgotPassword={handleForgotPassword}
                            onSkip={skipUnlock}
                        />
                    )}

                    {eventLogEnabled !== false && showEncryptionSetup && (
                        <EncryptionSetupPrompt
                            systemTitle={systemTitle}
                            onSetup={handleSetup}
                            onSkip={skipSetup}
                        />
                    )}

                    {/* Profile Setup Panel */}
                    {profileSetupPresentation && authToken && player?.oid && (
                        <ProfileSetupPanel
                            presentation={profileSetupPresentation}
                            authToken={authToken}
                            playerOid={player.oid}
                            onComplete={handleProfileSetupComplete}
                            onSkip={handleProfileSetupSkip}
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
                            memberNavigation={memberNavigation}
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

                    {memberReference && (
                        <DialogSheet
                            title="Member reference"
                            titleId="member-reference-title"
                            onCancel={() => setMemberReference(null)}
                        >
                            <div className="dialog-sheet-content">
                                <p>Programmer access is required to browse this member.</p>
                                <code>{memberReference}</code>
                                <p role="status">{memberCopyNotice}</p>
                            </div>
                            <div className="dialog-sheet-footer">
                                <button type="button" onClick={() => setMemberReference(null)}>Close</button>
                                <button
                                    type="button"
                                    onClick={() =>
                                        void navigator.clipboard.writeText(memberReference).then(() =>
                                            setMemberCopyNotice("Reference copied")
                                        ).catch(() => setMemberCopyNotice("Could not copy the reference"))}
                                >
                                    Copy reference
                                </button>
                            </div>
                        </DialogSheet>
                    )}

                    {isChangesOpen && changeService && authToken && (
                        <ChangeWorkspace
                            visible
                            key={authToken}
                            service={changeService}
                            authToken={authToken}
                            isWizard={Boolean((playerFlags ?? 0) & OBJFLAG_WIZARD)}
                            connectionRevision={changeConnectionRevision}
                            onClose={() => setIsChangesOpen(false)}
                        />
                    )}

                    {changeReview && authToken && (
                        <ChangeReview
                            visible
                            key={`${authToken}:${changeReview.provider}:${changeReview.review}:${changeReview.generation}:${
                                changeReview.row ?? ""
                            }`}
                            target={changeReview}
                            connectionRevision={changeConnectionRevision}
                            authToken={authToken}
                            onClose={() => setChangeReview(null)}
                        />
                    )}

                    {annotationCommand && (
                        <AnnotationCommandReview
                            key={JSON.stringify(annotationCommand)}
                            annotation={annotationCommand.annotation}
                            position={annotationCommand.position}
                            authToken={authToken}
                            revision={stateRevision}
                            onSubmit={submitInspectionCommand}
                            onClose={() => setAnnotationCommand(null)}
                        />
                    )}

                    {/* Inspect popover for object info */}
                    {inspect.inspectPopover && (
                        <InspectPopover
                            key={inspect.inspectPopover.requestId}
                            returnFocusTo={inspect.inspectPopover.returnFocusTo}
                            data={inspect.inspectPopover.data}
                            reference={inspect.inspectPopover.oref}
                            position={inspect.inspectPopover.position}
                            onClose={inspect.closeInspectPopover}
                            onCommand={submitInspectionCommand}
                            authToken={authToken}
                            revision={stateRevision}
                            isPreview={inspect.inspectPopover.isPreview}
                        />
                    )}
                </div>
            </AnnotationContext.Provider>
        </ArgumentContext.Provider>
    );
};
