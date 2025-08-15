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

import { forwardRef, useCallback, useImperativeHandle, useRef, useState } from "react";
import { InputArea } from "./InputArea";
import { OutputWindow } from "./OutputWindow";

export interface NarrativeMessage {
    id: string;
    content: string | string[];
    type: "narrative" | "input_echo" | "system" | "error";
    timestamp?: number;
    isHistorical?: boolean;
    contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback";
}

interface NarrativeProps {
    visible: boolean;
    connected: boolean;
    onSendMessage: (message: string) => void;
    onLoadMoreHistory?: () => void;
    isLoadingHistory?: boolean;
    onLinkClick?: (url: string) => void;
}

export interface NarrativeRef {
    addNarrativeContent: (content: string | string[], contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback") => void;
    addSystemMessage: (content: string | string[]) => void;
    addErrorMessage: (content: string | string[]) => void;
    addHistoricalMessages: (messages: NarrativeMessage[]) => void;
    prependHistoricalMessages: (messages: NarrativeMessage[]) => void;
    getContainerHeight: () => number;
}

export const Narrative = forwardRef<NarrativeRef, NarrativeProps>(({
    visible,
    connected,
    onSendMessage,
    onLoadMoreHistory,
    isLoadingHistory = false,
    onLinkClick,
}, ref) => {
    const [messages, setMessages] = useState<NarrativeMessage[]>([]);
    const [commandHistory, setCommandHistory] = useState<string[]>([]);
    const narrativeContainerRef = useRef<HTMLDivElement>(null);

    // Add a new message to the narrative
    const addMessage = useCallback((
        content: string | string[],
        type: NarrativeMessage["type"] = "narrative",
        contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback",
    ) => {
        const newMessage: NarrativeMessage = {
            id: `msg_${Date.now()}_${Math.random()}`,
            content,
            type,
            timestamp: Date.now(),
            contentType,
        };

        setMessages(prev => [...prev, newMessage]);
    }, []);

    // Handle sending messages
    const handleSendMessage = useCallback((message: string) => {
        // Echo the input to the narrative
        addMessage(`> ${message}`, "input_echo");

        // Send to server
        onSendMessage(message);
    }, [addMessage, onSendMessage]);

    // Add to command history
    const addToHistory = useCallback((command: string) => {
        setCommandHistory(prev => [...prev, command]);
    }, []);

    // Add a method to add narrative content from WebSocket messages
    const addNarrativeContent = useCallback(
        (content: string | string[], contentType?: "text/plain" | "text/djot" | "text/html" | "text/traceback") => {
            addMessage(content, "narrative", contentType);
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

    // Expose methods to parent component
    useImperativeHandle(ref, () => ({
        addNarrativeContent,
        addSystemMessage,
        addErrorMessage,
        addHistoricalMessages,
        prependHistoricalMessages,
        getContainerHeight,
    }), [
        addNarrativeContent,
        addSystemMessage,
        addErrorMessage,
        addHistoricalMessages,
        prependHistoricalMessages,
        getContainerHeight,
    ]);

    if (!visible) {
        return null;
    }

    return (
        <div
            ref={narrativeContainerRef}
            className="narrative"
            id="narrative"
            aria-label="Narrative and input"
            style={{
                display: "flex",
                flexDirection: "column",
                height: "100%",
                overflow: "hidden",
            }}
        >
            {/* History viewing indicator - TODO: implement */}
            {/* <HistoryIndicator /> */}

            {/* Output display area - should grow to fill space and handle its own scrolling */}
            <OutputWindow
                messages={messages}
                onLoadMoreHistory={onLoadMoreHistory}
                isLoadingHistory={isLoadingHistory}
                onLinkClick={onLinkClick}
            />

            {/* Command input area - fixed at bottom */}
            <div
                style={{
                    flexShrink: 0,
                }}
            >
                <InputArea
                    visible={connected}
                    disabled={!connected}
                    onSendMessage={handleSendMessage}
                    commandHistory={commandHistory}
                    onAddToHistory={addToHistory}
                />
            </div>
        </div>
    );
});
