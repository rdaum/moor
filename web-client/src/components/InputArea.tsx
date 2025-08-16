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

import React, { useCallback, useEffect, useRef, useState } from "react";

interface InputAreaProps {
    visible: boolean;
    disabled: boolean;
    onSendMessage: (message: string) => void;
    commandHistory: string[];
    onAddToHistory: (command: string) => void;
}

const ENCOURAGING_PLACEHOLDERS = [
    "What would you like to explore?",
    "Ready for your next adventure?",
    "What's on your mind?",
    "How can we help you today?",
    "What would you like to try?",
    "Share your thoughts...",
    "What's your next move?",
    "Ready to discover something new?",
];

export const InputArea: React.FC<InputAreaProps> = ({
    visible,
    disabled,
    onSendMessage,
    commandHistory,
    onAddToHistory,
}) => {
    const [input, setInput] = useState("");
    const [historyOffset, setHistoryOffset] = useState(0);
    const [placeholderIndex, setPlaceholderIndex] = useState(() =>
        Math.floor(Math.random() * ENCOURAGING_PLACEHOLDERS.length)
    );
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    // Focus input area when it becomes visible and enabled
    useEffect(() => {
        if (visible && !disabled && textareaRef.current) {
            setTimeout(() => {
                textareaRef.current?.focus();
            }, 100);
        }
    }, [visible, disabled]);

    // Auto-resize textarea based on content
    useEffect(() => {
        const textarea = textareaRef.current;
        if (!textarea) return;

        // Reset height to auto to get the scroll height
        textarea.style.height = "auto";

        // Set height to scroll height, but constrain within min/max bounds
        const scrollHeight = textarea.scrollHeight;
        const minHeight = 48; // 3rem in pixels (approximate)
        const maxHeight = 128; // 8rem in pixels (approximate)

        const newHeight = Math.min(Math.max(scrollHeight, minHeight), maxHeight);
        textarea.style.height = `${newHeight}px`;
    }, [input]);

    // Navigate through command history
    const navigateHistory = useCallback((direction: "up" | "down") => {
        const isMultiline = input.includes("\n");
        const textarea = textareaRef.current;
        if (!textarea) return;

        const cursorAtEdge = textarea.selectionStart === 0
            || (textarea.selectionStart === textarea.selectionEnd
                && textarea.selectionStart === textarea.value.length);

        // Skip history navigation if in multiline mode with cursor in middle
        if (isMultiline && !cursorAtEdge) {
            return; // Let default behavior handle cursor movement
        }

        let newOffset = historyOffset;

        if (direction === "up" && historyOffset < commandHistory.length) {
            newOffset = historyOffset + 1;
        } else if (direction === "down" && historyOffset > 0) {
            newOffset = historyOffset - 1;
        } else {
            return; // Cannot navigate further
        }

        setHistoryOffset(newOffset);

        // Calculate the history index
        const historyIndex = commandHistory.length - newOffset;

        // Set input value from history or clear if nothing available
        if (historyIndex >= 0 && historyIndex < commandHistory.length) {
            const historyValue = commandHistory[historyIndex];
            setInput(historyValue ? historyValue.trimEnd() : "");
        } else {
            setInput("");
        }
    }, [input, historyOffset, commandHistory]);

    // Send input to server
    const sendInput = useCallback(() => {
        const trimmedInput = input.trim();

        if (!trimmedInput) {
            return;
        }

        // Split by lines and send each non-empty line
        const lines = input.split("\n");
        for (const line of lines) {
            if (line.trim()) {
                onSendMessage(line.trim());
            }
        }

        // Add to command history and reset
        if (trimmedInput) {
            onAddToHistory(trimmedInput);
        }

        // Clear input and reset history offset
        setInput("");
        setHistoryOffset(0);

        // Pick a new encouraging placeholder for next input
        setPlaceholderIndex(Math.floor(Math.random() * ENCOURAGING_PLACEHOLDERS.length));
    }, [input, onSendMessage, onAddToHistory, disabled]);

    // Handle paste events
    const handlePaste = useCallback((e: React.ClipboardEvent) => {
        // Directly process the pasted content at cursor position
        e.stopPropagation();
        e.preventDefault();

        const pastedData = e.clipboardData?.getData("text") || "";
        if (!pastedData) return;

        const textarea = textareaRef.current;
        if (!textarea) return;

        // Insert the pasted data at the current cursor position
        const selStart = textarea.selectionStart || 0;
        const selEnd = textarea.selectionEnd || 0;

        const newValue = textarea.value.substring(0, selStart)
            + pastedData
            + textarea.value.substring(selEnd);

        // Update React state
        setInput(newValue);

        // Place cursor after the pasted content
        const newPosition = selStart + pastedData.length;

        // Use setTimeout to ensure the state update has been applied
        setTimeout(() => {
            if (textarea) {
                textarea.selectionStart = newPosition;
                textarea.selectionEnd = newPosition;
            }
        }, 0);
    }, []);

    // Handle key events
    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        if (e.key === "ArrowUp") {
            const isMultiline = input.includes("\n");
            const textarea = textareaRef.current;
            if (!textarea) return;

            const cursorAtEdge = textarea.selectionStart === 0
                || (textarea.selectionStart === textarea.selectionEnd
                    && textarea.selectionStart === textarea.value.length);

            // Only prevent default and navigate history if conditions are met
            if (!isMultiline || cursorAtEdge) {
                if (historyOffset < commandHistory.length) {
                    e.preventDefault();
                    navigateHistory("up");
                }
            }
            // Otherwise, let default arrow key behavior handle cursor movement
        } else if (e.key === "ArrowDown") {
            const isMultiline = input.includes("\n");
            const textarea = textareaRef.current;
            if (!textarea) return;

            const cursorAtEdge = textarea.selectionStart === 0
                || (textarea.selectionStart === textarea.selectionEnd
                    && textarea.selectionStart === textarea.value.length);

            // Only prevent default and navigate history if conditions are met
            if (!isMultiline || cursorAtEdge) {
                if (historyOffset > 0) {
                    e.preventDefault();
                    navigateHistory("down");
                }
            }
            // Otherwise, let default arrow key behavior handle cursor movement
        } else if (e.key === "Enter" && e.shiftKey) {
            // Shift+Enter for newlines - let default behavior handle it
            // React will update the state through onChange
        } else if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            sendInput();
        }
    }, [navigateHistory, sendInput, input, historyOffset, commandHistory]);

    if (!visible) {
        return null;
    }

    return (
        <div style={{ width: "100%" }}>
            <textarea
                ref={textareaRef}
                id="input_area"
                className="input_area"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                onPaste={handlePaste}
                disabled={disabled}
                placeholder={ENCOURAGING_PLACEHOLDERS[placeholderIndex]}
                autoComplete="off"
                spellCheck={false}
                aria-label="Command input"
                aria-describedby="input-help"
                aria-multiline="true"
                style={{
                    minHeight: "3rem",
                    height: "auto",
                    maxHeight: "8rem",
                    width: "100%",
                    boxSizing: "border-box",
                    resize: "none",
                }}
            />
            <div id="input-help" className="sr-only">
                Use Shift+Enter for new lines. Arrow keys navigate command history when at start or end of input.
            </div>
        </div>
    );
};
