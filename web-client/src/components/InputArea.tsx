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

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAuthContext } from "../context/AuthContext";
import { useVerbSuggestions } from "../hooks/useVerbSuggestions";
import {
    extractFullVerbName,
    findCommonPrefix,
    getCompletionSuffix,
    getVerbPlaceholder,
    KNOWN_VERBS,
    parseVerbNames,
} from "../lib/known-verbs";
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
    // Working buffer for history edits - preserves changes when navigating away
    // Key: historyOffset (0 = new input, 1+ = history entries)
    const [historyBuffer, setHistoryBuffer] = useState<Record<number, string>>({});
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
    // Placeholder text for the current verb pill (from server hint)
    const [verbPillPlaceholder, setVerbPillPlaceholder] = useState<string | null>(null);
    // Accessibility: announcement for screen readers
    const [srAnnouncement, setSrAnnouncement] = useState<string>("");

    // Completion state
    const [completionIndex, setCompletionIndex] = useState(0);

    // Get auth context for verb suggestions
    const { authState } = useAuthContext();
    const authToken = authState.player?.authToken ?? null;
    const playerOid = authState.player?.oid ?? null;

    // Fetch verb suggestions from server
    const { suggestions: serverSuggestions, available: serverAvailable } = useVerbSuggestions(authToken, playerOid);

    // Compute matching verbs for completion using proper verbcasecmp matching
    const completionMatches = useMemo(() => {
        // Only complete when: palette enabled, no verb pill, input has content, no spaces (first word only)
        if (!verbPaletteEnabled || verbPill || !input || input.includes(" ") || input.includes("\n")) {
            return [];
        }

        const prefix = input;
        const matches: Array<{ verb: string; hint: string | null; suffix: string }> = [];

        // Use server suggestions if available, otherwise fall back to KNOWN_VERBS
        if (serverAvailable && serverSuggestions.length > 0) {
            for (const suggestion of serverSuggestions) {
                // Parse space-separated verb names (aliases)
                const names = parseVerbNames(suggestion.verb);
                for (const pattern of names) {
                    const suffix = getCompletionSuffix(pattern, prefix);
                    if (suffix !== null && suffix !== "") {
                        // Found a match - use the full verb name
                        const fullVerb = extractFullVerbName(pattern);
                        // Avoid duplicates
                        if (!matches.some(m => m.verb.toLowerCase() === fullVerb.toLowerCase())) {
                            matches.push({
                                verb: fullVerb,
                                hint: suggestion.hint,
                                suffix,
                            });
                        }
                        break; // Only need one match per suggestion
                    }
                }
            }
        } else {
            // Fall back to KNOWN_VERBS (these are simple strings, no patterns)
            for (const verb of KNOWN_VERBS) {
                if (
                    verb.toLowerCase().startsWith(prefix.toLowerCase()) && verb.toLowerCase() !== prefix.toLowerCase()
                ) {
                    matches.push({
                        verb,
                        hint: getVerbPlaceholder(verb),
                        suffix: verb.slice(prefix.length),
                    });
                }
            }
        }

        return matches;
    }, [input, verbPill, verbPaletteEnabled, serverAvailable, serverSuggestions]);

    // Current completion suggestion (for ghosted display)
    // completionIndex of -1 means dismissed by user
    const currentCompletion = completionMatches.length > 0 && completionIndex >= 0
        ? completionMatches[completionIndex % completionMatches.length]
        : null;

    // Reset completion index when input changes
    useEffect(() => {
        setCompletionIndex(0);
    }, [input]);

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

    // Navigate through command history (caller handles cursor/multiline checks)
    const navigateHistory = useCallback((direction: "up" | "down") => {
        const newOffset = direction === "up" ? historyOffset + 1 : historyOffset - 1;

        // Compute the updated buffer (need to use it immediately, not wait for state)
        const updatedBuffer = { ...historyBuffer, [historyOffset]: input };
        setHistoryBuffer(updatedBuffer);

        setHistoryOffset(newOffset);

        // Check if we have a buffered (edited) version for this position
        if (newOffset in updatedBuffer) {
            setInput(updatedBuffer[newOffset]);
            setSayPillActive(newOffset === 0 ? sayModeEnabled : false);
        } else {
            // No buffered version - use original history or empty for new input
            const historyIndex = commandHistory.length - newOffset;
            if (historyIndex >= 0 && historyIndex < commandHistory.length) {
                const historyValue = commandHistory[historyIndex];
                setInput(historyValue ? historyValue.trimEnd() : "");
                setSayPillActive(false);
            } else {
                setInput("");
                setSayPillActive(sayModeEnabled);
            }
        }
    }, [historyOffset, commandHistory, sayModeEnabled, input, historyBuffer]);

    // Send input to server
    const sendInput = useCallback(() => {
        const trimmedInput = input.trim();

        // Allow sending verb pill alone (e.g., "look" with no args)
        if (!trimmedInput && !verbPill) {
            return;
        }

        // Split by lines and send each non-empty line
        const lines = trimmedInput ? input.split("\n") : [""];
        const commandsSent: string[] = [];
        for (const line of lines) {
            const trimmedLine = line.trim();
            // Build the command
            let messageToSend: string;
            if (verbPill) {
                messageToSend = trimmedLine ? `${verbPill} ${trimmedLine}` : verbPill;
            } else if (sayModeEnabled && sayPillActive) {
                messageToSend = `say ${trimmedLine}`;
            } else {
                messageToSend = trimmedLine;
            }
            if (messageToSend) {
                onSendMessage(messageToSend);
                commandsSent.push(messageToSend);
            }
        }

        // Add the actual command(s) sent to history
        for (const cmd of commandsSent) {
            onAddToHistory(cmd);
        }

        // Clear input and reset state
        setInput("");
        setHistoryOffset(0);
        setHistoryBuffer({}); // Clear working buffer on send
        setVerbPill(null);
        setVerbPillPlaceholder(null);
        setSayPillActive(sayModeEnabled);

        // Pick a new encouraging placeholder (skip if user prefers reduced motion)
        if (!prefersReducedMotion.current) {
            setPlaceholderIndex(Math.floor(Math.random() * ENCOURAGING_PLACEHOLDERS.length));
        }
    }, [input, onSendMessage, onAddToHistory, sayModeEnabled, sayPillActive, verbPill]);

    // Announce to screen readers
    const announce = useCallback((message: string) => {
        setSrAnnouncement(message);
        // Clear after a delay so the same message can be announced again
        setTimeout(() => setSrAnnouncement(""), 1000);
    }, []);

    // Accept a completion match - sets verb pill
    const acceptCompletion = useCallback((match: { verb: string; hint: string | null }) => {
        setVerbPill(match.verb);
        setVerbPillPlaceholder(match.hint);
        setSayPillActive(false);
        setInput("");
        setCompletionIndex(0);
        announce(`${match.verb} command selected`);
    }, [announce]);

    // Clear verb pill and restore appropriate state
    const clearVerbPill = useCallback(() => {
        setVerbPill(null);
        setVerbPillPlaceholder(null);
        if (sayModeEnabled) {
            setSayPillActive(true);
            announce("say mode restored");
        } else {
            announce("command mode");
        }
    }, [sayModeEnabled, announce]);

    // Handle verb selection from palette
    const handleVerbSelect = useCallback((verb: string, placeholder: string | null) => {
        setVerbPill(verb);
        setVerbPillPlaceholder(placeholder);
        setSayPillActive(false);
        announce(`${verb} command selected`);
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
                clearVerbPill();
                return;
            }
            if (sayPillActive && sayModeEnabled) {
                e.preventDefault();
                setSayPillActive(false);
                announce("command mode");
                return;
            }
        }

        // Escape dismisses completions (restores normal Tab behavior)
        if (e.key === "Escape" && completionMatches.length > 0) {
            e.preventDefault();
            setCompletionIndex(-1); // -1 means dismissed
            announce("Completions dismissed");
            return;
        }

        // Tab completion: single match accepts, multiple matches fills to common prefix or cycles
        if (e.key === "Tab" && completionMatches.length > 0 && completionIndex >= 0 && !e.shiftKey) {
            e.preventDefault();

            // Single match - accept it
            if (completionMatches.length === 1) {
                acceptCompletion(completionMatches[0]);
                return;
            }

            // Multiple matches - find common prefix
            const commonPrefix = findCommonPrefix(completionMatches.map(m => m.verb));

            // If common prefix is longer than input, fill to it
            if (commonPrefix.length > input.length) {
                // Use the casing from the first match
                const firstVerb = completionMatches[0].verb;
                setInput(firstVerb.slice(0, commonPrefix.length));
                announce(`Completed to ${commonPrefix}`);
                return;
            }

            // Already at common prefix - cycle through candidates
            const nextIndex = (completionIndex + 1) % completionMatches.length;
            setCompletionIndex(nextIndex);
            const nextCompletion = completionMatches[nextIndex];
            announce(`${nextCompletion.verb}, ${nextIndex + 1} of ${completionMatches.length}`);
            return;
        }

        // Right arrow at end of input accepts completion (fish-style)
        if (e.key === "ArrowRight" && currentCompletion) {
            const textarea = textareaRef.current;
            if (textarea && textarea.selectionStart === textarea.value.length) {
                e.preventDefault();
                acceptCompletion(currentCompletion);
                return;
            }
        }

        // Arrow keys for history navigation (when not in multiline or cursor at edge)
        if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            const textarea = textareaRef.current;
            if (!textarea) return;

            const isMultiline = input.includes("\n");
            const cursorAtEdge = textarea.selectionStart === 0
                || (textarea.selectionStart === textarea.selectionEnd
                    && textarea.selectionStart === textarea.value.length);

            if (!isMultiline || cursorAtEdge) {
                const direction = e.key === "ArrowUp" ? "up" : "down";
                const canNavigate = direction === "up"
                    ? historyOffset < commandHistory.length
                    : historyOffset > 0;

                if (canNavigate) {
                    e.preventDefault();
                    navigateHistory(direction);
                }
            }
        } else if (e.key === "Enter" && e.shiftKey) {
            // Shift+Enter for newlines - let default behavior handle it
            // React will update the state through onChange
        } else if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            sendInput();
        }
    }, [
        acceptCompletion,
        announce,
        clearVerbPill,
        commandHistory,
        completionIndex,
        completionMatches,
        currentCompletion,
        historyOffset,
        input,
        navigateHistory,
        sayModeEnabled,
        sayPillActive,
        sendInput,
        verbPill,
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
    // Screen reader users get minimal placeholders since aria-label provides context
    const getPlaceholder = (): string => {
        // For screen reader users (proxied by prefers-reduced-motion), use minimal placeholders
        // to avoid redundant announcements since aria-label already describes the input
        if (prefersReducedMotion.current) {
            return "";
        }
        if (activePill === "say") {
            return "What would you like to say?";
        }
        if (verbPill) {
            // Use placeholder from server, fall back to static lookup
            if (verbPillPlaceholder) return verbPillPlaceholder;
            const staticPlaceholder = getVerbPlaceholder(verbPill);
            if (staticPlaceholder) return staticPlaceholder;
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
                {activePill
                    ? (
                        <button
                            type="button"
                            className="say-mode-pill"
                            onClick={() => {
                                if (verbPill) {
                                    clearVerbPill();
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
                    )
                    : currentCompletion && (
                        <span
                            className="say-mode-pill say-mode-pill-ghost"
                            aria-hidden="true"
                        >
                            <span className="completion-typed">{input}</span>
                            <span className="completion-suffix">{currentCompletion.suffix}</span>
                        </span>
                    )}
                <div className="input_area_wrapper">
                    <textarea
                        ref={textareaRef}
                        id="input_area"
                        className={`input_area_inner${
                            currentCompletion && !activePill ? " input-completion-active" : ""
                        }`}
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
            </div>

            {
                /* Help text - kept static to avoid re-announcements while typing.
                Completion details are announced via the live region when user interacts (Tab, etc). */
            }
            <div id="input-help" className="sr-only">
                {activePill
                    ? `${activePill} command active. Press Backspace to remove. Use Shift+Enter for new lines.`
                    : "Press Tab for completions, Right Arrow to accept. Use Shift+Enter for new lines. Arrow keys navigate history."}
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
