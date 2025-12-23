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

import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import { InputMetadata } from "../types/input";
import { getCommandEchoEnabled } from "./CommandEchoToggle";
import { InputArea } from "./InputArea";
import { LinkPreview } from "./LinkPreviewCard";
import { OutputWindow } from "./OutputWindow";

export interface EventMetadata {
    verb?: string;
    actor?: any;
    actorName?: string;
    content?: string;
    thisObj?: any;
    thisName?: string;
    dobj?: any;
    dobjName?: string;
    iobj?: any;
    timestamp?: number;
}

export interface NarrativeMessage {
    id: string;
    content: string | string[];
    type: "narrative" | "input_echo" | "system" | "error";
    timestamp?: number;
    isHistorical?: boolean;
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback";
    noNewline?: boolean;
    presentationHint?: string;
    groupId?: string;
    ttsText?: string;
    thumbnail?: { contentType: string; data: string };
    linkPreview?: LinkPreview;
    eventMetadata?: EventMetadata;
}

interface NarrativeProps {
    visible: boolean;
    connectionStatus: "disconnected" | "connecting" | "connected" | "error";
    onSendMessage: (message: string | Uint8Array | ArrayBuffer) => void;
    onLoadMoreHistory?: () => void;
    isLoadingHistory?: boolean;
    onLinkClick?: (url: string, position?: { x: number; y: number }) => void;
    onLinkHoldStart?: (url: string, position: { x: number; y: number }) => void;
    onLinkHoldEnd?: () => void;
    playerOid?: string | null;
    onMessageAppended?: (message: NarrativeMessage) => void;
    fontSize?: number;
    inputMetadata?: InputMetadata | null;
    onClearInputMetadata?: () => void;
    shouldShowDisconnectDivider?: boolean;
}

export interface NarrativeRef {
    addNarrativeContent: (
        content: string | string[],
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
        noNewline?: boolean,
        presentationHint?: string,
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
        linkPreview?: LinkPreview,
        eventMetadata?: EventMetadata,
    ) => void;
    addSystemMessage: (content: string | string[]) => void;
    addErrorMessage: (content: string | string[]) => void;
    addHistoricalMessages: (messages: NarrativeMessage[]) => void;
    prependHistoricalMessages: (messages: NarrativeMessage[]) => void;
    getContainerHeight: () => number;
    clearAll: () => void;
    getLastMessageTimestamp: () => number;
}

const COMMAND_HISTORY_STORAGE_PREFIX = "moor-command-history";
const MAX_COMMAND_HISTORY = 500;

const getCommandHistoryStorageKey = (playerOid?: string | null) => {
    if (!playerOid) {
        return null;
    }
    return `${COMMAND_HISTORY_STORAGE_PREFIX}:${playerOid}`;
};

