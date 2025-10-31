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

// Property editor component for editing MOO object properties
// Optimized for plain text with future support for HTML and markdown

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";

interface PropertyEditorProps {
    visible: boolean;
    onClose: () => void;
    title: string;
    objectCurie: string;
    propertyName: string;
    initialContent: string;
    authToken: string;
    uploadAction?: string; // For MCP-triggered editors
    onSendMessage?: (message: string) => boolean; // WebSocket send function
    splitMode?: boolean; // When true, renders as embedded split component instead of modal
    onSplitDrag?: (e: React.MouseEvent) => void; // Handler for split dragging in split mode
    onSplitTouchStart?: (e: React.TouchEvent) => void; // Handler for split touch dragging in split mode
    onToggleSplitMode?: () => void; // Handler to toggle between split and floating modes
    isInSplitMode?: boolean; // Whether currently in split mode (for icon display)
    contentType?: "text/plain" | "text/html" | "text/markdown"; // Future support for different content types
}

interface SaveError {
    type: "network" | "other";
    message: string;
}

const FONT_SIZE_STORAGE_KEY = "moor-code-editor-font-size";
const MIN_FONT_SIZE = 10;
const MAX_FONT_SIZE = 24;

export const PropertyEditor: React.FC<PropertyEditorProps> = ({
    visible,
    onClose,
    title,
    objectCurie,
    propertyName,
    initialContent,
    authToken,
    uploadAction,
    onSendMessage,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
    contentType = "text/plain",
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<SaveError[]>([]);
    const [isSaving, setIsSaving] = useState(false);
    const [position, setPosition] = useState({ x: 50, y: 50 });
    const [size, setSize] = useState({ width: 800, height: 600 });
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const editorThemeObserverRef = useRef<MutationObserver | null>(null);
    const editorThemeListenerRef = useRef<(() => void) | null>(null);
    const containerRef = useRef<HTMLDivElement | null>(null);
    const [fontSize, setFontSize] = useState(() => {
        const fallback = isMobile ? 16 : 12;
        if (typeof window === "undefined") {
            return fallback;
        }
        const stored = window.localStorage.getItem(FONT_SIZE_STORAGE_KEY);
        if (!stored) {
            return fallback;
        }
        const parsed = parseInt(stored, 10);
        if (!Number.isFinite(parsed)) {
            return fallback;
        }
        return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, parsed));
    });
    const decreaseFontSize = useCallback(() => {
        setFontSize(prev => Math.max(MIN_FONT_SIZE, prev - 1));
    }, []);
    const increaseFontSize = useCallback(() => {
        setFontSize(prev => Math.min(MAX_FONT_SIZE, prev + 1));
    }, []);

    // Parse actual object ID from uploadAction and create enhanced title
    const enhancedTitle = React.useMemo(() => {
        if (uploadAction) {
            const propertyMatch = uploadAction.match(/@set-note-string\s+#(\d+)\.(\w+)/);
            if (propertyMatch) {
                const actualObjectId = propertyMatch[1];
                const actualPropertyName = propertyMatch[2];
                return `${title} (#${actualObjectId}.${actualPropertyName})`;
            }
        }
        return title;
    }, [title, uploadAction]);

    // Reset content when initial content changes
    useEffect(() => {
        setContent(initialContent);
    }, [initialContent]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (editorThemeObserverRef.current) {
                editorThemeObserverRef.current.disconnect();
                editorThemeObserverRef.current = null;
            }
            if (editorThemeListenerRef.current) {
                window.removeEventListener("storage", editorThemeListenerRef.current);
                editorThemeListenerRef.current = null;
            }
            if (editorRef.current) {
                editorRef.current.dispose();
            }
        };
    }, []);

    // Mouse event handlers for dragging
    const handleMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return; // Only left mouse button
        setIsDragging(true);
        setDragStart({
            x: e.clientX - position.x,
            y: e.clientY - position.y,
        });
        e.preventDefault();
    }, [position]);

    const handleMouseMove = useCallback((e: MouseEvent) => {
        if (isDragging) {
            const newX = e.clientX - dragStart.x;
            const newY = e.clientY - dragStart.y;

            // Keep window within viewport bounds
            const maxX = window.innerWidth - size.width;
            const maxY = window.innerHeight - size.height;

            setPosition({
                x: Math.max(0, Math.min(maxX, newX)),
                y: Math.max(0, Math.min(maxY, newY)),
            });
        } else if (isResizing) {
            const deltaX = e.clientX - resizeStart.x;
            const deltaY = e.clientY - resizeStart.y;

            const newWidth = Math.max(400, resizeStart.width + deltaX);
            const newHeight = Math.max(300, resizeStart.height + deltaY);

            setSize({ width: newWidth, height: newHeight });
        }
    }, [isDragging, isResizing, dragStart, resizeStart, size]);

    const handleMouseUp = useCallback(() => {
        setIsDragging(false);
        setIsResizing(false);
    }, []);

    const handleResizeMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        setIsResizing(true);
        setResizeStart({
            x: e.clientX,
            y: e.clientY,
            width: size.width,
            height: size.height,
        });
        e.preventDefault();
        e.stopPropagation();
    }, [size]);

    // Add global mouse event listeners
    useEffect(() => {
        if (isDragging || isResizing) {
            document.addEventListener("mousemove", handleMouseMove);
            document.addEventListener("mouseup", handleMouseUp);
            document.body.style.userSelect = "none";

            return () => {
                document.removeEventListener("mousemove", handleMouseMove);
                document.removeEventListener("mouseup", handleMouseUp);
                document.body.style.userSelect = "";
            };
        }
    }, [isDragging, isResizing, handleMouseMove, handleMouseUp]);

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem(FONT_SIZE_STORAGE_KEY, fontSize.toString());
        }
        if (editorRef.current) {
            editorRef.current.updateOptions({ fontSize });
        }
    }, [fontSize]);

    // Get Monaco language based on content type
    const getLanguage = (type: string) => {
        switch (type) {
            case "text/html":
                return "html";
            case "text/markdown":
                return "markdown";
            default:
                return "plaintext";
        }
    };

    const handleEditorWillMount = useCallback((_monaco: Monaco) => {
        // For plain text, we don't need custom language configuration
        // Monaco already handles plaintext, html, and markdown

        // We could add custom completions for HTML or markdown in the future
        if (contentType === "text/html") {
            // Future: Add custom HTML completions or validation
        } else if (contentType === "text/markdown") {
            // Future: Add custom markdown completions or shortcuts
        }
    }, [contentType]);

    const handleEditorDidMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monaco: Monaco) => {
        editorRef.current = editor;

        // Set Monaco theme to match client theme
        const savedTheme = localStorage.getItem("theme");
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        const isDarkTheme = savedTheme ? savedTheme === "dark" : prefersDark;

        monaco.editor.setTheme(isDarkTheme ? "vs-dark" : "vs");

        // Listen for theme changes
        const handleThemeChange = () => {
            const currentTheme = localStorage.getItem("theme");
            const currentPrefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
            const currentIsDarkTheme = currentTheme ? currentTheme === "dark" : currentPrefersDark;
            monaco.editor.setTheme(currentIsDarkTheme ? "vs-dark" : "vs");
        };

        // Listen for storage changes (theme toggle)
        window.addEventListener("storage", handleThemeChange);
        editorThemeListenerRef.current = handleThemeChange;

        // Also listen for changes to the light-theme class on body
        const observer = new MutationObserver((mutations) => {
            mutations.forEach((mutation) => {
                if (mutation.type === "attributes" && mutation.attributeName === "class") {
                    handleThemeChange();
                }
            });
        });
        editorThemeObserverRef.current = observer;
        observer.observe(document.body, { attributes: true });

        // Focus the editor
        editor.focus();

        // Force layout update to prevent artifacts
        setTimeout(() => {
            editor.layout();
        }, 100);
        editor.updateOptions({ fontSize });
    }, [fontSize]);

    const handleEditorChange = useCallback((value: string | undefined) => {
        setContent(value || "");
    }, []);

    const saveProperty = useCallback(async () => {
        if (isSaving) return;

        setIsSaving(true);
        setErrors([]);

        try {
            if (uploadAction && onSendMessage) {
                // MCP-style saving via WebSocket
                console.log("Saving via WebSocket with upload action:", uploadAction);

                // Send the upload action first
                const uploadSent = onSendMessage(uploadAction);
                if (!uploadSent) {
                    throw new Error("Failed to send upload action via WebSocket");
                }

                // Send each line of content
                const lines = content.split("\n");
                for (const line of lines) {
                    const lineSent = onSendMessage(line);
                    if (!lineSent) {
                        throw new Error("Failed to send content line via WebSocket");
                    }
                }

                // Send terminator
                const terminatorSent = onSendMessage(".");
                if (!terminatorSent) {
                    throw new Error("Failed to send terminator via WebSocket");
                }

                // For now, assume success (real error handling would need server response parsing)
                setErrors([]);
                console.log("WebSocket property save completed");
            } else {
                // REST API saving for present-triggered editors
                const response = await fetch(
                    `/properties/${encodeURIComponent(objectCurie)}/${encodeURIComponent(propertyName)}`,
                    {
                        method: "POST",
                        headers: {
                            "X-Moor-Auth-Token": authToken,
                            "Content-Type": "text/plain",
                        },
                        body: content,
                    },
                );

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
                }

                // For properties, we typically don't expect complex error responses
                // Just check if the request succeeded
                setErrors([]);
            }
        } catch (error) {
            setErrors([{
                type: "other",
                message: error instanceof Error ? error.message : "Unknown save error",
            }]);
        } finally {
            setIsSaving(false);
        }
    }, [authToken, content, objectCurie, propertyName, uploadAction, onSendMessage, isSaving]);

    const formatError = (error: SaveError): string => {
        return error.message;
    };

    // Focus management for modal
    useEffect(() => {
        if (!visible) return;

        // Store the previously focused element
        const previouslyFocused = document.activeElement as HTMLElement;

        // Focus the modal container when it opens
        if (containerRef.current) {
            containerRef.current.focus();
        }

        // Handle keyboard events for focus trapping
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                onClose();
                return;
            }

            if (e.key === "Tab") {
                const focusableElements = containerRef.current?.querySelectorAll(
                    "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])",
                );

                if (!focusableElements || focusableElements.length === 0) return;

                const firstElement = focusableElements[0] as HTMLElement;
                const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                if (e.shiftKey) {
                    // Shift+Tab: if focus is on first element, move to last
                    if (document.activeElement === firstElement) {
                        e.preventDefault();
                        lastElement.focus();
                    }
                } else {
                    // Tab: if focus is on last element, move to first
                    if (document.activeElement === lastElement) {
                        e.preventDefault();
                        firstElement.focus();
                    }
                }
            }
        };

        document.addEventListener("keydown", handleKeyDown);

        // Cleanup: restore focus when modal closes
        return () => {
            document.removeEventListener("keydown", handleKeyDown);
            if (previouslyFocused) {
                previouslyFocused.focus();
            }
        };
    }, [visible, onClose]);

    if (!visible) {
        return null;
    }

    // Split mode styling - fills container
    const splitStyle = {
        width: "100%",
        height: "100%",
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        display: "flex",
        flexDirection: "column" as const,
        overflow: "hidden",
    };

    // Modal mode styling - floating window
    const modalStyle = {
        position: "fixed" as const,
        top: `${position.y}px`,
        left: `${position.x}px`,
        width: `${size.width}px`,
        height: `${size.height}px`,
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        borderRadius: "var(--radius-lg)",
        boxShadow: "0 8px 32px var(--color-shadow)",
        zIndex: 1000,
        display: "flex",
        flexDirection: "column" as const,
        cursor: isDragging ? "grabbing" : "default",
    };

    const isSplitDraggable = splitMode && typeof onSplitDrag === "function";
    const titleMouseDownHandler = isSplitDraggable ? onSplitDrag : (splitMode ? undefined : handleMouseDown);
    const titleTouchStartHandler = isSplitDraggable ? onSplitTouchStart : undefined;

    return (
        <div
            ref={containerRef}
            className="property_editor_container"
            role={splitMode ? "region" : "dialog"}
            aria-modal={splitMode ? undefined : "true"}
            aria-labelledby="property-editor-title"
            tabIndex={-1}
            style={splitMode ? splitStyle : modalStyle}
        >
            {/* Title bar */}
            <div
                onMouseDown={titleMouseDownHandler}
                onTouchStart={titleTouchStartHandler}
                style={{
                    padding: "var(--space-md)",
                    borderBottom: "1px solid var(--color-border-light)",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    backgroundColor: "var(--color-bg-header)",
                    borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                    cursor: isSplitDraggable
                        ? "row-resize"
                        : (splitMode ? "default" : (isDragging ? "grabbing" : "grab")),
                    touchAction: isSplitDraggable ? "none" : "auto", // Prevent default touch behaviors when in split mode
                }}
            >
                <h3
                    id="property-editor-title"
                    style={{
                        margin: 0,
                        color: "var(--color-text-primary)",
                        display: "flex",
                        alignItems: "baseline",
                        width: "100%",
                    }}
                >
                    <span style={{ fontWeight: "700" }}>
                        Property editor
                    </span>
                    <span
                        style={{
                            fontSize: "0.9em",
                            color: "var(--color-text-secondary)",
                            fontWeight: "normal",
                            textAlign: "center",
                            flex: 1,
                            marginLeft: "var(--space-sm)",
                            marginRight: "var(--space-sm)",
                            fontFamily: "monospace",
                        }}
                    >
                        {enhancedTitle}
                    </span>
                </h3>
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "4px",
                            backgroundColor: "var(--color-bg-secondary)",
                            border: "1px solid var(--color-border-medium)",
                            borderRadius: "var(--radius-sm)",
                            padding: "2px 6px",
                        }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <button
                            onClick={decreaseFontSize}
                            aria-label="Decrease editor font size"
                            style={{
                                background: "transparent",
                                border: "none",
                                color: "var(--color-text-secondary)",
                                cursor: fontSize <= MIN_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize <= MIN_FONT_SIZE ? 0.5 : 1,
                                fontSize: "14px",
                                padding: "2px 4px",
                            }}
                            disabled={fontSize <= MIN_FONT_SIZE}
                        >
                            –
                        </button>
                        <span
                            style={{
                                fontFamily: "var(--font-mono)",
                                fontSize: "12px",
                                color: "var(--color-text-secondary)",
                                minWidth: "38px",
                                textAlign: "center",
                            }}
                            aria-live="polite"
                        >
                            {fontSize}px
                        </span>
                        <button
                            onClick={increaseFontSize}
                            aria-label="Increase editor font size"
                            style={{
                                background: "transparent",
                                border: "none",
                                color: "var(--color-text-secondary)",
                                cursor: fontSize >= MAX_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize >= MAX_FONT_SIZE ? 0.5 : 1,
                                fontSize: "14px",
                                padding: "2px 4px",
                            }}
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>
                    {/* Save button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation(); // Prevent drag handler from firing
                            saveProperty();
                        }}
                        disabled={isSaving}
                        aria-label="Save property"
                        title="Save property"
                        style={{
                            backgroundColor: isSaving ? "var(--color-bg-secondary)" : "var(--color-button-primary)",
                            color: "white",
                            border: "none",
                            padding: "6px 12px",
                            borderRadius: "var(--radius-sm)",
                            cursor: isSaving ? "not-allowed" : "pointer",
                            opacity: isSaving ? 0.6 : 1,
                            fontSize: "12px",
                            fontWeight: "600",
                        }}
                    >
                        {isSaving ? "💾" : "💾"}
                    </button>

                    {/* Split/Float toggle button - only on desktop */}
                    {!isMobile && onToggleSplitMode && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation(); // Prevent drag handler from firing
                                onToggleSplitMode();
                            }}
                            aria-label={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            title={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            style={{
                                background: "transparent",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "var(--radius-sm)",
                                cursor: "pointer",
                                color: "var(--color-text-secondary)",
                                padding: "4px 6px",
                                fontSize: "12px",
                                display: "flex",
                                alignItems: "center",
                            }}
                        >
                            {isInSplitMode ? "🪟" : "⇅"}
                        </button>
                    )}
                    <button
                        onClick={onClose}
                        aria-label="Close property editor"
                        style={{
                            background: "transparent",
                            border: "none",
                            fontSize: "1.2em",
                            cursor: "pointer",
                            color: "var(--color-text-secondary)",
                            padding: "4px 8px",
                        }}
                    >
                        <span aria-hidden="true">×</span>
                    </button>
                </div>
            </div>

            {/* Error panel */}
            {errors.length > 0 && (
                <div
                    className="property_save_errors"
                    style={{
                        height: "80px",
                        padding: "var(--space-sm)",
                        backgroundColor: "var(--color-bg-error)",
                        borderTop: "1px solid var(--color-border-light)",
                        borderBottom: "1px solid var(--color-border-light)",
                        overflowY: "auto",
                    }}
                >
                    <pre
                        style={{
                            margin: 0,
                            color: "var(--color-text-error)",
                            fontSize: "0.9em",
                            fontFamily: "var(--font-mono)",
                        }}
                    >
                        {errors.map(formatError).join('\n')}
                    </pre>
                </div>
            )}

            {/* Monaco Editor */}
            <div
                style={{
                    flex: 1,
                    minHeight: 0,
                    position: "relative",
                    overflow: "hidden", // Prevent rendering artifacts
                }}
            >
                <Editor
                    value={content}
                    language={getLanguage(contentType)}
                    theme="vs-dark"
                    onChange={handleEditorChange}
                    beforeMount={handleEditorWillMount}
                    onMount={handleEditorDidMount}
                    options={{
                        minimap: { enabled: !isMobile },
                        fontSize,
                        fontFamily: "Monaco, Menlo, \"Ubuntu Mono\", monospace",
                        automaticLayout: true,
                        colorDecorators: contentType === "text/html",
                        dragAndDrop: false,
                        emptySelectionClipboard: false,
                        autoClosingDelete: "never",
                        wordWrap: isMobile ? "on" : "off",
                        lineNumbers: "on",
                        folding: !isMobile,
                        renderWhitespace: isMobile ? "none" : "selection",
                        stickyScroll: { enabled: false }, // Disable sticky scroll
                        overviewRulerLanes: 0, // Disable overview ruler
                        hideCursorInOverviewRuler: true, // Hide cursor in overview
                        scrollbar: {
                            verticalScrollbarSize: isMobile ? 8 : 10, // Thinner scrollbar on mobile
                            horizontalScrollbarSize: isMobile ? 8 : 10,
                        },
                        // Features that make sense for properties but not MOO code
                        suggest: {
                            showKeywords: false, // No MOO keywords for properties
                            showSnippets: contentType !== "text/plain", // Only for markup languages
                        },
                        quickSuggestions: {
                            other: contentType !== "text/plain",
                            comments: false,
                            strings: contentType === "text/html",
                        },
                    }}
                />
            </div>

            {/* Resize handle - only in modal mode */}
            {!splitMode && (
                <div
                    onMouseDown={handleResizeMouseDown}
                    onTouchStart={(e) => {
                        if (e.touches.length === 1) {
                            const touch = e.touches[0];
                            handleResizeMouseDown({
                                ...e,
                                button: 0,
                                clientX: touch.clientX,
                                clientY: touch.clientY,
                                preventDefault: () => e.preventDefault(),
                                stopPropagation: () => e.stopPropagation(),
                            } as unknown as React.MouseEvent<HTMLDivElement>);
                        }
                    }}
                    tabIndex={0}
                    role="button"
                    aria-label="Resize editor window"
                    onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            // Start resize mode - could be enhanced with arrow key support
                            handleResizeMouseDown({
                                ...e,
                                button: 0,
                                clientX: size.width + position.x,
                                clientY: size.height + position.y,
                            } as unknown as React.MouseEvent<HTMLDivElement>);
                        }
                    }}
                    style={{
                        position: "absolute",
                        bottom: 0,
                        right: 0,
                        width: "22px",
                        height: "22px",
                        cursor: "nwse-resize",
                        borderBottomRightRadius: "var(--radius-lg)",
                        borderTopLeftRadius: "6px",
                        backgroundColor: "var(--color-surface-raised)",
                        borderTop: "1px solid var(--color-border-medium)",
                        borderLeft: "1px solid var(--color-border-medium)",
                        boxShadow: "inset 0 0 0 1px rgba(0, 0, 0, 0.1)",
                        zIndex: 5,
                    }}
                >
                    <div
                        style={{
                            position: "absolute",
                            inset: "4px",
                            borderBottom: "2px solid var(--color-border-strong)",
                            borderRight: "2px solid var(--color-border-strong)",
                            borderBottomRightRadius: "4px",
                            pointerEvents: "none",
                        }}
                    />
                    <div
                        style={{
                            position: "absolute",
                            right: "6px",
                            bottom: "6px",
                            width: "10px",
                            height: "10px",
                            clipPath: "polygon(0 100%, 100% 0, 100% 100%)",
                            background:
                                "linear-gradient(135deg, transparent 0%, transparent 30%, var(--color-border-strong) 30%, var(--color-border-strong) 50%, transparent 50%)",
                            pointerEvents: "none",
                        }}
                    />
                    <span
                        aria-hidden="true"
                        style={{
                            position: "absolute",
                            right: "4px",
                            bottom: "2px",
                            fontSize: "14px",
                            color: "var(--color-border-strong)",
                            lineHeight: 1,
                            pointerEvents: "none",
                            userSelect: "none",
                        }}
                    >
                        ↘
                    </span>
                </div>
            )}
        </div>
    );
};
