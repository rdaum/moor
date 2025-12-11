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
import { getVerbPlaceholder, startsWithKnownVerb } from "../lib/known-verbs";
import { InputMetadata } from "../types/input";
import { RichInputPrompt } from "./RichInputPrompt";
import { getSayModeEnabled } from "./SayModeToggle";
import { VerbPalette } from "./VerbPalette";
import { getVerbPaletteEnabled } from "./VerbPaletteToggle";

interface InputAreaProps {
    visible: boolean;
    disabled: boolean;
    onSendMessage: (message: string | Uint8Array | ArrayBuffer) => void;
    commandHistory: string[];
    onAddToHistory: (command: string) => void;
    inputMetadata?: InputMetadata | null;
    onClearInputMetadata?: () => void;
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
    inputMetadata,
    onClearInputMetadata,
}) => {
    const [input, setInput] = useState("");
    const [historyOffset, setHistoryOffset] = useState(0);
    const [placeholderIndex, setPlaceholderIndex] = useState(() =>
        Math.floor(Math.random() * ENCOURAGING_PLACEHOLDERS.length)
    );
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    // Say mode and verb palette settings
    const [sayModeEnabled, setSayModeEnabled] = useState(getSayModeEnabled);
    const [verbPaletteEnabled, setVerbPaletteEnabled] = useState(getVerbPaletteEnabled);
    // Whether the say pill is active for current input (can be toggled off per-input)
    const [sayPillActive, setSayPillActive] = useState(true);
    // Verb pill from palette selection (overrides say pill when set)
    const [verbPill, setVerbPill] = useState<string | null>(null);
    // Accessibility: announcement for screen readers
    const [srAnnouncement, setSrAnnouncement] = useState<string>("");

    // Detect if user prefers reduced motion (common for screen reader users)
    const prefersReducedMotion = useRef(
        typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    );

    // Listen for settings changes
    useEffect(() => {
        const handleSayModeChange = (e: CustomEvent<boolean>) => {
            setSayModeEnabled(e.detail);
            setSayPillActive(e.detail);
        };
        const handleVerbPaletteChange = (e: CustomEvent<boolean>) => {
            setVerbPaletteEnabled(e.detail);
        };

        window.addEventListener("sayModeChanged", handleSayModeChange as EventListener);
        window.addEventListener("verbPaletteChanged", handleVerbPaletteChange as EventListener);

        return () => {
            window.removeEventListener("sayModeChanged", handleSayModeChange as EventListener);
            window.removeEventListener("verbPaletteChanged", handleVerbPaletteChange as EventListener);
        };
    }, []);

    // Focus input area when it becomes visible and enabled, or when returning from rich input prompt
    useEffect(() => {
        if (visible && !disabled && !inputMetadata?.input_type && textareaRef.current) {
            // Only focus if the textarea doesn't already have focus
            if (document.activeElement !== textareaRef.current) {
                textareaRef.current.focus();
            }
        }
    }, [visible, disabled, inputMetadata]);

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

    // Determine if we should apply say prefix to a line
    const shouldApplySayPrefix = useCallback((line: string): boolean => {
        // No prefix if there's a verb pill (it takes precedence)
        if (verbPill) return false;
        // No prefix if say mode is disabled or pill is inactive
        if (!sayModeEnabled || !sayPillActive) return false;
        // No prefix if line already starts with a known verb
        if (startsWithKnownVerb(line)) return false;
        return true;
    }, [sayModeEnabled, sayPillActive, verbPill]);

    // Send input to server
    const sendInput = useCallback(() => {
        const trimmedInput = input.trim();

        // Allow sending verb pill alone (e.g., "look" with no args)
        if (!trimmedInput && !verbPill) {
            return;
        }

        // Split by lines and send each non-empty line
        const lines = trimmedInput ? input.split("\n") : [""];
        for (const line of lines) {
            const trimmedLine = line.trim();
            // Build the command
            let messageToSend: string;
            if (verbPill) {
                // Verb pill takes precedence
                messageToSend = trimmedLine ? `${verbPill} ${trimmedLine}` : verbPill;
            } else if (shouldApplySayPrefix(trimmedLine)) {
                messageToSend = `say ${trimmedLine}`;
            } else {
                messageToSend = trimmedLine;
            }
            if (messageToSend) {
                onSendMessage(messageToSend);
            }
        }

        // Add original input to command history (not the transformed version)
        if (trimmedInput) {
            onAddToHistory(trimmedInput);
        }

        // Clear input and reset state
        setInput("");
        setHistoryOffset(0);
        setVerbPill(null);
        // Reset say pill to active for next input
        setSayPillActive(sayModeEnabled);

        // Pick a new encouraging placeholder for next input (skip if user prefers reduced motion)
        if (!prefersReducedMotion.current) {
            setPlaceholderIndex(Math.floor(Math.random() * ENCOURAGING_PLACEHOLDERS.length));
        }
    }, [input, onSendMessage, onAddToHistory, shouldApplySayPrefix, sayModeEnabled, verbPill]);

    // Announce to screen readers
    const announce = useCallback((message: string) => {
        setSrAnnouncement(message);
        // Clear after a delay so the same message can be announced again
        setTimeout(() => setSrAnnouncement(""), 1000);
    }, []);

    // Handle verb selection from palette
    const handleVerbSelect = useCallback((verb: string) => {
        // Set verb pill and disable say mode for this input
        setVerbPill(verb);
        setSayPillActive(false);
        announce(`${verb} command selected`);
        // Focus the textarea
        textareaRef.current?.focus();
    }, [announce]);

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
        // Handle backspace to remove verb pill or say pill when input is empty
        if (e.key === "Backspace" && input === "") {
            if (verbPill) {
                e.preventDefault();
                setVerbPill(null);
                // Restore say pill if say mode is enabled
                if (sayModeEnabled) {
                    setSayPillActive(true);
                    announce("say mode restored");
                } else {
                    announce("command mode");
                }
                return;
            }
            if (sayPillActive && sayModeEnabled) {
                e.preventDefault();
                setSayPillActive(false);
                announce("command mode");
                return;
            }
        }

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
    }, [
        navigateHistory,
        sendInput,
        input,
        historyOffset,
        commandHistory,
        sayPillActive,
        sayModeEnabled,
        verbPill,
        announce,
    ]);

    // Handler for rich input submission
    const handleRichInputSubmit = useCallback((value: string | Uint8Array) => {
        onSendMessage(value);
        if (onClearInputMetadata) {
            onClearInputMetadata();
        }
    }, [onSendMessage, onClearInputMetadata]);

    if (!visible) {
        return null;
    }

    // If we have input metadata, render the rich input prompt instead
    if (inputMetadata && inputMetadata.input_type) {
        return (
            <div className="w-full">
                <RichInputPrompt
                    metadata={inputMetadata}
                    onSubmit={handleRichInputSubmit}
                    disabled={disabled}
                />
            </div>
        );
    }

    // Determine which pill to show (verb pill takes precedence over say pill)
    const showSayPill = !verbPill && sayModeEnabled && sayPillActive;
    const activePill = verbPill || (showSayPill ? "say" : null);

    // Get context-sensitive placeholder
    const getPlaceholder = (): string => {
        if (activePill === "say") {
            return "What would you like to say?";
        }
        if (verbPill) {
            const verbPlaceholder = getVerbPlaceholder(verbPill);
            if (verbPlaceholder) return verbPlaceholder;
        }
        return ENCOURAGING_PLACEHOLDERS[placeholderIndex];
    };

    // Default text input
    return (
        <div className="input_area_container">
            {/* Verb palette above input */}
            <VerbPalette
                visible={verbPaletteEnabled}
                onVerbSelect={handleVerbSelect}
            />

            {/* Input area with pill inside */}
            <div className="input_area_box">
                {activePill && (
                    <button
                        type="button"
                        className="say-mode-pill"
                        onClick={() => {
                            if (verbPill) {
                                setVerbPill(null);
                                if (sayModeEnabled) {
                                    setSayPillActive(true);
                                    announce("say mode restored");
                                } else {
                                    announce("command mode");
                                }
                            } else {
                                setSayPillActive(false);
                                announce("command mode");
                            }
                        }}
                        title="Click or press Backspace to remove"
                        aria-label={`${activePill} command active. Click to remove.`}
                    >
                        {activePill}
                    </button>
                )}
                <textarea
                    ref={textareaRef}
                    id="input_area"
                    className="input_area_inner"
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={handleKeyDown}
                    onPaste={handlePaste}
                    disabled={disabled}
                    autoComplete="off"
                    spellCheck={false}
                    aria-label={activePill ? `${activePill} command` : "Command input"}
                    aria-describedby="input-help"
                    aria-multiline="true"
                    placeholder={getPlaceholder()}
                />
            </div>

            <div id="input-help" className="sr-only">
                {activePill
                    ? `${activePill} command active. Press Backspace to remove. Use Shift+Enter for new lines.`
                    : "Use Shift+Enter for new lines. Arrow keys navigate command history when at start or end of input."}
            </div>

            {/* Live region for screen reader announcements */}
            <div
                role="status"
                aria-live="polite"
                aria-atomic="true"
                className="sr-only"
            >
                {srAnnouncement}
            </div>
        </div>
    );
};