export const Narrative = forwardRef<NarrativeRef, NarrativeProps>(({
    visible,
    connectionStatus,
    onSendMessage,
    onLoadMoreHistory,
    isLoadingHistory = false,
    onLinkClick,
    onLinkHoldStart,
    onLinkHoldEnd,
    playerOid,
    onMessageAppended,
    fontSize,
    inputMetadata,
    onClearInputMetadata,
    shouldShowDisconnectDivider = false,
}, ref) => {
    const connected = connectionStatus === "connected";
    const [messages, setMessages] = useState<NarrativeMessage[]>([]);
    const [commandHistory, setCommandHistory] = useState<string[]>([]);
    const narrativeContainerRef = useRef<HTMLDivElement>(null);
    const storageKeyRef = useRef<string | null>(null);
    const previousStorageKeyRef = useRef<string | null>(null);
    const lastMessageTimestampRef = useRef<number>(0);
    const lastDisconnectMessageTimestampRef = useRef<number>(0);
    const currentStorageKey = getCommandHistoryStorageKey(playerOid);

    if (storageKeyRef.current !== currentStorageKey) {
        previousStorageKeyRef.current = storageKeyRef.current;
        storageKeyRef.current = currentStorageKey;
    }

    const clearStoredHistory = useCallback(() => {
        if (typeof window === "undefined") {
            return;
        }

        try {
            const storage = window.localStorage;
            const keysToRemove: string[] = [];
            for (let i = 0; i < storage.length; i += 1) {
                const key = storage.key(i);
                if (key && key.startsWith(COMMAND_HISTORY_STORAGE_PREFIX)) {
                    keysToRemove.push(key);
                }
            }
            keysToRemove.forEach(key => storage.removeItem(key));
        } catch (error) {
            console.warn("Failed to clear stored command history:", error);
        }
    }, []);

    // Add a new message to the narrative
    const addMessage = useCallback((
        content: string | string[],
        type: NarrativeMessage["type"] = "narrative",
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
        noNewline?: boolean,
        presentationHint?: string,
        groupId?: string,
        ttsText?: string,
        thumbnail?: { contentType: string; data: string },
        linkPreview?: LinkPreview,
        eventMetadata?: EventMetadata,
    ) => {
        const now = Date.now();
        const newMessage: NarrativeMessage = {
            id: `msg_${now}_${Math.random()}`,
            content,
            type,
            timestamp: now,
            contentType,
            noNewline,
            presentationHint,
            groupId,
            ttsText,
            thumbnail,
            linkPreview,
            eventMetadata,
        };

        // Track the latest message timestamp (update even for input echo)
        lastMessageTimestampRef.current = now;

        setMessages(prev => [...prev, newMessage]);
        if (type !== "input_echo") {
            onMessageAppended?.(newMessage);
        }
    }, [onMessageAppended]);

    // Handle sending messages
    const handleSendMessage = useCallback((message: string | Uint8Array | ArrayBuffer) => {
        // Echo the input to the narrative if setting is enabled (only for text)
        if (typeof message === "string" && getCommandEchoEnabled()) {
            addMessage(message, "input_echo");
        }

        // Send to server
        onSendMessage(message);
    }, [addMessage, onSendMessage]);

    // Add to command history
    const addToHistory = useCallback((command: string) => {
        setCommandHistory(prev => {
            const nextHistory = [...prev, command];
            const cappedHistory = nextHistory.length > MAX_COMMAND_HISTORY
                ? nextHistory.slice(-MAX_COMMAND_HISTORY)
                : nextHistory;

            if (typeof window !== "undefined") {
                const storageKey = storageKeyRef.current;
                if (storageKey) {
                    try {
                        window.localStorage.setItem(storageKey, JSON.stringify(cappedHistory));
                    } catch (error) {
                        console.warn("Failed to persist command history:", error);
                    }
                }
            }

            return cappedHistory;
        });
    }, []);

    // Add a method to add narrative content from WebSocket messages
    const addNarrativeContent = useCallback(
        (
            content: string | string[],
            contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
            noNewline?: boolean,
            presentationHint?: string,
            groupId?: string,
            ttsText?: string,
            thumbnail?: { contentType: string; data: string },
            linkPreview?: LinkPreview,
            eventMetadata?: EventMetadata,
        ) => {
            addMessage(
                content,
                "narrative",
                contentType,
                noNewline,
                presentationHint,
                groupId,
                ttsText,
                thumbnail,
                linkPreview,
                eventMetadata,
            );
        },
        [addMessage],
    );

    // Add system message
    const addSystemMessage = useCallback((content: string | string[]) => {
        addMessage(content, "system");
    }, [addMessage]);

    // Add error message
    const addErrorMessage = useCallback((content: string | string[]) => {
        addMessage(content, "error");
    }, [addMessage]);

    // Add historical messages (prepend to existing messages) - for initial load
    const addHistoricalMessages = useCallback((historicalMessages: NarrativeMessage[]) => {
        setMessages(prev => {
            // Preserve any live messages that arrived after history boundary
            const liveMessages = prev.filter(msg => !msg.isHistorical);
            const newMessages = [...historicalMessages, ...liveMessages];
            return newMessages;
        });
    }, []);

    // Prepend more historical messages (for infinite scroll)
    const prependHistoricalMessages = useCallback((moreHistoricalMessages: NarrativeMessage[]) => {
        setMessages(prev => {
            // Prepend to the beginning of existing messages
            const newMessages = [...moreHistoricalMessages, ...prev];
            return newMessages;
        });
    }, []);

    // Get container height for dynamic sizing
    const getContainerHeight = useCallback(() => {
        return narrativeContainerRef.current?.clientHeight || window.innerHeight * 0.7;
    }, []);

    // Clear all messages and command history (used on logout)
    const clearAll = useCallback(() => {
        if (typeof window !== "undefined") {
            clearStoredHistory();
            storageKeyRef.current = null;
            previousStorageKeyRef.current = null;
        }

        setMessages([]);
        setCommandHistory([]);
    }, [clearStoredHistory]);

    // Expose methods to parent component
    useImperativeHandle(ref, () => ({
        addNarrativeContent,
        addSystemMessage,
        addErrorMessage,
        addHistoricalMessages,
        prependHistoricalMessages,
        getContainerHeight,
        clearAll,
        getLastMessageTimestamp: () => lastDisconnectMessageTimestampRef.current,
    }), [
        addNarrativeContent,
        addSystemMessage,
        addErrorMessage,
        addHistoricalMessages,
        prependHistoricalMessages,
        getContainerHeight,
        clearAll,
    ]);

    // Track the current storage key for this player and load stored history when it changes
    useEffect(() => {
        if (typeof window === "undefined") {
            setCommandHistory([]);
            return;
        }

        const storageKey = storageKeyRef.current;

        if (!storageKey) {
            setCommandHistory([]);
            return;
        }

        try {
            const raw = window.localStorage.getItem(storageKey);
            if (!raw) {
                setCommandHistory([]);
                return;
            }
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed)) {
                const normalized = parsed.map(item => (typeof item === "string" ? item : String(item)));
                if (normalized.length > MAX_COMMAND_HISTORY) {
                    const capped = normalized.slice(-MAX_COMMAND_HISTORY);
                    window.localStorage.setItem(storageKey, JSON.stringify(capped));
                    setCommandHistory(capped);
                } else {
                    setCommandHistory(normalized);
                }
            } else {
                window.localStorage.removeItem(storageKey);
                setCommandHistory([]);
            }
        } catch (error) {
            console.warn("Failed to read command history from storage:", error);
            window.localStorage.removeItem(storageKey);
            setCommandHistory([]);
        }
    }, [currentStorageKey]);

    // Track disconnect time: when we lose connection, save the last message timestamp
    useEffect(() => {
        if (connectionStatus === "disconnected" || connectionStatus === "error") {
            lastDisconnectMessageTimestampRef.current = lastMessageTimestampRef.current;
        }
    }, [connectionStatus]);

    if (!visible) {
        return null;
    }

    const promptActive = Boolean(inputMetadata?.input_type);

    return (
        <div
            ref={narrativeContainerRef}
            className="narrative"
            id="narrative"
        >
            {/* History viewing indicator - TODO: implement */}
            {/* <HistoryIndicator /> */}

            {/* Output display area - should grow to fill space and handle its own scrolling */}
            <div className={`narrative-output-wrapper${promptActive ? " narrative-output-wrapper--dimmed" : ""}`}>
                <OutputWindow
                    messages={messages}
                    onLoadMoreHistory={onLoadMoreHistory}
                    isLoadingHistory={isLoadingHistory}
                    onLinkClick={onLinkClick}
                    onLinkHoldStart={onLinkHoldStart}
                    onLinkHoldEnd={onLinkHoldEnd}
                    fontSize={fontSize}
                    shouldShowDisconnectDivider={shouldShowDisconnectDivider}
                    playerOid={playerOid}
                />
                {promptActive && (
                    <>
                        <div className="narrative_output_scrim" aria-hidden="true" />
                        <span className="sr-only" role="status">
                            A prompt response is required before continuing normal interaction.
                        </span>
                    </>
                )}
            </div>

            {/* Command input area - fixed at bottom */}
            <div className="narrative-input-container">
                <InputArea
                    visible={connected}
                    disabled={!connected}
                    onSendMessage={handleSendMessage}
                    commandHistory={commandHistory}
                    onAddToHistory={addToHistory}
                    inputMetadata={inputMetadata}
                    onClearInputMetadata={onClearInputMetadata}
                />
            </div>

            {/* Connection status overlay */}
            {(connectionStatus === "connecting" || connectionStatus === "error") && (
                <div
                    style={{
                        position: "absolute",
                        inset: 0,
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        backgroundColor: "rgba(0, 0, 0, 0.3)",
                        backdropFilter: "blur(4px)",
                        borderRadius: "var(--radius-lg)",
                        zIndex: 100,
                    }}
                >
                    <div
                        role="status"
                        aria-live="polite"
                        aria-atomic="true"
                        style={{
                            display: "flex",
                            flexDirection: "column",
                            alignItems: "center",
                            gap: "16px",
                            padding: "32px",
                            backgroundColor: "var(--color-bg-primary)",
                            borderRadius: "var(--radius-lg)",
                            boxShadow: "0 8px 32px rgba(0, 0, 0, 0.3)",
                        }}
                    >
                        {connectionStatus === "connecting"
                            ? (
                                <>
                                    <div className="loading-spinner large" aria-hidden="true" />
                                    <div
                                        style={{
                                            color: "var(--color-text-primary)",
                                            fontSize: "16px",
                                            fontWeight: 600,
                                            fontFamily: "var(--font-ui)",
                                        }}
                                    >
                                        Connecting to server...
                                    </div>
                                </>
                            )
                            : (
                                <>
                                    <div
                                        aria-hidden="true"
                                        style={{
                                            color: "var(--color-text-error)",
                                            fontSize: "24px",
                                            fontWeight: 600,
                                        }}
                                    >
                                        ⚠
                                    </div>
                                    <div
                                        style={{
                                            color: "var(--color-text-error)",
                                            fontSize: "16px",
                                            fontWeight: 600,
                                            fontFamily: "var(--font-ui)",
                                        }}
                                    >
                                        Connection error
                                    </div>
                                    <div
                                        style={{
                                            color: "var(--color-text-secondary)",
                                            fontSize: "14px",
                                            fontFamily: "var(--font-ui)",
                                            textAlign: "center",
                                        }}
                                    >
                                        Retrying connection...
                                    </div>
                                </>
                            )}
                    </div>
                </div>
            )}
        </div>
    );
});
