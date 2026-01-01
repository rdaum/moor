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

import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { usePersistentState } from "../hooks/usePersistentState";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { registerMooLanguage } from "../lib/monaco-moo";
import { registerMooCompletionProvider } from "../lib/monaco-moo-completions";
import { performEvalFlatBuffer } from "../lib/rpc-fb.js";
import { DialogSheet } from "./DialogSheet";
import { EditorWindow, useTitleBarDrag } from "./EditorWindow";
import { useTheme } from "./ThemeProvider";
import { monacoThemeFor } from "./themeSupport";

interface VerbEditorProps {
    visible: boolean;
    onClose: () => void;
    title: string;
    objectCurie: string;
    verbName: string;
    verbNames?: string;
    initialContent: string;
    authToken: string;
    uploadAction?: string; // For MCP-triggered editors
    onSendMessage?: (message: string) => boolean; // WebSocket send function
    splitMode?: boolean; // When true, renders as embedded split component instead of modal
    onSplitDrag?: (e: React.MouseEvent) => void; // Handler for split dragging in split mode
    onSplitTouchStart?: (e: React.TouchEvent) => void; // Handler for split touch dragging in split mode
    onToggleSplitMode?: () => void; // Handler to toggle between split and floating modes
    isInSplitMode?: boolean; // Whether currently in split mode (for icon display)
    // Verb metadata
    owner?: string;
    definer?: string;
    permissions?: { readable: boolean; writable: boolean; executable: boolean; debug: boolean };
    argspec?: { dobj: string; prep: string; iobj: string };
    onSave?: () => void; // Callback to refresh verb data after metadata save
    onDelete?: () => void; // Callback to delete verb
    normalizeObjectInput?: (raw: string) => string; // Utility to convert object references to MOO expressions
    getDollarName?: (objId: string) => string | null; // Get $ name for an object ID
    // Navigation for multiple editors
    onPreviousEditor?: () => void; // Navigate to previous editor
    onNextEditor?: () => void; // Navigate to next editor
    editorCount?: number; // Total number of editors
    currentEditorIndex?: number; // Current editor index (0-based)
}

interface CompileError {
    type: "parse" | "other";
    message: string;
    line?: number;
    column?: number;
    endLine?: number;
    endColumn?: number;
    span?: { start: number; end: number };
    contextLine?: string;
    expectedTokens?: string[];
    notes?: string[];
}

const FONT_SIZE_STORAGE_KEY = "moor-code-editor-font-size";

