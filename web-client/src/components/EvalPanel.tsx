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

// MOO code evaluation panel - executes MOO code and displays results

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { usePersistentState } from "../hooks/usePersistentState";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { registerMooLanguage } from "../lib/monaco-moo";
import { mooCompletionManager } from "../lib/monaco-moo-completions";
import { performEvalMoorVar } from "../lib/rpc-fb.js";
import { useTheme } from "./ThemeProvider";
import { monacoThemeFor } from "./themeSupport";

interface EvalPanelProps {
    visible: boolean;
    onClose: () => void;
    authToken: string;
    splitMode?: boolean;
    onSplitDrag?: (e: React.MouseEvent) => void;
    onSplitTouchStart?: (e: React.TouchEvent) => void;
    onToggleSplitMode?: () => void;
    isInSplitMode?: boolean;
}

export const EvalPanel: React.FC<EvalPanelProps> = ({
    visible,
    onClose,
    authToken,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const isTouchDevice = useTouchDevice();
    const [content, setContent] = useState("// Enter MOO code to evaluate\nreturn 1 + 1;");
    const [result, setResult] = useState<string | null>(null);
    const [error, setError] = useState<
        { message: string; span?: { start: number; end: number }; line?: number; col?: number } | null
    >(null);
    const [isEvaluating, setIsEvaluating] = useState(false);
    const [position, setPosition] = useState({ x: 50, y: 50 });
    const [size, setSize] = useState({ width: 800, height: 600 });
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
    const [editorHeight, setEditorHeight] = useState(60); // Percentage of editor pane
    const [isSplitDragging, setIsSplitDragging] = useState(false);
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const errorDecorationsRef = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
    const modelUriRef = useRef<string | null>(null);
    const { theme } = useTheme();
    const monacoTheme = React.useMemo(() => monacoThemeFor(theme), [theme]);

    const FONT_SIZE_STORAGE_KEY = "moor-eval-panel-font-size";
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 24;

    const clampFontSize = (size: number) => Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, size));
    const [fontSize, setFontSize] = usePersistentState<number>(
        FONT_SIZE_STORAGE_KEY,
        () => (isMobile ? 14 : 12),
        {
            serialize: value => clampFontSize(value).toString(),
            deserialize: raw => {
                const parsed = Number(raw);
                return Number.isFinite(parsed) ? clampFontSize(parsed) : null;
            },
        },
    );

    // Apply font size to editor when it changes
    useEffect(() => {
        if (editorRef.current) {
            editorRef.current.updateOptions({ fontSize });
        }
    }, [fontSize]);

    useEffect(() => {
        monaco.editor.setTheme(monacoTheme);
    }, [monacoTheme]);

    // Update completion context when authToken changes
    useEffect(() => {
        if (modelUriRef.current) {
            mooCompletionManager.updateContext(modelUriRef.current, { authToken });
        }
    }, [authToken]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (modelUriRef.current) {
                mooCompletionManager.unregister(modelUriRef.current);
                modelUriRef.current = null;
            }
            if (editorRef.current) {
                editorRef.current.dispose();
            }
        };
    }, []);

    // Update error decorations when error changes
    useEffect(() => {
        if (!editorRef.current || !errorDecorationsRef.current) return;

        if (error && error.line && error.col) {
            const model = editorRef.current.getModel();
            if (model) {
                const line = Math.min(error.line, model.getLineCount());
                const lineMaxColumn = model.getLineMaxColumn(line);
                const col = Math.min(error.col, lineMaxColumn);

                // Create a range for the error location (highlight the word)
                const lineContent = model.getLineContent(line);
                const wordEnd = lineContent.indexOf(" ", col - 1);
                const endCol = wordEnd !== -1 ? wordEnd + 1 : Math.min(col + 5, lineMaxColumn);

                const range = new monaco.Range(line, col, line, Math.max(col + 1, endCol));

                errorDecorationsRef.current.set([
                    {
                        range,
                        options: {
                            isWholeLine: false,
                            className: "moo-error-inline",
                            glyphMarginClassName: "codicon codicon-error",
                            glyphMarginHoverMessage: { value: error.message },
                        },
                    },
                ]);
            }
        } else {
            errorDecorationsRef.current.clear();
        }
    }, [error]);

    const handleEvaluate = useCallback(async () => {
        setIsEvaluating(true);
        setError(null);
        setResult(null);

        try {
            const moorVar = await performEvalMoorVar(authToken, content);

            // Use MOO literal representation for display
            const literal = moorVar.toLiteral();
            setResult(literal);
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            // Try to extract span information from error message if available
            // Parse error format: "Eval failed: Parse error at line 2, col 4: message (span info in debug output)"
            const parseErrorMatch = errorMsg.match(/Parse error at line (\d+), col (\d+)/);

            setError({
                message: errorMsg,
                line: parseErrorMatch ? parseInt(parseErrorMatch[1], 10) : undefined,
                col: parseErrorMatch ? parseInt(parseErrorMatch[2], 10) : undefined,
            });
        } finally {
            setIsEvaluating(false);
        }
    }, [authToken, content]);

    // Handle keyboard shortcuts
    const handleEditorMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monacoInstance: Monaco) => {
        editorRef.current = editor;

        // Create custom decoration collection for error highlighting
        errorDecorationsRef.current = editor.createDecorationsCollection();

        // Add CSS for error decorations
        const style = document.createElement("style");
        style.textContent = `
            .monaco-editor .moo-error-decoration {
                background: rgba(255, 0, 0, 0.2) !important;
                border: 1px solid #ff0000 !important;
                border-radius: 2px !important;
            }
            .monaco-editor .moo-error-inline {
                background: rgba(255, 0, 0, 0.3) !important;
                color: #ffffff !important;
                font-weight: bold !important;
                text-decoration: underline wavy #ff0000 !important;
            }
        `;
        document.head.appendChild(style);

        // Set up MOO language
        registerMooLanguage(monacoInstance);

        // Register this editor's context with the completion manager
        const model = editor.getModel();
        if (model) {
            const modelUri = model.uri.toString();
            if (modelUriRef.current && modelUriRef.current !== modelUri) {
                mooCompletionManager.unregister(modelUriRef.current);
            }
            modelUriRef.current = modelUri;
            mooCompletionManager.register(modelUri, { authToken }, monacoInstance);
        }

        monacoInstance.editor.setTheme(monacoTheme);

        // Add keybinding for Ctrl+Enter / Cmd+Enter to evaluate
        editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
            handleEvaluate();
        });

        // Focus the editor
        editor.focus();
    }, [authToken, handleEvaluate, monacoTheme]);

    // Dragging and resizing handlers
    const handleMouseDown = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
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

    const handleSplitDragStart = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        setIsSplitDragging(true);
        e.preventDefault();
        e.stopPropagation();
    }, []);

    const handleSplitMouseMove = useCallback((e: MouseEvent) => {
        if (isSplitDragging) {
            const editor = document.querySelector(".editor-body") as HTMLElement;
            if (editor) {
                const rect = editor.getBoundingClientRect();
                const relativeY = e.clientY - rect.top;
                const percentage = Math.max(20, Math.min(80, (relativeY / rect.height) * 100));
                setEditorHeight(percentage);
            }
        }
    }, [isSplitDragging]);

    const handleSplitMouseUp = useCallback(() => {
        setIsSplitDragging(false);
    }, []);

    // Add global mouse event listeners
    useEffect(() => {
        if (isDragging || isResizing || isSplitDragging) {
            document.addEventListener("mousemove", isDragging || isResizing ? handleMouseMove : handleSplitMouseMove);
            document.addEventListener("mouseup", isDragging || isResizing ? handleMouseUp : handleSplitMouseUp);
            document.body.style.userSelect = "none";

            return () => {
                document.removeEventListener(
                    "mousemove",
                    isDragging || isResizing ? handleMouseMove : handleSplitMouseMove,
                );
                document.removeEventListener("mouseup", isDragging || isResizing ? handleMouseUp : handleSplitMouseUp);
                document.body.style.userSelect = "";
            };
        }
    }, [
        isDragging,
        isResizing,
        isSplitDragging,
        handleMouseMove,
        handleMouseUp,
        handleSplitMouseMove,
        handleSplitMouseUp,
    ]);

    if (!visible) {
        return null;
    }

    // Split mode styling
    const splitStyle = {
        width: "100%",
        height: "100%",
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        display: "flex",
        flexDirection: "column" as const,
        overflow: "hidden",
    };

    // Modal mode styling
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
            className="editor_container"
            role={splitMode ? "region" : "dialog"}
            aria-modal={splitMode ? undefined : "true"}
            aria-labelledby="eval-panel-title"
            tabIndex={-1}
            style={splitMode ? splitStyle : modalStyle}
        >
            {/* Title bar */}
            <div
                className="editor-title-bar"
                onMouseDown={titleMouseDownHandler}
                onTouchStart={titleTouchStartHandler}
                style={{
                    borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                    cursor: isSplitDraggable
                        ? "row-resize"
                        : (splitMode ? "default" : (isDragging ? "grabbing" : "grab")),
                    touchAction: isSplitDraggable ? "none" : "auto",
                }}
            >
                <h3 id="eval-panel-title" className="editor-title">
                    <span className="editor-title-label">Eval λ</span>
                </h3>
                <div className="editor-toolbar">
                    {/* Font size controls */}
                    <div className="font-size-control" onClick={(e) => e.stopPropagation()}>
                        <button
                            onClick={() => {
                                const newSize = Math.max(MIN_FONT_SIZE, fontSize - 1);
                                setFontSize(newSize);
                            }}
                            aria-label="Decrease font size"
                            className="font-size-button"
                            disabled={fontSize <= MIN_FONT_SIZE}
                        >
                            –
                        </button>
                        <span className="font-size-display" aria-live="polite">
                            {fontSize}px
                        </span>
                        <button
                            onClick={() => {
                                const newSize = Math.min(MAX_FONT_SIZE, fontSize + 1);
                                setFontSize(newSize);
                            }}
                            aria-label="Increase font size"
                            className="font-size-button"
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>

                    {/* Split/Float toggle button - only on non-touch devices */}
                    {!isTouchDevice && onToggleSplitMode && (
                        <button
                            className="btn btn-secondary btn-sm"
                            onClick={(e) => {
                                e.stopPropagation();
                                onToggleSplitMode();
                            }}
                            aria-label={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            title={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}

                    <button className="editor-btn-close" onClick={onClose} aria-label="Close eval panel">
                        <span aria-hidden="true">×</span>
                    </button>
                </div>
            </div>

            {/* Editor area with horizontal split */}
            <div className="editor-body" style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0 }}>
                {/* Editor pane */}
                <div style={{ height: `${editorHeight}%`, overflow: "hidden", flex: "0 0 auto" }}>
                    <Editor
                        height="100%"
                        defaultLanguage="moo"
                        value={content}
                        onChange={(value) => {
                            setContent(value || "");
                            // Clear error when user starts typing
                            if (error) {
                                setError(null);
                            }
                        }}
                        onMount={handleEditorMount}
                        options={{
                            fontSize,
                            minimap: { enabled: false },
                            scrollBeyondLastLine: false,
                            wordWrap: "on",
                            automaticLayout: true,
                            tabSize: 4,
                        }}
                        theme={document.documentElement.classList.contains("dark") ? "vs-dark" : "vs"}
                    />
                </div>

                {/* Draggable splitter bar */}
                <div
                    className={`browser-resize-handle ${isSplitDragging ? "dragging" : ""}`}
                    onMouseDown={handleSplitDragStart}
                    style={{
                        zIndex: 10,
                    }}
                />

                {/* Results pane */}
                <div
                    style={{
                        height: `${100 - editorHeight}%`,
                        overflow: "auto",
                        flex: "1",
                        minHeight: 0,
                        backgroundColor: "var(--color-bg-secondary)",
                    }}
                >
                    {result || error
                        ? (
                            <div style={{ padding: "12px 16px" }}>
                                {error
                                    ? (
                                        <div className="editor-error-text">
                                            <strong>Error:</strong> {error.message}
                                        </div>
                                    )
                                    : (
                                        <pre
                                            style={{
                                                margin: 0,
                                                fontFamily: "var(--font-mono)",
                                                fontSize: "0.875rem",
                                                whiteSpace: "pre-wrap",
                                                wordBreak: "break-word",
                                            }}
                                        >
                                        {result}
                                        </pre>
                                    )}
                            </div>
                        )
                        : (
                            <div
                                style={{
                                    padding: "12px 16px",
                                    color: "var(--color-text-tertiary)",
                                    fontSize: "0.875rem",
                                }}
                            >
                                Results will appear here
                            </div>
                        )}
                </div>
            </div>

            {/* Bottom toolbar */}
            <div className="editor-footer">
                <div className="editor-footer-info">
                    Press Ctrl+Enter / Cmd+Enter to evaluate
                </div>
                <div className="editor-footer-actions">
                    <button
                        onClick={handleEvaluate}
                        disabled={isEvaluating}
                        className="btn btn-primary btn-sm"
                    >
                        {isEvaluating ? "Evaluating..." : "Evaluate"}
                    </button>
                </div>
            </div>

            {/* Resize handle - only in modal mode */}
            {!splitMode && (
                <div
                    className="editor-resize-handle"
                    onMouseDown={handleResizeMouseDown}
                    onTouchStart={(e) => {
                        if (e.touches.length === 1) {
                            const touch = e.touches[0];
                            handleResizeMouseDown({
                                ...e,
                                button: 0,
                                clientX: touch.clientX,
                                clientY: touch.clientY,
                            } as unknown as React.MouseEvent);
                        }
                    }}
                    aria-hidden="true"
                />
            )}
        </div>
    );
};
