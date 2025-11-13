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
import { usePersistentState } from "../hooks/usePersistentState";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { buildAuthHeaders } from "../lib/authHeaders";
import { EditorWindow, useTitleBarDrag } from "./EditorWindow";
import { useTheme } from "./ThemeProvider";
import { monacoThemeFor } from "./themeSupport";

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
    const isTouchDevice = useTouchDevice();
    const { theme } = useTheme();
    const monacoTheme = React.useMemo(() => monacoThemeFor(theme), [theme]);
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<SaveError[]>([]);
    const [isSaving, setIsSaving] = useState(false);
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const clampFontSize = (size: number) => Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, size));
    const [fontSize, setFontSize] = usePersistentState<number>(
        FONT_SIZE_STORAGE_KEY,
        () => (isMobile ? 16 : 12),
        {
            serialize: value => clampFontSize(value).toString(),
            deserialize: raw => {
                const parsed = Number(raw);
                return Number.isFinite(parsed) ? clampFontSize(parsed) : null;
            },
        },
    );
    const decreaseFontSize = useCallback(() => {
        setFontSize(prev => clampFontSize(prev - 1));
    }, [setFontSize]);
    const increaseFontSize = useCallback(() => {
        setFontSize(prev => clampFontSize(prev + 1));
    }, [setFontSize]);

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
            if (editorRef.current) {
                editorRef.current.dispose();
            }
        };
    }, []);

    useEffect(() => {
        if (editorRef.current) {
            editorRef.current.updateOptions({ fontSize });
        }
    }, [fontSize]);

    useEffect(() => {
        monaco.editor.setTheme(monacoTheme);
    }, [monacoTheme]);

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

    const handleEditorDidMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monacoInstance: Monaco) => {
        editorRef.current = editor;
        monacoInstance.editor.setTheme(monacoTheme);
        editor.focus();
        setTimeout(() => {
            editor.layout();
        }, 100);
        editor.updateOptions({ fontSize });
    }, [fontSize, monacoTheme]);

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
                        headers: (() => {
                            const headers = buildAuthHeaders(authToken);
                            headers["Content-Type"] = "text/plain";
                            return headers;
                        })(),
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

    const isSplitDraggable = splitMode && typeof onSplitDrag === "function";

    // Title bar component that uses the drag hook (must be inside EditorWindow)
    const TitleBar: React.FC = () => {
        const titleBarDragProps = useTitleBarDrag();

        return (
            <div
                {...(isSplitDraggable
                    ? {
                        onMouseDown: onSplitDrag,
                        onTouchStart: onSplitTouchStart,
                        style: {
                            cursor: "row-resize",
                            touchAction: "none",
                        },
                    }
                    : titleBarDragProps)}
                className="editor-title-bar"
            >
                <h3
                    id="property-editor-title"
                    className="editor-title"
                >
                    <span className="editor-title-label">
                        Property editor
                    </span>
                    <span className="editor-title-path">
                        {enhancedTitle}
                    </span>
                </h3>
                <div className="editor-toolbar">
                    <div
                        className="font-size-control"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <button
                            onClick={decreaseFontSize}
                            aria-label="Decrease editor font size"
                            className="font-size-button"
                            disabled={fontSize <= MIN_FONT_SIZE}
                        >
                            –
                        </button>
                        <span
                            className="font-size-display"
                            aria-live="polite"
                        >
                            {fontSize}px
                        </span>
                        <button
                            onClick={increaseFontSize}
                            aria-label="Increase editor font size"
                            className="font-size-button"
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>
                    {/* Save button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            saveProperty();
                        }}
                        disabled={isSaving}
                        aria-label="Save property"
                        title="Save property"
                        className="editor-btn-save"
                    >
                        {isSaving ? "💾" : "💾"}
                    </button>

                    {/* Split/Float toggle button - only on non-touch devices */}
                    {!isTouchDevice && onToggleSplitMode && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                onToggleSplitMode();
                            }}
                            aria-label={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            title={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            className="btn-ghost editor-btn-toggle-split"
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}
                    <button
                        onClick={onClose}
                        aria-label="Close property editor"
                        className="editor-btn-close"
                    >
                        <span aria-hidden="true">×</span>
                    </button>
                </div>
            </div>
        );
    };

    return (
        <EditorWindow
            visible={visible}
            onClose={onClose}
            splitMode={splitMode}
            defaultPosition={{ x: 50, y: 50 }}
            defaultSize={{ width: 800, height: 600 }}
            minSize={{ width: 400, height: 300 }}
            ariaLabel={`Property editor for ${enhancedTitle}`}
            className="property_editor_container"
        >
            <TitleBar />

            {/* Error panel */}
            {errors.length > 0 && (
                <div className="editor-error-list">
                    <pre className="editor-error-text-pre">
                        {errors.map(formatError).join('\n')}
                    </pre>
                </div>
            )}

            {/* Monaco Editor */}
            <div className="editor-content-area">
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
        </EditorWindow>
    );
};
