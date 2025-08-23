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

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";

interface VerbEditorProps {
    visible: boolean;
    onClose: () => void;
    title: string;
    objectCurie: string;
    verbName: string;
    initialContent: string;
    authToken: string;
    uploadAction?: string; // For MCP-triggered editors
    onSendMessage?: (message: string) => boolean; // WebSocket send function
    splitMode?: boolean; // When true, renders as embedded split component instead of modal
    onSplitDrag?: (e: React.MouseEvent) => void; // Handler for split dragging in split mode
    onSplitTouchStart?: (e: React.TouchEvent) => void; // Handler for split touch dragging in split mode
    onToggleSplitMode?: () => void; // Handler to toggle between split and floating modes
    isInSplitMode?: boolean; // Whether currently in split mode (for icon display)
}

interface CompileError {
    type: "parse" | "other";
    message: string;
    line?: number;
    column?: number;
}

export const VerbEditor: React.FC<VerbEditorProps> = ({
    visible,
    onClose,
    title,
    objectCurie,
    verbName,
    initialContent,
    authToken,
    uploadAction,
    onSendMessage,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<CompileError[]>([]);
    const [isCompiling, setIsCompiling] = useState(false);
    const [position, setPosition] = useState({ x: 50, y: 50 });
    const [size, setSize] = useState({ width: 800, height: 600 });
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const containerRef = useRef<HTMLDivElement | null>(null);

    // Reset content when initial content changes
    useEffect(() => {
        setContent(initialContent);
    }, [initialContent]);

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

    // Configure MOO language for Monaco
    const handleEditorWillMount = useCallback((monaco: Monaco) => {
        // Register MOO language
        monaco.languages.register({ id: "moo" });

        // Define MOO language tokens
        monaco.languages.setMonarchTokensProvider("moo", {
            tokenizer: {
                root: [
                    // Keywords
                    [
                        /\b(if|else|elseif|endif|while|endwhile|for|endfor|return|break|continue|try|except|endtry|finally|endfork|fork|let|const|global)\b/,
                        "keyword",
                    ],

                    // Error constants
                    [
                        /\b(E_TYPE|E_DIV|E_PERM|E_PROPNF|E_VERBNF|E_VARNF|E_INVIND|E_RECMOVE|E_MAXREC|E_RANGE|E_ARGS|E_NACC|E_INVARG|E_QUOTA|E_FLOAT)\b/,
                        "constant",
                    ],

                    // Object references
                    [/#-?\d+/, "number.object"],

                    // System references
                    [/\$\w+/, "variable.system"],

                    // Strings
                    [/"([^"\\]|\\.)*$/, "string.invalid"],
                    [/"/, "string", "@string"],

                    // Numbers
                    [/\d*\.\d+([eE][\-+]?\d+)?/, "number.float"],
                    [/0[xX][0-9a-fA-F]+/, "number.hex"],
                    [/\d+/, "number"],

                    // Operators
                    [/[=!<>]=?/, "operator.comparison"],
                    [/[+\-*/%^]/, "operator.arithmetic"],
                    [/[&|]/, "operator.logical"],
                    [/[=]/, "operator.assignment"],

                    // Comments
                    [/\/\*/, "comment", "@comment"],
                    [/\/\/.*$/, "comment"],
                ],

                string: [
                    [/[^\\"]+/, "string"],
                    [/\\./, "string.escape"],
                    [/"/, "string", "@pop"],
                ],

                comment: [
                    [/[^\/*]+/, "comment"],
                    [/\*\//, "comment", "@pop"],
                    [/[\/*]/, "comment"],
                ],
            },
        });

        // Define MOO language configuration
        monaco.languages.setLanguageConfiguration("moo", {
            comments: {
                lineComment: "//",
                blockComment: ["/*", "*/"],
            },
            brackets: [
                ["{", "}"],
                ["[", "]"],
                ["(", ")"],
            ],
            autoClosingPairs: [
                { open: "{", close: "}" },
                { open: "[", close: "]" },
                { open: "(", close: ")" },
                { open: "\"", close: "\"" },
            ],
        });
    }, []);

    const handleEditorDidMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor) => {
        editorRef.current = editor;

        // Focus the editor
        editor.focus();
        
        // Force layout update to prevent artifacts
        setTimeout(() => {
            editor.layout();
        }, 100);
    }, []);

    const handleEditorChange = useCallback((value: string | undefined) => {
        setContent(value || "");
    }, []);

    const compileVerb = useCallback(async () => {
        if (isCompiling) return;

        setIsCompiling(true);
        setErrors([]);

        try {
            if (uploadAction && onSendMessage) {
                // MCP-style compilation via WebSocket
                console.log("Compiling via WebSocket with upload action:", uploadAction);

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
                console.log("WebSocket compilation completed");
            } else {
                // REST API compilation for present-triggered editors
                const response = await fetch(
                    `/verbs/${encodeURIComponent(objectCurie)}/${encodeURIComponent(verbName)}`,
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

                const result = await response.json();

                // Check for compilation errors
                if (result && typeof result === "object" && Object.keys(result).length > 0) {
                    // Parse compilation errors
                    const compilationErrors: CompileError[] = [];

                    if (result.error && result.line && result.column) {
                        compilationErrors.push({
                            type: "parse",
                            message: result.error,
                            line: result.line,
                            column: result.column,
                        });
                    } else if (result.error) {
                        compilationErrors.push({
                            type: "other",
                            message: result.error,
                        });
                    }

                    setErrors(compilationErrors);
                } else {
                    // Successful compilation
                    setErrors([]);
                }
            }
        } catch (error) {
            setErrors([{
                type: "other",
                message: error instanceof Error ? error.message : "Unknown compilation error",
            }]);
        } finally {
            setIsCompiling(false);
        }
    }, [authToken, content, objectCurie, verbName, uploadAction, onSendMessage, isCompiling]);

    const formatError = (error: CompileError): string => {
        if (error.type === "parse" && error.line && error.column) {
            return `At line ${error.line}, column ${error.column}: ${error.message}`;
        }
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

    return (
        <div
            ref={containerRef}
            className="editor_container"
            role={splitMode ? "region" : "dialog"}
            aria-modal={splitMode ? undefined : "true"}
            aria-labelledby="verb-editor-title"
            tabIndex={-1}
            style={splitMode ? splitStyle : modalStyle}
        >
            {/* Title bar */}
            <div
                onMouseDown={splitMode ? onSplitDrag : handleMouseDown}
                onTouchStart={splitMode ? onSplitTouchStart : undefined}
                style={{
                    padding: "var(--space-md)",
                    borderBottom: "1px solid var(--color-border-light)",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    backgroundColor: "var(--color-bg-header)",
                    borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                    cursor: splitMode ? "row-resize" : (isDragging ? "grabbing" : "grab"),
                    touchAction: splitMode ? "none" : "auto", // Prevent default touch behaviors when in split mode
                }}
            >
                <h3 id="verb-editor-title" style={{ margin: 0, color: "var(--color-text-primary)" }}>
                    {title}
                </h3>
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
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
                        aria-label="Close verb editor"
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

            {/* Compile button */}
            <div style={{ padding: "var(--space-sm)" }}>
                <button
                    onClick={compileVerb}
                    disabled={isCompiling}
                    style={{
                        backgroundColor: "var(--color-button-primary)",
                        color: "white",
                        border: "none",
                        padding: "8px 16px",
                        borderRadius: "var(--radius-sm)",
                        cursor: isCompiling ? "not-allowed" : "pointer",
                        opacity: isCompiling ? 0.6 : 1,
                    }}
                >
                    {isCompiling ? "Compiling..." : "Compile"}
                </button>
            </div>

            {/* Error panel */}
            {errors.length > 0 && (
                <div
                    className="verb_compile_errors"
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
            <div style={{ 
                flex: 1, 
                minHeight: 0,
                position: "relative",
                overflow: "hidden" // Prevent rendering artifacts
            }}>
                <Editor
                    value={content}
                    language="moo"
                    theme="vs-dark"
                    onChange={handleEditorChange}
                    beforeMount={handleEditorWillMount}
                    onMount={handleEditorDidMount}
                    options={{
                        minimap: { enabled: !isMobile },
                        fontSize: isMobile ? 16 : 12,
                        fontFamily: "Monaco, Menlo, \"Ubuntu Mono\", monospace",
                        automaticLayout: true,
                        colorDecorators: true,
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
                    }}
                />
            </div>

            {/* Resize handle - only in modal mode */}
            {!splitMode && (
                <div
                    onMouseDown={handleResizeMouseDown}
                    tabIndex={0}
                    role="button"
                    aria-label="Resize editor window"
                    onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            // Start resize mode - could be enhanced with arrow key support
                            handleResizeMouseDown({
                                ...e,
                                clientX: size.width + position.x,
                                clientY: size.height + position.y,
                            } as any);
                        }
                    }}
                    style={{
                        position: "absolute",
                        bottom: 0,
                        right: 0,
                        width: "16px",
                        height: "16px",
                        cursor: "nw-resize",
                        background:
                            "linear-gradient(-45deg, transparent 0%, transparent 30%, var(--color-border-medium) 30%, var(--color-border-medium) 70%, transparent 70%)",
                        borderBottomRightRadius: "var(--radius-lg)",
                    }}
                />
            )}
        </div>
    );
};
