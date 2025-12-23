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

// Generic text editor component for editing notes, descriptions, and other freeform content
// Saves via verb invocation rather than property REST API

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { usePersistentState } from "../hooks/usePersistentState";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { MoorVar } from "../lib/MoorVar";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";
import { EditorWindow, useTitleBarDrag } from "./EditorWindow";
import { useTheme } from "./ThemeProvider";
import { monacoThemeFor } from "./themeSupport";

interface TextEditorProps {
    visible: boolean;
    onClose: () => void;
    title: string;
    description?: string; // Explanatory blurb shown to user
    objectCurie: string;
    verbName: string;
    sessionId?: string; // Optional session ID passed as first arg on save
    initialContent: string;
    authToken: string;
    contentType: "text/plain" | "text/djot";
    textMode: "string" | "list"; // How to send content back
    splitMode?: boolean;
    onSplitDrag?: (e: React.MouseEvent) => void;
    onSplitTouchStart?: (e: React.TouchEvent) => void;
    onToggleSplitMode?: () => void;
    isInSplitMode?: boolean;
}

interface SaveError {
    type: "network" | "verb" | "other";
    message: string;
}

const FONT_SIZE_STORAGE_KEY = "moor-text-editor-font-size";
const MIN_FONT_SIZE = 10;
const MAX_FONT_SIZE = 24;

export const TextEditor: React.FC<TextEditorProps> = ({
    visible,
    onClose,
    title,
    description,
    objectCurie,
    verbName,
    sessionId,
    initialContent,
    authToken,
    contentType,
    textMode,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const isTouchDevice = useTouchDevice();
    const { theme } = useTheme();
    const monacoTheme = React.useMemo(() => monacoThemeFor(theme), [theme]);
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<SaveError[]>([]);
    const [isSaving, setIsSaving] = useState(false);
    const [saveSuccess, setSaveSuccess] = useState(false);
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
            case "text/djot":
                return "markdown"; // Use markdown mode for djot (similar syntax)
            default:
                return "plaintext";
        }
    };

    const handleEditorWillMount = useCallback((_monaco: Monaco) => {
        // Nothing special needed for plain text or djot
    }, []);

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
        setSaveSuccess(false); // Clear success indicator on edit
    }, []);

    const saveContent = useCallback(async () => {
        if (isSaving) return;

        setIsSaving(true);
        setErrors([]);
        setSaveSuccess(false);

        try {
            // Build the FlatBuffer args based on text mode
            let argsBuffer: Uint8Array;
            if (textMode === "string") {
                // Send content as a single string
                argsBuffer = MoorVar.buildTextEditorArgs(sessionId, content);
            } else {
                // Send content as a list of strings (one per line)
                const lines = content.split("\n");
                argsBuffer = MoorVar.buildTextEditorArgs(sessionId, lines);
            }

            // Invoke the verb (throws on error)
            await invokeVerbFlatBuffer(authToken, objectCurie, verbName, argsBuffer);

            setErrors([]);
            setSaveSuccess(true);

            // Clear success after a moment
            setTimeout(() => setSaveSuccess(false), 2000);
        } catch (error) {
            const message = error instanceof Error ? error.message : "Unknown save error";
            setErrors([{
                type: message.includes("network") ? "network" : "verb",
                message,
            }]);
        } finally {
            setIsSaving(false);
        }
    }, [authToken, content, isSaving, objectCurie, sessionId, textMode, verbName]);

    const formatError = (error: SaveError): string => {
        return error.message;
    };

    const isSplitDraggable = splitMode && typeof onSplitDrag === "function";

    // Title bar component
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
                    id="text-editor-title"
                    className="editor-title"
                >
                    <span className="editor-title-label">
                        {contentType === "text/djot" ? "Rich text editor" : "Text editor"}
                    </span>
                    <span className="editor-title-path">
                        {title}
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
                            saveContent();
                        }}
                        disabled={isSaving}
                        aria-label="Save content"
                        title="Save content"
                        className="editor-btn-save"
                    >
                        {isSaving ? "⏳" : saveSuccess ? "✓" : "💾"}
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
                        aria-label="Close text editor"
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
            defaultSize={{ width: 700, height: 500 }}
            minSize={{ width: 400, height: 300 }}
            ariaLabel={`Text editor for ${title}`}
            className="text_editor_container"
        >
            <TitleBar />

            {/* Description blurb */}
            {description && (
                <div className="editor-description">
                    {description}
                </div>
            )}

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
                        colorDecorators: false,
                        dragAndDrop: false,
                        emptySelectionClipboard: false,
                        autoClosingDelete: "never",
                        wordWrap: "on", // Always wrap for text content
                        lineNumbers: "on",
                        folding: !isMobile,
                        renderWhitespace: isMobile ? "none" : "selection",
                        stickyScroll: { enabled: false },
                        overviewRulerLanes: 0,
                        hideCursorInOverviewRuler: true,
                        scrollbar: {
                            verticalScrollbarSize: isMobile ? 8 : 10,
                            horizontalScrollbarSize: isMobile ? 8 : 10,
                        },
                        suggest: {
                            showKeywords: false,
                            showSnippets: contentType === "text/djot",
                        },
                        quickSuggestions: {
                            other: false,
                            comments: false,
                            strings: false,
                        },
                    }}
                />
            </div>
        </EditorWindow>
    );
};
