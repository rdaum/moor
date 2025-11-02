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
import { useTouchDevice } from "../hooks/useTouchDevice";
import { performEvalFlatBuffer } from "../lib/rpc-fb.js";
import { objToString } from "../lib/var.js";

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
    // Verb metadata
    owner?: string;
    definer?: string;
    permissions?: { readable: boolean; writable: boolean; executable: boolean; debug: boolean };
    argspec?: { dobj: string; prep: string; iobj: string };
    onSave?: () => void; // Callback to refresh verb data after metadata save
    onDelete?: () => void; // Callback to delete verb
    normalizeObjectInput?: (raw: string) => string; // Utility to convert object references to MOO expressions
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
    initialContent,
    authToken,
    uploadAction,
    onSendMessage,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
    owner,
    definer,
    permissions,
    argspec,
    onSave,
    onDelete,
    normalizeObjectInput,
    onPreviousEditor,
    onNextEditor,
    editorCount = 1,
    currentEditorIndex = 0,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const isTouchDevice = useTouchDevice();
    const [content, setContent] = useState(initialContent);
    const [errors, setErrors] = useState<CompileError[]>([]);
    const [isCompiling, setIsCompiling] = useState(false);
    const [compileSuccess, setCompileSuccess] = useState(false);
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
    const errorDecorationsRef = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);
    const completionProviderRef = useRef<monaco.IDisposable | null>(null);
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 24;
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

    // Word wrap state
    const [wordWrap, setWordWrap] = useState<"on" | "off">(() => {
        if (typeof window === "undefined") {
            return isMobile ? "on" : "off";
        }
        const stored = window.localStorage.getItem("moor-editor-wordwrap");
        if (stored === "on" || stored === "off") {
            return stored;
        }
        return isMobile ? "on" : "off";
    });

    // Minimap state
    const [minimapEnabled, setMinimapEnabled] = useState<boolean>(() => {
        if (typeof window === "undefined") {
            return !isMobile;
        }
        const stored = window.localStorage.getItem("moor-editor-minimap");
        if (stored === "true") return true;
        if (stored === "false") return false;
        return !isMobile;
    });

    // Verb metadata editing state
    const [isEditingOwner, setIsEditingOwner] = useState(false);
    const [editOwnerValue, setEditOwnerValue] = useState(owner ? `#${owner}` : "");
    const [editPermissions, setEditPermissions] = useState(
        permissions || { readable: false, writable: false, executable: false, debug: false },
    );
    const [isSavingMetadata, setIsSavingMetadata] = useState(false);
    const [metadataSaveSuccess, setMetadataSaveSuccess] = useState(false);

    // Sync local state when props change, but don't clear the success state
    useEffect(() => {
        setEditOwnerValue(owner ? `#${owner}` : "");
        setEditPermissions(permissions || { readable: false, writable: false, executable: false, debug: false });
        // Don't clear metadataSaveSuccess - let the timeout handle it
    }, [owner, permissions]);

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
            if (editorThemeObserverRef.current) {
                editorThemeObserverRef.current.disconnect();
                editorThemeObserverRef.current = null;
            }
            if (editorThemeListenerRef.current) {
                window.removeEventListener("storage", editorThemeListenerRef.current);
                editorThemeListenerRef.current = null;
            }
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

            if (!objExpr || !ownerExpr) {
                throw new Error("Invalid object reference");
            }

            // Call: set_verb_info(obj, verbname, {owner, "perms", verbname})
            // Note: We keep the same verb name for now (not editing names yet)
            const expr =
                `return set_verb_info(${objExpr}, "${verbName}", {${ownerExpr}, "${permsStr}", "${verbName}"});`;

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
        || permissions?.readable !== editPermissions.readable
        || permissions?.writable !== editPermissions.writable
        || permissions?.executable !== editPermissions.executable
        || permissions?.debug !== editPermissions.debug;

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

    // Save word wrap preference and update editor
    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem("moor-editor-wordwrap", wordWrap);
        }
        if (editorRef.current) {
            editorRef.current.updateOptions({ wordWrap });
        }
    }, [wordWrap]);

    // Save minimap preference and update editor
    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem("moor-editor-minimap", minimapEnabled.toString());
        }
        if (editorRef.current) {
            editorRef.current.updateOptions({ minimap: { enabled: minimapEnabled } });
        }
    }, [minimapEnabled]);

    // Configure MOO language for Monaco
    const handleEditorWillMount = useCallback((monaco: Monaco) => {
        // Register MOO language
        monaco.languages.register({ id: "moo" });

        // Define MOO language tokens
        monaco.languages.setMonarchTokensProvider("moo", {
            tokenizer: {
                root: [
                    // Control flow keywords
                    [
                        /\b(if|elseif|else|endif|while|endwhile|for|endfor|try|except|endtry|finally|fork|endfork|begin|end)\b/,
                        "keyword.control",
                    ],

                    // Flow control
                    [/\b(return|break|continue|pass|raise)\b/, "keyword.control"],

                    // Declaration keywords
                    [/\b(let|const|global|fn|endfn)\b/, "keyword.declaration"],

                    // Special keywords
                    [/\b(any|in)\b/, "keyword.operator"],

                    // Built-in constants
                    [/\b(true|false)\b/, "constant.language"],

                    // Type constants
                    [/\b(INT|NUM|FLOAT|STR|ERR|OBJ|LIST|MAP|BOOL|FLYWEIGHT|SYM)\b/, "type"],

                    // Error constants (without parentheses, to allow string highlighting inside)
                    [/\bE_[A-Z_]+\b/, "constant.other"],

                    // Binary literals (base64-encoded)
                    [/b"[A-Za-z0-9+/=_-]*"/, "string.binary"],

                    // Object references (#123, #-1)
                    [/#-?\d+/, "number.hex"],

                    // System properties and verbs ($property)
                    [/\$[a-zA-Z_][a-zA-Z0-9_]*/, "variable.predefined"],

                    // Try expression start delimiter (backtick)
                    [/`/, { token: "keyword.try", next: "@tryExpression", bracket: "@open" }],

                    // Symbols ('symbol)
                    [/'[a-zA-Z_][a-zA-Z0-9_]*/, "string.key"],

                    // Range end marker ($)
                    [/\$(?=\s*[\]})])/, "constant.numeric"],

                    // Strings
                    [/"([^"\\]|\\.)*$/, "string.invalid"],
                    [/"/, "string", "@string"],

                    // Numbers - floats first to avoid conflicts
                    [/\d*\.\d+([eE][-+]?\d+)?/, "number.float"],
                    [/\d+[eE][-+]?\d+/, "number.float"],
                    [/\d+/, "number"],

                    // Operators - order matters, specific to general
                    [/\.\./, "keyword.operator"], // Range operator
                    [/->/, "keyword.operator"], // Map arrow
                    [/=>/, "keyword.operator"], // Lambda arrow
                    [/>>>/, "operator.bitwise"],
                    [/>>/, "operator.bitwise"],
                    [/<<(?!=)/, "operator.bitwise"],
                    [/&\./, "operator.bitwise"],
                    [/\|\./, "operator.bitwise"],
                    [/\^\./, "operator.bitwise"],
                    [/(==|!=|<=|>=)/, "operator.comparison"],
                    [/(&&|\|\|)/, "operator.logical"],
                    [/[<>]/, "operator.comparison"],
                    [/=/, "operator.assignment"],
                    [/!/, "operator.logical"],
                    [/~/, "operator.bitwise"],
                    [/[+\-*/%^]/, "operator.arithmetic"],
                    [/\?/, "operator.conditional"], // Ternary begin
                    [/\|/, "operator.conditional"], // Ternary separator
                    [/:/, "keyword.operator"], // Verb call
                    [/\./, "operator.accessor"], // Property access
                    [/@/, "keyword.operator"], // Scatter/splat operator

                    // Comments
                    [/\/\*/, "comment", "@comment"],
                    [/\/\/.*$/, "comment"],

                    // Identifiers
                    [/[a-zA-Z_][a-zA-Z0-9_]*/, "identifier"],
                ],

                string: [
                    [/[^\\"]+/, "string"],
                    [/\\./, "string.escape"],
                    [/"/, "string", "@pop"],
                ],

                comment: [
                    [/[^/*]+/, "comment"],
                    [/\*\//, "comment", "@pop"],
                    [/[/*]/, "comment"],
                ],

                tryExpression: [
                    [/'(?![a-zA-Z_])/, { token: "keyword.try", next: "@pop", bracket: "@close" }],
                    [/=>/, "keyword.operator"],
                    [/!/, "keyword.operator"],
                    { include: "@root" },
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
                ["{", "}"], // Lists and blocks
                ["[", "]"], // Maps and indexing
                ["(", ")"], // Function calls and grouping
                ["<", ">"], // Flyweights
            ],
            autoClosingPairs: [
                { open: "{", close: "}" },
                { open: "[", close: "]" },
                { open: "(", close: ")" },
                { open: "<", close: ">" },
                { open: "\"", close: "\"" },
                { open: "`", close: "'" }, // Try expressions
            ],
            surroundingPairs: [
                { open: "{", close: "}" },
                { open: "[", close: "]" },
                { open: "(", close: ")" },
                { open: "<", close: ">" },
                { open: "\"", close: "\"" },
            ],
        });
    }, []);

    const handleEditorDidMount = useCallback((editor: monaco.editor.IStandaloneCodeEditor, monaco: Monaco) => {
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

        // Set Monaco theme to match client theme
        const savedTheme = localStorage.getItem("theme");
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        const isDarkTheme = savedTheme
            ? (savedTheme === "dark" || savedTheme === "crt" || savedTheme === "crt-amber")
            : prefersDark;

        monaco.editor.setTheme(isDarkTheme ? "vs-dark" : "vs");

        // Listen for theme changes
        const handleThemeChange = () => {
            const currentTheme = localStorage.getItem("theme");
            const currentPrefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
            const currentIsDarkTheme = currentTheme
                ? (currentTheme === "dark" || currentTheme === "crt" || currentTheme === "crt-amber")
                : currentPrefersDark;
            monaco.editor.setTheme(currentIsDarkTheme ? "vs-dark" : "vs");
        };

        // Listen for storage changes (theme toggle)
        window.addEventListener("storage", handleThemeChange);
        editorThemeListenerRef.current = handleThemeChange; // ref for later disposal

        // Also listen for changes to the light-theme class on body
        const observer = new MutationObserver((mutations) => {
            mutations.forEach((mutation) => {
                if (mutation.type === "attributes" && mutation.attributeName === "class") {
                    handleThemeChange();
                }
            });
        });
        editorThemeObserverRef.current = observer; // ref for later disposal
        observer.observe(document.body, { attributes: true });

        // Cache for verb/property lookups to avoid repeated API calls
        const completionCache = new Map<
            string,
            { verbs?: any; properties?: any; timestamp: number }
        >();
        const CACHE_TTL = 30000; // 30 seconds

        const getCachedVerbs = async (cacheKey: string, fetchFn: () => Promise<any>) => {
            const cached = completionCache.get(cacheKey);
            if (cached && cached.verbs && Date.now() - cached.timestamp < CACHE_TTL) {
                return cached.verbs;
            }
            const verbs = await fetchFn();
            completionCache.set(cacheKey, { ...cached, verbs, timestamp: Date.now() });
            return verbs;
        };

        const getCachedProperties = async (cacheKey: string, fetchFn: () => Promise<any>) => {
            const cached = completionCache.get(cacheKey);
            if (cached && cached.properties && Date.now() - cached.timestamp < CACHE_TTL) {
                return cached.properties;
            }
            const properties = await fetchFn();
            completionCache.set(cacheKey, { ...cached, properties, timestamp: Date.now() });
            return properties;
        };

        const getCachedBuiltins = async (cacheKey: string, fetchFn: () => Promise<any>) => {
            const cached = completionCache.get(cacheKey);
            if (cached && (cached as any).builtins && Date.now() - cached.timestamp < CACHE_TTL) {
                return (cached as any).builtins;
            }
            const builtins = await fetchFn();
            completionCache.set(cacheKey, { ...cached, builtins, timestamp: Date.now() } as any);
            return builtins;
        };

        // Generic property completion for any object reference
        const addPropertyCompletions = async (
            objectRef: any,
            cacheKey: string,
            contextLabel: string,
            prefix: string,
            startColumn: number,
            position: any,
            suggestions: monaco.languages.CompletionItem[],
        ) => {
            try {
                const propsReply = await getCachedProperties(cacheKey, () => objectRef.getProperties());
                const propsLength = propsReply.propertiesLength();

                for (let i = 0; i < propsLength; i++) {
                    const propInfo = propsReply.properties(i);
                    if (!propInfo) continue;

                    const nameSymbol = propInfo.name();
                    const propName = nameSymbol?.value();
                    if (!propName || !propName.startsWith(prefix)) continue;

                    const definerId = objToString(propInfo.definer());
                    const ownerId = objToString(propInfo.owner());

                    suggestions.push({
                        label: {
                            label: propName,
                            detail: definerId ? ` #${definerId}` : "",
                        },
                        kind: monaco.languages.CompletionItemKind.Property,
                        insertText: propName,
                        sortText: i.toString().padStart(4, "0"),
                        documentation: `Property on ${contextLabel} (defined in #${definerId || "?"}, owner: #${
                            ownerId || "?"
                        }, ${propInfo.r() ? "readable" : "not readable"}, ${propInfo.w() ? "writable" : "read-only"})`,
                        range: {
                            startLineNumber: position.lineNumber,
                            endLineNumber: position.lineNumber,
                            startColumn,
                            endColumn: position.column,
                        },
                    });
                }
            } catch (error) {
                console.warn(`Failed to fetch properties for ${contextLabel}:`, error);
            }
        };

        // Generic verb completion for any object reference
        const addVerbCompletions = async (
            objectRef: any,
            cacheKey: string,
            contextLabel: string,
            prefix: string,
            startColumn: number,
            position: any,
            suggestions: monaco.languages.CompletionItem[],
        ) => {
            try {
                const verbsReply = await getCachedVerbs(cacheKey, () => objectRef.getVerbs());
                const verbsLength = verbsReply.verbsLength();
                let sortIndex = 0;

                for (let i = 0; i < verbsLength; i++) {
                    const verbInfo = verbsReply.verbs(i);
                    if (!verbInfo) continue;

                    const locationId = objToString(verbInfo.location());
                    const ownerId = objToString(verbInfo.owner());
                    const namesLength = verbInfo.namesLength();

                    for (let j = 0; j < namesLength; j++) {
                        const nameSymbol = verbInfo.names(j);
                        const verbName = nameSymbol?.value();
                        if (!verbName || !verbName.startsWith(prefix)) continue;

                        suggestions.push({
                            label: {
                                label: verbName,
                                detail: locationId ? ` #${locationId}` : "",
                            },
                            kind: monaco.languages.CompletionItemKind.Method,
                            insertText: verbName,
                            sortText: sortIndex.toString().padStart(4, "0"),
                            documentation: `Verb on ${contextLabel} (defined in #${locationId || "?"}, owner: #${
                                ownerId || "?"
                            }, ${verbInfo.r() ? "readable" : "not readable"}, ${
                                verbInfo.x() ? "executable" : "not executable"
                            })`,
                            range: {
                                startLineNumber: position.lineNumber,
                                endLineNumber: position.lineNumber,
                                startColumn,
                                endColumn: position.column,
                            },
                        });
                        sortIndex++;
                    }
                }
            } catch (error) {
                console.warn(`Failed to fetch verbs for ${contextLabel}:`, error);
            }
        };

        // Add builtin function completions
        const addBuiltinCompletions = async (
            prefix: string,
            startColumn: number,
            position: any,
            suggestions: monaco.languages.CompletionItem[],
        ) => {
            try {
                const builtins = await getCachedBuiltins("builtins", async () => {
                    // performEvalFlatBuffer returns converted JS objects/arrays
                    const result = await performEvalFlatBuffer(authToken, "return function_info();");
                    // Result is an array of [name, min_args, max_args, [arg_types]]
                    const builtinList = [];
                    if (Array.isArray(result)) {
                        for (const item of result) {
                            if (Array.isArray(item) && item.length >= 4) {
                                const [name, minArgs, maxArgs, argTypes] = item;
                                if (
                                    typeof name === "string" && typeof minArgs === "number"
                                    && typeof maxArgs === "number"
                                ) {
                                    builtinList.push({
                                        name,
                                        minArgs,
                                        maxArgs,
                                        argTypes: Array.isArray(argTypes) ? argTypes : [],
                                    });
                                }
                            }
                        }
                    }
                    return builtinList;
                });

                // Type code to string mapping
                const typeToString = (typeCode: number): string => {
                    switch (typeCode) {
                        case -2:
                            return "num";
                        case -1:
                            return "any";
                        case 0:
                            return "int";
                        case 1:
                            return "obj";
                        case 2:
                            return "str";
                        case 3:
                            return "err";
                        case 4:
                            return "list";
                        case 5:
                            return "clear";
                        case 6:
                            return "none";
                        case 7:
                            return "float";
                        default:
                            return "?";
                    }
                };

                // Add completions for matching builtins
                let sortIndex = 0;
                for (const builtin of builtins) {
                    if (!builtin.name.startsWith(prefix)) continue;

                    // Build arg signature
                    const argSig = builtin.argTypes.length > 0
                        ? builtin.argTypes.map((t: number) => typeToString(t)).join(", ")
                        : "";
                    const argsDesc = builtin.minArgs === builtin.maxArgs
                        ? `${builtin.minArgs} arg${builtin.minArgs === 1 ? "" : "s"}`
                        : builtin.maxArgs === -1
                        ? `${builtin.minArgs}+ args`
                        : `${builtin.minArgs}-${builtin.maxArgs} args`;

                    suggestions.push({
                        label: {
                            label: builtin.name,
                            detail: argSig ? ` (${argSig})` : "",
                        },
                        kind: monaco.languages.CompletionItemKind.Function,
                        insertText: builtin.name,
                        sortText: sortIndex.toString().padStart(4, "0"),
                        documentation: `Builtin function: ${builtin.name}(${argSig}) - ${argsDesc}`,
                        range: {
                            startLineNumber: position.lineNumber,
                            endLineNumber: position.lineNumber,
                            startColumn,
                            endColumn: position.column,
                        },
                    });
                    sortIndex++;
                }
            } catch (error) {
                console.warn("Failed to fetch builtin functions:", error);
            }
        };

        // Dispose old completion provider if it exists
        if (completionProviderRef.current) {
            completionProviderRef.current.dispose();
        }

        // Add completion provider for MOO block structures and smart completions
        completionProviderRef.current = monaco.languages.registerCompletionItemProvider("moo", {
            provideCompletionItems: async (model, position) => {
                const suggestions: monaco.languages.CompletionItem[] = [];
                const lineContent = model.getLineContent(position.lineNumber);
                const beforeCursor = lineContent.substring(0, position.column - 1);

                // Extract actual object ID from uploadAction for "this" completion
                let actualObjectId: number | null = null;
                if (uploadAction) {
                    const programMatch = uploadAction.match(/@program\s+#(\d+):/);
                    if (programMatch) {
                        actualObjectId = parseInt(programMatch[1]);
                    }
                }

                // Check for smart completion patterns
                const thisVerbMatch = beforeCursor.match(/\bthis:(\w*)$/);
                const thisPropMatch = beforeCursor.match(/\bthis\.(\w*)$/);
                const objVerbMatch = beforeCursor.match(/#(-?\d+):(\w*)$/);
                const objPropMatch = beforeCursor.match(/#(-?\d+)\.(\w*)$/);
                const sysVerbMatch = beforeCursor.match(/\$(\w+):(\w*)$/);
                const sysPropMatch = beforeCursor.match(/\$(\w+)\.(\w*)$/);

                // Smart completion for this: verbs
                if (thisVerbMatch) {
                    const { MoorRemoteObject, curieORef } = await import("../lib/rpc");
                    const { oidRef } = await import("../lib/var");
                    const currentObject = actualObjectId
                        ? new MoorRemoteObject(oidRef(actualObjectId), authToken)
                        : new MoorRemoteObject(curieORef(objectCurie), authToken);
                    const cacheKey = actualObjectId ? `#${actualObjectId}:verbs` : `this:verbs`;

                    await addVerbCompletions(
                        currentObject,
                        cacheKey,
                        "this object",
                        thisVerbMatch[1],
                        position.column - thisVerbMatch[1].length,
                        position,
                        suggestions,
                    );
                } // Smart completion for this. properties
                else if (thisPropMatch) {
                    const { MoorRemoteObject, curieORef } = await import("../lib/rpc");
                    const { oidRef } = await import("../lib/var");
                    const currentObject = actualObjectId
                        ? new MoorRemoteObject(oidRef(actualObjectId), authToken)
                        : new MoorRemoteObject(curieORef(objectCurie), authToken);
                    const cacheKey = actualObjectId ? `#${actualObjectId}:properties` : `this:properties`;

                    await addPropertyCompletions(
                        currentObject,
                        cacheKey,
                        "this object",
                        thisPropMatch[1],
                        position.column - thisPropMatch[1].length,
                        position,
                        suggestions,
                    );
                } // Smart completion for #123: object verb calls
                else if (objVerbMatch) {
                    const { MoorRemoteObject } = await import("../lib/rpc");
                    const { oidRef } = await import("../lib/var");
                    const objectId = parseInt(objVerbMatch[1]);
                    const targetObject = new MoorRemoteObject(oidRef(objectId), authToken);

                    await addVerbCompletions(
                        targetObject,
                        `#${objectId}:verbs`,
                        `object #${objectId}`,
                        objVerbMatch[2],
                        position.column - objVerbMatch[2].length,
                        position,
                        suggestions,
                    );
                } // Smart completion for #123. object property access
                else if (objPropMatch) {
                    const { MoorRemoteObject } = await import("../lib/rpc");
                    const { oidRef } = await import("../lib/var");
                    const objectId = parseInt(objPropMatch[1]);
                    const targetObject = new MoorRemoteObject(oidRef(objectId), authToken);

                    await addPropertyCompletions(
                        targetObject,
                        `#${objectId}:properties`,
                        `object #${objectId}`,
                        objPropMatch[2],
                        position.column - objPropMatch[2].length,
                        position,
                        suggestions,
                    );
                } // Smart completion for $thing. property access
                else if (sysPropMatch) {
                    const { MoorRemoteObject } = await import("../lib/rpc");
                    const { sysobjRef } = await import("../lib/var");
                    const targetObject = new MoorRemoteObject(sysobjRef([sysPropMatch[1]]), authToken);

                    await addPropertyCompletions(
                        targetObject,
                        `$${sysPropMatch[1]}:properties`,
                        `$${sysPropMatch[1]}`,
                        sysPropMatch[2],
                        position.column - sysPropMatch[2].length,
                        position,
                        suggestions,
                    );
                } // Smart completion for $thing: verb calls
                else if (sysVerbMatch) {
                    const { MoorRemoteObject } = await import("../lib/rpc");
                    const { sysobjRef } = await import("../lib/var");
                    const targetObject = new MoorRemoteObject(sysobjRef([sysVerbMatch[1]]), authToken);

                    await addVerbCompletions(
                        targetObject,
                        `$${sysVerbMatch[1]}:verbs`,
                        `$${sysVerbMatch[1]}`,
                        sysVerbMatch[2],
                        position.column - sysVerbMatch[2].length,
                        position,
                        suggestions,
                    );
                } else {
                    const letSnippetMatch = beforeCursor.match(/\blet(?:\s+([a-zA-Z_][a-zA-Z0-9_]*))?\s*$/);
                    if (letSnippetMatch) {
                        const replaceLength = letSnippetMatch[0].length;
                        const startColumn = Math.max(1, position.column - replaceLength);
                        const variableName = letSnippetMatch[1] || "name";
                        const letRange = {
                            startLineNumber: position.lineNumber,
                            endLineNumber: position.lineNumber,
                            startColumn,
                            endColumn: position.column,
                        };

                        const letSnippets = [
                            {
                                label: "let assignment",
                                insertText: `let \${1:${variableName}} = \${2:value};`,
                                documentation: "Bind a local variable to the result of an expression.",
                                detail: "Bind a single variable",
                                sortText: "00",
                            },
                            {
                                label: "let scatter assignment",
                                insertText:
                                    `let {\${1:${variableName}}, \${2:?optional = default}, \${3:@rest}} = \${4:expr};`,
                                documentation:
                                    "Unpack a list (or map) into variables, with optional and rest bindings.",
                                detail: "Unpack a collection",
                                sortText: "10",
                            },
                        ];

                        for (const snippet of letSnippets) {
                            suggestions.push({
                                label: snippet.label,
                                kind: monaco.languages.CompletionItemKind.Snippet,
                                insertText: snippet.insertText,
                                insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                                documentation: snippet.documentation,
                                detail: snippet.detail,
                                range: letRange,
                                sortText: snippet.sortText,
                                filterText: "let",
                            });
                        }

                        return { suggestions };
                    }

                    const constSnippetMatch = beforeCursor.match(/\bconst(?:\s+([a-zA-Z_][a-zA-Z0-9_]*))?\s*$/);
                    if (constSnippetMatch) {
                        const replaceLength = constSnippetMatch[0].length;
                        const startColumn = Math.max(1, position.column - replaceLength);
                        const constantName = constSnippetMatch[1] || "NAME";
                        const constRange = {
                            startLineNumber: position.lineNumber,
                            endLineNumber: position.lineNumber,
                            startColumn,
                            endColumn: position.column,
                        };

                        const constSnippets = [
                            {
                                label: "const assignment",
                                insertText: `const \${1:${constantName}} = \${2:value};`,
                                documentation: "Define a constant value within the current scope.",
                                detail: "Define a constant",
                                sortText: "00",
                            },
                            {
                                label: "const scatter assignment",
                                insertText:
                                    `const {\${1:${constantName}}, \${2:?optional = default}, \${3:@rest}} = \${4:expr};`,
                                documentation: "Unpack values into constant bindings; the rest binding remains a list.",
                                detail: "Unpack to constants",
                                sortText: "10",
                            },
                        ];

                        for (const snippet of constSnippets) {
                            suggestions.push({
                                label: snippet.label,
                                kind: monaco.languages.CompletionItemKind.Snippet,
                                insertText: snippet.insertText,
                                insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                                documentation: snippet.documentation,
                                detail: snippet.detail,
                                range: constRange,
                                sortText: snippet.sortText,
                                filterText: "const",
                            });
                        }

                        return { suggestions };
                    }

                    // Check for builtin function completions
                    // Trigger when typing a word that's not after : or .
                    const word = model.getWordUntilPosition(position);
                    const beforeWord = lineContent.substring(0, word.startColumn - 1);
                    const isAfterObjectOp = beforeWord.match(/[:\.]$/);

                    if (!isAfterObjectOp && word.word.length > 0) {
                        await addBuiltinCompletions(word.word, word.startColumn, position, suggestions);
                    }
                } // If no smart completions matched, show block templates
                if (suggestions.length === 0) {
                    const word = model.getWordUntilPosition(position);
                    const defaultRange = {
                        startLineNumber: position.lineNumber,
                        endLineNumber: position.lineNumber,
                        startColumn: word.startColumn,
                        endColumn: word.endColumn,
                    };

                    const blockSnippets: Array<{
                        label: string;
                        insertText: string;
                        documentation: string;
                        sortText: string;
                        detailText?: string;
                        filterText?: string;
                    }> = [
                        {
                            label: "begin/end block",
                            insertText: "begin\n\t${1}\nend",
                            documentation: "Wrap statements in a begin...end block to group work or scope locals.",
                            sortText: "00",
                            detailText: "Group statements",
                            filterText: "begin",
                        },
                        {
                            label: "if/endif conditional",
                            insertText: "if (${1:condition})\n\t${2}\nendif",
                            documentation: "Conditional block; fill in optional elseif/else by hand as needed.",
                            sortText: "10",
                            detailText: "Branch on a condition",
                            filterText: "if",
                        },
                        {
                            label: "while loop",
                            insertText: "while (${1:condition})\n\t${2}\nendwhile",
                            documentation: "Loop while the condition stays true.",
                            sortText: "20",
                            detailText: "Repeat while true",
                            filterText: "while",
                        },
                        {
                            label: "for ... in (collection)",
                            insertText: "for ${1:item} in (${2:collection})\n\t${3}\nendfor",
                            documentation: "Iterate values (and optional index/key) from a collection.",
                            sortText: "30",
                            detailText: "Loop over a collection",
                            filterText: "for",
                        },
                        {
                            label: "for ... in [start..end]",
                            insertText: "for ${1:i} in [${2:start}..${3:end}]\n\t${4}\nendfor",
                            documentation: "Iterate across a numeric range inclusive of both ends.",
                            sortText: "40",
                            detailText: "Loop over a range",
                            filterText: "for",
                        },
                        {
                            label: "try/except block",
                            insertText: "try\n\t${1}\nexcept (${2:E_ANY})\n\t${3}\nendtry",
                            documentation: "Wrap statements and handle errors in one or more except clauses.",
                            sortText: "50",
                            detailText: "Catch errors",
                            filterText: "try",
                        },
                        {
                            label: "try expression",
                            insertText: "` ${1:dodgy()} ! ${2:any} => ${3:fallback()}'",
                            documentation: "Inline try expression of the form ` expr ! codes => handler '.",
                            sortText: "60",
                            detailText: "Inline error handling",
                            filterText: "try",
                        },
                        {
                            label: "fork/endfork block",
                            insertText: "fork (${1:0})\n\t${2}\nendfork",
                            documentation: "Run statements in a forked task after an optional delay.",
                            sortText: "70",
                            detailText: "Spawn task",
                            filterText: "fork",
                        },
                        {
                            label: "fn/endfn local function",
                            insertText: "fn ${1:name}(${2:args})\n\t${3}\nendfn",
                            documentation: "Define a local function; returns the function value when used in an expr.",
                            sortText: "80",
                            detailText: "Define helper function",
                            filterText: "fn",
                        },
                    ];

                    // Keyword completions for flow control
                    const keywordCompletions: Array<{
                        label: string;
                        insertText: string;
                        documentation: string;
                        sortText: string;
                    }> = [
                        {
                            label: "return",
                            insertText: "return ${1:value};",
                            documentation: "Return a value from the current verb or function.",
                            sortText: "90",
                        },
                        {
                            label: "raise",
                            insertText: "raise(${1:E_INVARG}(${2:\"message\"}));",
                            documentation: "Raise an error with an optional message.",
                            sortText: "91",
                        },
                        {
                            label: "break",
                            insertText: "break;",
                            documentation: "Exit the current loop immediately.",
                            sortText: "92",
                        },
                        {
                            label: "continue",
                            insertText: "continue;",
                            documentation: "Skip to the next iteration of the current loop.",
                            sortText: "93",
                        },
                        {
                            label: "pass",
                            insertText: "pass;",
                            documentation: "No-op statement; does nothing.",
                            sortText: "94",
                        },
                    ];

                    for (const snippet of blockSnippets) {
                        suggestions.push({
                            label: snippet.label,
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: snippet.insertText,
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: snippet.documentation,
                            detail: snippet.detailText,
                            range: defaultRange,
                            sortText: snippet.sortText,
                            filterText: snippet.filterText ?? snippet.label,
                        });
                    }

                    for (const keyword of keywordCompletions) {
                        suggestions.push({
                            label: keyword.label,
                            kind: monaco.languages.CompletionItemKind.Keyword,
                            insertText: keyword.insertText,
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: keyword.documentation,
                            range: defaultRange,
                            sortText: keyword.sortText,
                        });
                    }
                }

                return { suggestions };
            },
        });

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
                        const errorObj = unionToCompileErrorUnion(
                            errorType,
                            (obj: any) => compileError.error(obj),
                        );

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

    const titleMouseDownHandler = isSplitDraggable
        ? onSplitDrag
        : (splitMode ? undefined : handleMouseDown);
    const titleTouchStartHandler = isSplitDraggable ? onSplitTouchStart : undefined;

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
                onMouseDown={titleMouseDownHandler}
                onTouchStart={titleTouchStartHandler}
                className="editor-title-bar"
                style={{
                    borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                    cursor: isSplitDraggable
                        ? "row-resize"
                        : (splitMode ? "default" : (isDragging ? "grabbing" : "grab")),
                    touchAction: isSplitDraggable ? "none" : "auto",
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
                            aria-label={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            title={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            className="editor-btn-toggle-split"
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}
                    <button
                        onClick={onClose}
                        aria-label="Close verb editor"
                        className="editor-btn-close"
                    >
                        <span aria-hidden="true">×</span>
                    </button>
                </div>
            </div>

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
                                #{definer}
                            </span>
                        </div>
                    )}

                    {/* Separator bar */}
                    {definer && (owner || permissions || argspec) && <div className="verb-metadata-separator" />}

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
                                        #{owner}
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

                    {/* Argspec - read-only for now */}
                    {argspec && (
                        <div className="verb-metadata-item">
                            <span className="verb-metadata-label">
                                Argspec:
                            </span>
                            <span className="verb-metadata-value">
                                {argspec.dobj} {argspec.prep} {argspec.iobj}
                            </span>
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
                            "\"JetBrains Mono\", \"Fira Code\", \"Source Code Pro\", Consolas, \"Liberation Mono\", Monaco, Menlo, \"Courier New\", monospace",
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
                                clientX: size.width + position.x,
                                clientY: size.height + position.y,
                                button: 0,
                            } as unknown as React.MouseEvent<HTMLDivElement>);
                        }
                    }}
                    className="editor-resize-handle"
                >
                    <div className="editor-resize-handle-border" />
                    <div className="editor-resize-handle-grip" />
                    <span aria-hidden="true" className="editor-resize-handle-icon">
                        ↘
                    </span>
                </div>
            )}
        </div>
    );
};