export const VerbEditor: React.FC<VerbEditorProps> = ({
    visible,
    onClose,
    title,
    objectCurie,
    verbName,
    verbNames,
    initialContent,
    authToken,
    uploadAction,
    onSendMessage,
    splitMode = false,
    onSplitDrag: _onSplitDrag,
    onSplitTouchStart: _onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
    owner,
    definer,
    permissions,
    argspec,
    onSave,
    onDelete,
    normalizeObjectInput,
    getDollarName,
    onPreviousEditor,
    onNextEditor,
    editorCount = 1,
    currentEditorIndex = 0,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const isTouchDevice = useTouchDevice();
    const { theme } = useTheme();
    const monacoTheme = React.useMemo(() => monacoThemeFor(theme), [theme]);
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<CompileError[]>([]);
    const [isCompiling, setIsCompiling] = useState(false);
    const [compileSuccess, setCompileSuccess] = useState(false);
    const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
    const errorDecorationsRef = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
    const completionProviderRef = useRef<monaco.IDisposable | null>(null);
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 24;
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

    // Word wrap state
    const [wordWrap, setWordWrap] = usePersistentState<"on" | "off">(
        "moor-editor-wordwrap",
        () => (isMobile ? "on" : "off"),
        {
            serialize: value => value,
            deserialize: raw => (raw === "on" || raw === "off" ? raw : null),
        },
    );

    // Minimap state
    const [minimapEnabled, setMinimapEnabled] = usePersistentState<boolean>(
        "moor-editor-minimap",
        () => !isMobile,
        {
            serialize: value => (value ? "true" : "false"),
            deserialize: raw => (raw === "true" ? true : raw === "false" ? false : null),
        },
    );

    // Verb metadata editing state
    const [isEditingOwner, setIsEditingOwner] = useState(false);
    const [editOwnerValue, setEditOwnerValue] = useState(owner ? `#${owner}` : "");
    const [editVerbNames, setEditVerbNames] = useState(verbNames?.trim() || verbName);
    const [editArgspec, setEditArgspec] = useState(argspec || { dobj: "", prep: "", iobj: "" });
    const [argspecDraft, setArgspecDraft] = useState(argspec || { dobj: "", prep: "", iobj: "" });
    const [showArgspecDialog, setShowArgspecDialog] = useState(false);
    const [editPermissions, setEditPermissions] = useState(
        permissions || { readable: false, writable: false, executable: false, debug: false },
    );
    const [isSavingMetadata, setIsSavingMetadata] = useState(false);
    const [metadataSaveSuccess, setMetadataSaveSuccess] = useState(false);

    // Sync local state when props change, but don't clear the success state
    useEffect(() => {
        setEditOwnerValue(owner ? `#${owner}` : "");
        setEditVerbNames(verbNames?.trim() || verbName);
        const normalizedArgspec = argspec || { dobj: "", prep: "", iobj: "" };
        setEditArgspec(normalizedArgspec);
        setArgspecDraft(normalizedArgspec);
        setEditPermissions(permissions || { readable: false, writable: false, executable: false, debug: false });
        // Don't clear metadataSaveSuccess - let the timeout handle it
    }, [owner, permissions, verbNames, verbName, argspec]);

    // Parse actual object ID from uploadAction and create enhanced title
    const enhancedTitle = React.useMemo(() => {
        if (uploadAction) {
            const programMatch = uploadAction.match(/@program\s+#(\d+):(\w+)/);
            if (programMatch) {
                const actualObjectId = programMatch[1];
                const actualVerbName = programMatch[2];
                return `${title} (#${actualObjectId}:${actualVerbName})`;
            }
        }
        return title;
    }, [title, uploadAction]);

    // Reset content when initial content changes
    useEffect(() => {
        setContent(initialContent);
        setErrors([]);
        setCompileSuccess(false);
        if (editorRef.current) {
            const model = editorRef.current.getModel();
            if (model) {
                monaco.editor.setModelMarkers(model, "moo-compiler", []);
            }
        }
        if (errorDecorationsRef.current) {
            errorDecorationsRef.current.clear();
        }
    }, [initialContent]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (completionProviderRef.current) {
                completionProviderRef.current.dispose();
                completionProviderRef.current = null;
            }
            if (editorRef.current) {
                editorRef.current.dispose();
            }
        };
    }, []);

    // Verb metadata editing handlers
    const handleTogglePermission = (perm: "readable" | "writable" | "executable" | "debug") => {
        setEditPermissions((prev) => ({
            ...prev,
            [perm]: !prev[perm],
        }));
    };

    const handleSaveMetadata = async () => {
        if (!normalizeObjectInput) {
            console.error("Cannot save metadata: normalizeObjectInput function not provided");
            return;
        }

        setIsSavingMetadata(true);
        setMetadataSaveSuccess(false);

        try {
            const permsStr = `${editPermissions.readable ? "r" : ""}${editPermissions.writable ? "w" : ""}${
                editPermissions.executable ? "x" : ""
            }${editPermissions.debug ? "d" : ""}`;

            // Use the provided utility to normalize object references
            const objExpr = normalizeObjectInput(objectCurie);
            const ownerExpr = normalizeObjectInput(editOwnerValue);
            const normalizedVerbNames = editVerbNames.trim();
            const normalizedArgspec = {
                dobj: editArgspec.dobj.trim(),
                prep: editArgspec.prep.trim(),
                iobj: editArgspec.iobj.trim(),
            };

            if (!objExpr || !ownerExpr) {
                throw new Error("Invalid object reference");
            }
            if (!normalizedVerbNames) {
                throw new Error("Verb names cannot be empty");
            }
            if (!normalizedArgspec.dobj || !normalizedArgspec.prep || !normalizedArgspec.iobj) {
                throw new Error("Argspec values cannot be empty");
            }

            const escapeMooString = (value: string) => value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
            const statements: string[] = [];
            const hasInfoChanges = (owner ? `#${owner}` : "") !== editOwnerValue
                || (verbNames?.trim() || verbName) !== normalizedVerbNames
                || permissions?.readable !== editPermissions.readable
                || permissions?.writable !== editPermissions.writable
                || permissions?.executable !== editPermissions.executable
                || permissions?.debug !== editPermissions.debug;
            const hasArgspecChanges = argspec
                ? (argspec.dobj !== normalizedArgspec.dobj
                    || argspec.prep !== normalizedArgspec.prep
                    || argspec.iobj !== normalizedArgspec.iobj)
                : true;

            if (hasInfoChanges) {
                statements.push(
                    `set_verb_info(${objExpr}, "${verbName}", {${ownerExpr}, "${permsStr}", "${
                        escapeMooString(normalizedVerbNames)
                    }"})`,
                );
            }
            if (hasArgspecChanges) {
                statements.push(
                    `set_verb_args(${objExpr}, "${verbName}", "${escapeMooString(normalizedArgspec.dobj)}", `
                        + `"${escapeMooString(normalizedArgspec.prep)}", "${escapeMooString(normalizedArgspec.iobj)}")`,
                );
            }

            if (statements.length === 0) {
                setIsSavingMetadata(false);
                return;
            }

            const expr = `${statements.join("; ")}; return 1;`;

            console.debug("Evaluating set_verb_info expression:", expr);
            await performEvalFlatBuffer(authToken, expr);

            setIsSavingMetadata(false);
            setIsEditingOwner(false);
            setMetadataSaveSuccess(true);

            // Clear success message after 2 seconds
            setTimeout(() => {
                setMetadataSaveSuccess(false);
            }, 2000);

            // Notify parent to refresh
            if (onSave) {
                onSave();
            }
        } catch (err) {
            console.error("Failed to save verb metadata:", err);
            setIsSavingMetadata(false);
        }
    };

    // Track if metadata has changed
    const hasMetadataChanges = isEditingOwner
        || (owner ? `#${owner}` : "") !== editOwnerValue
        || (verbNames?.trim() || verbName) !== editVerbNames.trim()
        || (argspec
            ? (argspec.dobj !== editArgspec.dobj.trim()
                || argspec.prep !== editArgspec.prep.trim()
                || argspec.iobj !== editArgspec.iobj.trim())
            : true)
        || permissions?.readable !== editPermissions.readable
        || permissions?.writable !== editPermissions.writable
        || permissions?.executable !== editPermissions.executable
        || permissions?.debug !== editPermissions.debug;

    const dobjOptions = ["none", "any", "this"];
    const iobjOptions = ["none", "any", "this"];
    const prepOptions = [
        "none",
        "any",
        "with",
        "at",
        "in-front-of",
        "in",
        "on",
        "from",
        "over",
        "through",
        "under",
        "behind",
        "beside",
        "for",
        "is",
        "as",
        "off",
        "named",
    ];

    useEffect(() => {
        if (editorRef.current) {
            editorRef.current.updateOptions({ fontSize });
        }
    }, [fontSize]);

    // Save word wrap preference and update editor
    useEffect(() => {
        if (editorRef.current) {
            editorRef.current.updateOptions({ wordWrap });
        }
    }, [wordWrap]);

    // Save minimap preference and update editor
    useEffect(() => {
        if (editorRef.current) {
            editorRef.current.updateOptions({ minimap: { enabled: minimapEnabled } });
        }
    }, [minimapEnabled]);

    useEffect(() => {
        monaco.editor.setTheme(monacoTheme);
    }, [monacoTheme]);

    // Configure MOO language for Monaco
    const handleEditorWillMount = useCallback((monaco: Monaco) => {
        // Register MOO language support
        registerMooLanguage(monaco);
    }, []);

    const handleEditorDidMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monacoInstance: Monaco) => {
        editorRef.current = editor;

        // Create custom decoration collection for more visible error highlighting
        errorDecorationsRef.current = editor.createDecorationsCollection();

        // Add CSS for highly visible error decorations
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

        monacoInstance.editor.setTheme(monacoTheme);

        // Dispose old completion provider if it exists
        if (completionProviderRef.current) {
            completionProviderRef.current.dispose();
        }

        // Register MOO completion provider
        completionProviderRef.current = registerMooCompletionProvider(
            monacoInstance,
            authToken,
            objectCurie,
            uploadAction,
        );

        // Focus the editor
        editor.focus();

        // Force layout update to prevent artifacts
        setTimeout(() => {
            editor.layout();
        }, 100);
        editor.updateOptions({ fontSize });
    }, [authToken, fontSize, monacoTheme, objectCurie, uploadAction]);

    const handleEditorChange = useCallback((value: string | undefined) => {
        setContent(value || "");
    }, []);

    const compileVerb = useCallback(async () => {
        if (isCompiling) return;

        setIsCompiling(true);
        setErrors([]);
        setCompileSuccess(false);

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
                const { compileVerbFlatBuffer } = await import("../lib/rpc-fb.js");
                const { unionToCompileErrorUnion } = await import(
                    "../generated/moor-common/compile-error-union.js"
                );
                const { ParseError } = await import("../generated/moor-common/parse-error.js");
                const result = await compileVerbFlatBuffer(authToken, objectCurie, verbName, content);

                // Check for compilation errors
                if (!result.success) {
                    const compilationErrors: CompileError[] = [];

                    // Handle string errors (non-compilation errors)
                    if (typeof result.error === "string") {
                        compilationErrors.push({
                            type: "other",
                            message: result.error,
                        });
                    } else {
                        // Parse FlatBuffer CompileError
                        const compileError = result.error;
                        const errorType = compileError.errorType();

                        // Get the actual error object from the union
                        const accessor: Parameters<typeof unionToCompileErrorUnion>[1] = (obj) =>
                            compileError.error(obj);
                        const errorObj = unionToCompileErrorUnion(errorType, accessor);

                        if (errorObj instanceof ParseError) {
                            const position = errorObj.errorPosition();
                            const line = position ? Number(position.line()) : undefined;
                            const column = position ? Number(position.col()) : undefined;
                            const messageRaw = errorObj.message();
                            const message = typeof messageRaw === "string" ? messageRaw : "Parse error";

                            const contextLineRaw = errorObj.context();
                            const contextLine = typeof contextLineRaw === "string" ? contextLineRaw : undefined;

                            const textDecoder = typeof TextDecoder !== "undefined" ? new TextDecoder() : null;
                            const decodeBytes = (bytes: Uint8Array): string => {
                                if (textDecoder) {
                                    return textDecoder.decode(bytes);
                                }
                                let out = "";
                                for (let i = 0; i < bytes.length; i++) {
                                    out += String.fromCharCode(bytes[i]);
                                }
                                return out;
                            };

                            const expectedTokens: string[] = [];
                            const expectedTokensLength = errorObj.expectedTokensLength();
                            for (let i = 0; i < expectedTokensLength; i++) {
                                const token = errorObj.expectedTokens(i);
                                if (typeof token === "string") {
                                    expectedTokens.push(token);
                                } else if (token !== null && typeof token !== "string") {
                                    expectedTokens.push(decodeBytes(token as Uint8Array));
                                }
                            }

                            const notes: string[] = [];
                            const notesLength = errorObj.notesLength();
                            for (let i = 0; i < notesLength; i++) {
                                const note = errorObj.notes(i);
                                if (typeof note === "string") {
                                    notes.push(note);
                                } else if (note !== null && typeof note !== "string") {
                                    notes.push(decodeBytes(note as Uint8Array));
                                }
                            }

                            const span = errorObj.hasSpan()
                                ? {
                                    start: Number(errorObj.spanStart()),
                                    end: Number(errorObj.spanEnd()),
                                }
                                : undefined;

                            const endLine = errorObj.hasEnd()
                                ? Number(errorObj.endLine())
                                : undefined;
                            const endColumn = errorObj.hasEnd()
                                ? Number(errorObj.endCol())
                                : undefined;

                            compilationErrors.push({
                                type: "parse",
                                message,
                                line,
                                column,
                                endLine,
                                endColumn,
                                span,
                                contextLine,
                                expectedTokens: expectedTokens.length ? expectedTokens : undefined,
                                notes: notes.length ? notes : undefined,
                            });
                        } else {
                            // Handle other error types - just use the toString for now
                            compilationErrors.push({
                                type: "other",
                                message: compileError.toString() || "Compilation error",
                            });
                        }
                    }

                    setErrors(compilationErrors);

                    // Set Monaco error markers
                    if (editorRef.current) {
                        const model = editorRef.current.getModel();
                        if (model) {
                            const clampLine = (line: number): number =>
                                Math.min(Math.max(line, 1), model.getLineCount());
                            const clampColumn = (line: number, column: number): number => {
                                const maxColumn = model.getLineMaxColumn(line);
                                return Math.min(Math.max(column, 1), maxColumn);
                            };
                            const toHoverText = (error: CompileError): string => formatError(error);

                            const modelLength = model.getValueLength();
                            const computeRange = (error: CompileError): monaco.Range => {
                                if (
                                    error.type === "parse"
                                    && error.span
                                    && error.span.end > error.span.start
                                ) {
                                    const startOffset = Math.max(
                                        0,
                                        Math.min(error.span.start, modelLength),
                                    );
                                    const rawEnd = Math.max(
                                        error.span.end,
                                        error.span.start + 1,
                                    );
                                    const endOffset = Math.max(
                                        startOffset + 1,
                                        Math.min(rawEnd, modelLength),
                                    );
                                    const startPos = model.getPositionAt(startOffset);
                                    const endPos = model.getPositionAt(endOffset);
                                    return new monaco.Range(
                                        startPos.lineNumber,
                                        startPos.column,
                                        endPos.lineNumber,
                                        endPos.column,
                                    );
                                }

                                const startLine = clampLine(error.line ?? 1);
                                const startColumn = clampColumn(startLine, error.column ?? 1);
                                const endLine = clampLine(error.endLine ?? startLine);
                                let endColumn = clampColumn(
                                    endLine,
                                    error.endColumn ?? startColumn,
                                );

                                if (
                                    error.type === "parse"
                                    && !error.endColumn
                                    && typeof error.line === "number"
                                    && (typeof error.endLine !== "number"
                                        || error.endLine === error.line)
                                ) {
                                    const lineText = model.getLineContent(startLine);
                                    const wordEnd = lineText.indexOf(" ", startColumn - 1);
                                    const fallback = Math.max(startColumn + 1, startColumn + 5);
                                    endColumn = wordEnd !== -1
                                        ? Math.max(startColumn + 1, wordEnd + 1)
                                        : Math.max(
                                            fallback,
                                            model.getLineMaxColumn(startLine),
                                        );
                                }

                                if (
                                    endLine === startLine
                                    && endColumn <= startColumn
                                ) {
                                    endColumn = Math.min(
                                        startColumn + 1,
                                        model.getLineMaxColumn(startLine),
                                    );
                                }

                                return new monaco.Range(
                                    startLine,
                                    startColumn,
                                    endLine,
                                    endColumn,
                                );
                            };

                            const markers = compilationErrors.map(error => {
                                const range = computeRange(error);
                                return {
                                    severity: monaco.MarkerSeverity.Error,
                                    message: toHoverText(error),
                                    startLineNumber: range.startLineNumber,
                                    startColumn: range.startColumn,
                                    endLineNumber: range.endLineNumber,
                                    endColumn: range.endColumn,
                                };
                            });
                            monaco.editor.setModelMarkers(model, "moo-compiler", markers);

                            // Add more visible decorations
                            if (errorDecorationsRef.current) {
                                const decorations = compilationErrors.map(error => {
                                    const range = computeRange(error);
                                    const hoverText = toHoverText(error);
                                    return {
                                        range,
                                        options: {
                                            className: "moo-error-decoration",
                                            inlineClassName: "moo-error-inline",
                                            hoverMessage: { value: hoverText },
                                            overviewRuler: {
                                                color: "#ff0000",
                                                position: monaco.editor.OverviewRulerLane.Right,
                                            },
                                        },
                                    };
                                });
                                errorDecorationsRef.current.set(decorations);
                            }
                        }
                    }
                } else {
                    // Successful compilation
                    setErrors([]);
                    setCompileSuccess(true);

                    // Auto-hide success message after 3 seconds
                    setTimeout(() => {
                        setCompileSuccess(false);
                    }, 3000);

                    // Clear Monaco error markers and decorations
                    if (editorRef.current) {
                        const model = editorRef.current.getModel();
                        if (model) {
                            monaco.editor.setModelMarkers(model, "moo-compiler", []);
                        }
                        if (errorDecorationsRef.current) {
                            errorDecorationsRef.current.clear();
                        }
                    }
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
        if (error.type === "parse") {
            const segments: string[] = [];
            const locationBits: string[] = [];
            if (typeof error.line === "number") {
                locationBits.push(`line ${error.line}`);
            }
            if (typeof error.column === "number") {
                locationBits.push(`column ${error.column}`);
            }
            const locationPrefix = locationBits.length > 0 ? `At ${locationBits.join(", ")}` : "Parse error";
            segments.push(`${locationPrefix}: ${error.message}`);

            if (error.contextLine) {
                segments.push(`  ${error.contextLine.trimEnd()}`);
            }
            if (error.expectedTokens && error.expectedTokens.length > 0) {
                segments.push(`  Expected ${error.expectedTokens.join(", ")}`);
            }
            if (error.notes && error.notes.length > 0) {
                for (const note of error.notes) {
                    segments.push(`  Note: ${note}`);
                }
            }

            return segments.join("\n");
        }
        return error.message;
    };

    // Track which errors are expanded
    const [expandedErrors, setExpandedErrors] = useState<Set<number>>(new Set());

    const toggleErrorExpanded = (index: number) => {
        setExpandedErrors(prev => {
            const next = new Set(prev);
            if (next.has(index)) {
                next.delete(index);
            } else {
                next.add(index);
            }
            return next;
        });
    };

    // Reset expanded state when errors change
    useEffect(() => {
        setExpandedErrors(new Set());
    }, [errors]);

    // Track if content has changed from original
    const hasUnsavedChanges = content !== initialContent;

    // Title bar component that uses the drag hook (must be inside EditorWindow)
    const TitleBar: React.FC = () => {
        const titleBarDragProps = useTitleBarDrag();

        return (
            <div
                {...titleBarDragProps}
                className="editor-title-bar"
                style={{
                    ...titleBarDragProps.style,
                    borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                }}
            >
                <h3
                    id="verb-editor-title"
                    className="editor-title"
                >
                    <span className="editor-title-label">
                        Verb editor{hasUnsavedChanges && (
                            <span
                                style={{ color: "var(--color-text-secondary)", marginLeft: "4px", fontSize: "0.8em" }}
                            >
                                ●
                            </span>
                        )}
                    </span>
                    <span className="editor-title-path">
                        {enhancedTitle}
                    </span>
                </h3>
                <div className="editor-toolbar">
                    {/* Navigation arrows for multiple editors (only in split/docked mode) */}
                    {splitMode && editorCount > 1 && onPreviousEditor && onNextEditor && (
                        <div className="editor-nav-controls">
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    onPreviousEditor();
                                }}
                                aria-label="Previous editor"
                                title="Previous editor"
                                className="editor-nav-button"
                            >
                                ◀
                            </button>
                            <span className="editor-nav-indicator">
                                {currentEditorIndex + 1}/{editorCount}
                            </span>
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    onNextEditor();
                                }}
                                aria-label="Next editor"
                                title="Next editor"
                                className="editor-nav-button"
                            >
                                ▶
                            </button>
                        </div>
                    )}
                    {/* Remove button - only shown if onDelete handler provided */}
                    {onDelete && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                onDelete();
                            }}
                            aria-label="Remove verb"
                            title="Remove verb"
                            className="btn btn-warning btn-sm"
                        >
                            Remove
                        </button>
                    )}
                    <div
                        className="font-size-control"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <button
                            onClick={() => setFontSize(prev => Math.max(MIN_FONT_SIZE, prev - 1))}
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
                            onClick={() => setFontSize(prev => Math.min(MAX_FONT_SIZE, prev + 1))}
                            aria-label="Increase editor font size"
                            className="font-size-button"
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>
                    {/* Word wrap toggle button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            setWordWrap(prev => prev === "on" ? "off" : "on");
                        }}
                        aria-label={wordWrap === "on" ? "Disable word wrap" : "Enable word wrap"}
                        title={wordWrap === "on" ? "Disable word wrap" : "Enable word wrap"}
                        className="btn btn-secondary btn-sm"
                        style={{ minWidth: "auto", padding: "0.25em 0.5em" }}
                    >
                        {wordWrap === "on" ? "↩" : "→"}
                    </button>
                    {/* Minimap toggle button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            setMinimapEnabled(prev => !prev);
                        }}
                        aria-label={minimapEnabled ? "Hide minimap" : "Show minimap"}
                        title={minimapEnabled ? "Hide minimap" : "Show minimap"}
                        className="btn btn-secondary btn-sm"
                        style={{ minWidth: "auto", padding: "0.25em 0.5em" }}
                    >
                        {minimapEnabled ? "🗺" : "▯"}
                    </button>
                    {/* Compile button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            compileVerb();
                        }}
                        disabled={isCompiling}
                        aria-label="Compile verb"
                        title="Compile verb"
                        className="btn btn-primary btn-sm"
                    >
                        {isCompiling ? "⏳" : "▶"}
                    </button>

                    {/* Split/Float toggle button - only on non-touch devices */}
                    {!isTouchDevice && onToggleSplitMode && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation(); // Prevent drag handler from firing
                                onToggleSplitMode();
                            }}
                            aria-label={isInSplitMode ? "Open in separate window" : "Dock to split view"}
                            title={isInSplitMode ? "Open in separate window" : "Dock to split view"}
                            className="editor-btn-toggle-split"
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}
                    {!splitMode && (
                        <button
                            onClick={onClose}
                            aria-label="Close verb editor"
                            className="editor-btn-close"
                        >
                            <span aria-hidden="true">×</span>
                        </button>
                    )}
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
            ariaLabel={`Verb editor for ${enhancedTitle}`}
            className="editor_container"
        >
            <TitleBar />

            {/* Error panel */}
            {errors.length > 0 && (
                <div className="verb-compile-errors">
                    <div className="verb-compile-errors-list">
                        {errors.map((error, index) => {
                            const isExpanded = expandedErrors.has(index);

                            return (
                                <div key={`${error.type}-${index}`} className="verb-compile-error">
                                    {/* Main error message */}
                                    <div className="verb-compile-error-content">
                                        <div className="verb-compile-error-message">
                                            {error.type === "parse" && typeof error.line === "number" && (
                                                <span className="verb-compile-error-location">
                                                    Line {error.line}
                                                    {typeof error.column === "number" && `, col ${error.column}`}:{" "}
                                                </span>
                                            )}
                                            {error.message}
                                        </div>

                                        {/* Details section (expected tokens) */}
                                        {error.type === "parse" && error.expectedTokens
                                            && error.expectedTokens.length > 0 && (
                                            <div className="verb-compile-error-details">
                                                <button
                                                    onClick={() => toggleErrorExpanded(index)}
                                                    className="verb-compile-error-toggle"
                                                >
                                                    {isExpanded ? "▼" : "▶"} {isExpanded ? "Hide" : "Show"} details
                                                </button>

                                                {isExpanded && (
                                                    <div className="verb-compile-error-expanded">
                                                        <div className="verb-compile-error-expanded-title">
                                                            Expected:
                                                        </div>
                                                        <div className="verb-compile-error-tokens">
                                                            {error.expectedTokens.join(", ")}
                                                        </div>
                                                    </div>
                                                )}
                                            </div>
                                        )}

                                        {/* Hints section (notes) */}
                                        {error.type === "parse" && error.notes && error.notes.length > 0 && (
                                            <div className="verb-compile-error-details">
                                                <button
                                                    onClick={() => toggleErrorExpanded(index + 1000)}
                                                    className="verb-compile-error-toggle hints"
                                                >
                                                    {expandedErrors.has(index + 1000) ? "▼" : "▶"}{" "}
                                                    {expandedErrors.has(index + 1000) ? "Hide" : "Show"} hints
                                                </button>

                                                {expandedErrors.has(index + 1000) && (
                                                    <div className="verb-compile-error-expanded hints">
                                                        <div className="verb-compile-error-expanded-title hints">
                                                            Hints:
                                                        </div>
                                                        <ul className="verb-compile-error-hints">
                                                            {error.notes.map((note, noteIndex) => (
                                                                <li key={noteIndex} className="verb-compile-error-hint">
                                                                    {note}
                                                                </li>
                                                            ))}
                                                        </ul>
                                                    </div>
                                                )}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}

            {/* Success banner */}
            {compileSuccess && (
                <div className="verb-compile-success">
                    <span className="verb-compile-success-icon">✓</span>
                    <span className="verb-compile-success-text">
                        Verb compiled successfully
                    </span>
                </div>
            )}

            {/* Verb metadata info panel */}
            {(owner || definer || permissions || argspec) && (
                <div className="verb-metadata-panel">
                    {/* Definer - read-only, visually separated */}
                    {definer && (
                        <div className="verb-metadata-item readonly">
                            <span className="verb-metadata-label">
                                Definer:
                            </span>
                            <span className="verb-metadata-value">
                                {(() => {
                                    const dollarName = getDollarName?.(definer);
                                    return dollarName ? `$${dollarName} / #${definer}` : `#${definer}`;
                                })()}
                            </span>
                        </div>
                    )}

                    {/* Separator bar */}
                    {definer && (owner || permissions || argspec) && <div className="verb-metadata-separator" />}

                    {/* Names - editable */}
                    <div className="verb-metadata-item">
                        <span className="verb-metadata-label">
                            Names:
                        </span>
                        <input
                            type="text"
                            value={editVerbNames}
                            onChange={(e) => setEditVerbNames(e.target.value)}
                            className="verb-metadata-input"
                            onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                    handleSaveMetadata();
                                } else if (e.key === "Escape") {
                                    setEditVerbNames(verbNames?.trim() || verbName);
                                }
                            }}
                        />
                    </div>

                    {/* Owner - editable */}
                    {owner && (
                        <div className="verb-metadata-item">
                            <span className="verb-metadata-label">
                                Owner:
                            </span>
                            {isEditingOwner
                                ? (
                                    <input
                                        type="text"
                                        value={editOwnerValue}
                                        onChange={(e) => setEditOwnerValue(e.target.value)}
                                        className="verb-metadata-input"
                                        onKeyDown={(e) => {
                                            if (e.key === "Enter") {
                                                handleSaveMetadata();
                                            } else if (e.key === "Escape") {
                                                setEditOwnerValue(owner ? `#${owner}` : "");
                                                setIsEditingOwner(false);
                                            }
                                        }}
                                        autoFocus
                                    />
                                )
                                : (
                                    <button
                                        onClick={() => setIsEditingOwner(true)}
                                        className="verb-metadata-button"
                                    >
                                        {(() => {
                                            const dollarName = getDollarName?.(owner);
                                            return dollarName ? `$${dollarName} / #${owner}` : `#${owner}`;
                                        })()}
                                    </button>
                                )}
                        </div>
                    )}

                    {/* Permissions - editable */}
                    {permissions && (
                        <div className="verb-metadata-item">
                            <span className="verb-metadata-label">
                                Perms:
                            </span>
                            <div className="verb-metadata-permissions">
                                <label className="verb-metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.readable}
                                        onChange={() => handleTogglePermission("readable")}
                                    />
                                    r
                                </label>
                                <label className="verb-metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.writable}
                                        onChange={() => handleTogglePermission("writable")}
                                    />
                                    w
                                </label>
                                <label className="verb-metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.executable}
                                        onChange={() => handleTogglePermission("executable")}
                                    />
                                    x
                                </label>
                                <label className="verb-metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.debug}
                                        onChange={() => handleTogglePermission("debug")}
                                    />
                                    d
                                </label>
                            </div>
                        </div>
                    )}

                    {/* Argspec - editable via dialog */}
                    {argspec && (
                        <div className="verb-metadata-item">
                            <span className="verb-metadata-label">
                                Argspec:
                            </span>
                            <button
                                type="button"
                                className="verb-metadata-button"
                                onClick={() => {
                                    setArgspecDraft(editArgspec);
                                    setShowArgspecDialog(true);
                                }}
                            >
                                {editArgspec.dobj} {editArgspec.prep} {editArgspec.iobj}
                            </button>
                        </div>
                    )}

                    {/* Save/Cancel buttons for metadata changes */}
                    {(hasMetadataChanges || metadataSaveSuccess) && (
                        <>
                            <button
                                onClick={handleSaveMetadata}
                                disabled={isSavingMetadata || metadataSaveSuccess}
                                style={{
                                    padding: "4px 10px",
                                    borderRadius: "var(--radius-sm)",
                                    border: "none",
                                    backgroundColor: metadataSaveSuccess
                                        ? "var(--color-bg-success, #28a745)"
                                        : "var(--color-button-primary)",
                                    color: "white",
                                    cursor: isSavingMetadata || metadataSaveSuccess ? "not-allowed" : "pointer",
                                    fontSize: "0.85em",
                                    fontWeight: "600",
                                    opacity: isSavingMetadata ? 0.6 : 1,
                                }}
                            >
                                {isSavingMetadata ? "Saving..." : metadataSaveSuccess ? "Saved ✓" : "Save"}
                            </button>
                            {hasMetadataChanges && !metadataSaveSuccess && (
                                <button
                                    onClick={() => {
                                        setEditOwnerValue(owner ? `#${owner}` : "");
                                        setEditVerbNames(verbNames?.trim() || verbName);
                                        const normalizedArgspec = argspec || { dobj: "", prep: "", iobj: "" };
                                        setEditArgspec(normalizedArgspec);
                                        setArgspecDraft(normalizedArgspec);
                                        setEditPermissions(
                                            permissions
                                                || {
                                                    readable: false,
                                                    writable: false,
                                                    executable: false,
                                                    debug: false,
                                                },
                                        );
                                        setIsEditingOwner(false);
                                    }}
                                    disabled={isSavingMetadata}
                                    style={{
                                        padding: "4px 10px",
                                        borderRadius: "var(--radius-sm)",
                                        border: "1px solid var(--color-border-medium)",
                                        backgroundColor: "transparent",
                                        color: "var(--color-text-secondary)",
                                        cursor: isSavingMetadata ? "not-allowed" : "pointer",
                                        fontSize: "0.85em",
                                        fontWeight: "600",
                                    }}
                                >
                                    Cancel
                                </button>
                            )}
                        </>
                    )}
                </div>
            )}

            {showArgspecDialog && (
                <DialogSheet
                    title="Edit Verb Argspec"
                    titleId="edit-verb-argspec"
                    onCancel={() => {
                        setArgspecDraft(editArgspec);
                        setShowArgspecDialog(false);
                    }}
                >
                    <form
                        onSubmit={(event) => {
                            event.preventDefault();
                            setEditArgspec(argspecDraft);
                            setShowArgspecDialog(false);
                        }}
                        className="dialog-sheet-content form-stack"
                    >
                        <div className="form-group">
                            <span className="form-group-label">Verb argument specification</span>
                            <div className="verb-argspec-grid">
                                <label className="verb-argspec-column">
                                    <span className="verb-argspec-label">dobj</span>
                                    <select
                                        value={argspecDraft.dobj}
                                        onChange={(e) => setArgspecDraft(prev => ({ ...prev, dobj: e.target.value }))}
                                        className="verb-argspec-select"
                                    >
                                        {dobjOptions.map((option) => (
                                            <option key={option} value={option}>{option}</option>
                                        ))}
                                    </select>
                                </label>
                                <label className="verb-argspec-column">
                                    <span className="verb-argspec-label">prep</span>
                                    <select
                                        value={argspecDraft.prep}
                                        onChange={(e) => setArgspecDraft(prev => ({ ...prev, prep: e.target.value }))}
                                        className="verb-argspec-select"
                                    >
                                        {prepOptions.map((option) => (
                                            <option key={option} value={option}>{option}</option>
                                        ))}
                                    </select>
                                </label>
                                <label className="verb-argspec-column">
                                    <span className="verb-argspec-label">iobj</span>
                                    <select
                                        value={argspecDraft.iobj}
                                        onChange={(e) => setArgspecDraft(prev => ({ ...prev, iobj: e.target.value }))}
                                        className="verb-argspec-select"
                                    >
                                        {iobjOptions.map((option) => (
                                            <option key={option} value={option}>{option}</option>
                                        ))}
                                    </select>
                                </label>
                            </div>
                        </div>
                        <div className="button-group">
                            <button
                                type="button"
                                onClick={() => {
                                    setArgspecDraft(editArgspec);
                                    setShowArgspecDialog(false);
                                }}
                                className="btn btn-secondary"
                            >
                                Cancel
                            </button>
                            <button type="submit" className="btn btn-primary">
                                Update Argspec
                            </button>
                        </div>
                    </form>
                </DialogSheet>
            )}

            {/* Monaco Editor */}
            <div className="editor-monaco-wrapper">
                <Editor
                    value={content}
                    language="moo"
                    theme="vs-dark"
                    onChange={handleEditorChange}
                    beforeMount={handleEditorWillMount}
                    onMount={handleEditorDidMount}
                    options={{
                        minimap: { enabled: minimapEnabled },
                        fontSize,
                        fontFamily:
                            "\"Comic Mono\", \"JetBrains Mono\", \"Fira Code\", \"Source Code Pro\", Consolas, \"Liberation Mono\", Monaco, Menlo, \"Courier New\", monospace",
                        automaticLayout: true,
                        colorDecorators: true,
                        dragAndDrop: false,
                        emptySelectionClipboard: false,
                        autoClosingDelete: "never",
                        wordWrap,
                        lineNumbers: "on",
                        folding: !isMobile,
                        renderWhitespace: "none", // Hide whitespace rendering completely
                        renderControlCharacters: false, // Hide control characters
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
        </EditorWindow>
    );
};
