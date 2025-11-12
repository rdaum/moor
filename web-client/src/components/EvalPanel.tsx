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

// MOO code evaluation panel - executes MOO code and displays results

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { usePersistentState } from "../hooks/usePersistentState";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { registerMooLanguage } from "../lib/monaco-moo";
import { registerMooCompletionProvider } from "../lib/monaco-moo-completions";
import { performEvalFlatBuffer } from "../lib/rpc-fb.js";
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
    const [error, setError] = useState<string | null>(null);
    const [isEvaluating, setIsEvaluating] = useState(false);
    const [position, setPosition] = useState({ x: 50, y: 50 });
    const [size, setSize] = useState({ width: 800, height: 600 });
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const completionProviderRef = useRef<monaco.IDisposable | null>(null);
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

    const handleEvaluate = useCallback(async () => {
        setIsEvaluating(true);
        setError(null);
        setResult(null);

        try {
            const evalResult = await performEvalFlatBuffer(authToken, content);

            // Check if result is an error
            if (evalResult && typeof evalResult === "object" && "error" in evalResult) {
                const errorResult = evalResult as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "Evaluation failed";
                setError(msg);
                return;
            }

            // Format the result nicely
            let formattedResult: string;
            if (evalResult === null || evalResult === undefined) {
                formattedResult = "=> None";
            } else if (typeof evalResult === "object") {
                formattedResult = `=> ${JSON.stringify(evalResult, null, 2)}`;
            } else {
                formattedResult = `=> ${String(evalResult)}`;
            }

            setResult(formattedResult);
        } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setIsEvaluating(false);
        }
    }, [authToken, content]);

    // Handle keyboard shortcuts
    const handleEditorMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monacoInstance: Monaco) => {
        editorRef.current = editor;

        // Set up MOO language
        registerMooLanguage(monacoInstance);

        // Dispose old completion provider if it exists
        if (completionProviderRef.current) {
            completionProviderRef.current.dispose();
        }

        // Register MOO completion provider (without object context for eval panel)
        completionProviderRef.current = registerMooCompletionProvider(
            monacoInstance,
            authToken,
        );

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

            {/* Editor area */}
            <div className="editor-body">
                <div className="editor-main">
                    <Editor
                        height="100%"
                        defaultLanguage="moo"
                        value={content}
                        onChange={(value) => setContent(value || "")}
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

                {/* Result/Error display */}
                {(result || error) && (
                    <div className={error ? "editor-error" : "editor-panel-content"}>
                        {error
                            ? (
                                <div className="editor-error-text">
                                    <strong>Error:</strong> {error}
                                </div>
                            )
                            : (
                                <pre className="editor-error-text-pre">
                                {result}
                                </pre>
                            )}
                    </div>
                )}
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
