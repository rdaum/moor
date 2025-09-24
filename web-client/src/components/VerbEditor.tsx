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
    const editorThemeObserverRef = useRef<MutationObserver | null>(null);
    const editorThemeListenerRef = useRef<(() => void) | null>(null);
    const containerRef = useRef<HTMLDivElement | null>(null);
    const errorDecorationsRef = useRef<monaco.editor.IEditorDecorationsCollection | null>(null);

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
                    [/\b(return|break|continue|pass)\b/, "keyword.control"],

                    // Declaration keywords
                    [/\b(let|const|global|fn|endfn)\b/, "keyword.declaration"],

                    // Special keywords
                    [/\b(any|in)\b/, "keyword.operator"],

                    // Built-in constants
                    [/\b(true|false)\b/, "constant.language"],

                    // Type constants
                    [/\b(INT|NUM|FLOAT|STR|ERR|OBJ|LIST|MAP|BOOL|FLYWEIGHT|SYM)\b/, "type"],

                    // Error constants with optional message
                    [/\bE_[A-Z_]+\([^)]*\)/, "constant.other"],
                    [/\bE_[A-Z_]+/, "constant.other"],

                    // Binary literals (base64-encoded)
                    [/b"[A-Za-z0-9+/=_-]*"/, "string.regexp"],

                    // Object references (#123, #-1)
                    [/#-?\d+/, "number.hex"],

                    // System properties and verbs ($property)
                    [/\$[a-zA-Z_][a-zA-Z0-9_]*/, "variable.predefined"],

                    // Symbols ('symbol)
                    [/'[a-zA-Z_][a-zA-Z0-9_]*/, "string.key"],

                    // Try expressions (backtick to single quote)
                    [/`[^']*'/, "string.escape"],

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
                    [/(==|!=|<=|>=)/, "operator.comparison"],
                    [/(&&|\|\|)/, "operator.logical"],
                    [/[<>]/, "operator.comparison"],
                    [/=/, "operator.assignment"],
                    [/!/, "operator.logical"],
                    [/[+\-*/%^]/, "operator.arithmetic"],
                    [/[?|]/, "operator.conditional"], // Ternary operators
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
            { verbs?: any[]; properties?: Record<string, any>; timestamp: number }
        >();
        const CACHE_TTL = 30000; // 30 seconds

        const getCachedVerbs = async (cacheKey: string, fetchFn: () => Promise<any[]>) => {
            const cached = completionCache.get(cacheKey);
            if (cached && cached.verbs && Date.now() - cached.timestamp < CACHE_TTL) {
                return cached.verbs;
            }
            const verbs = await fetchFn();
            completionCache.set(cacheKey, { ...cached, verbs, timestamp: Date.now() });
            return verbs;
        };

        const getCachedProperties = async (cacheKey: string, fetchFn: () => Promise<Record<string, any>>) => {
            const cached = completionCache.get(cacheKey);
            if (cached && cached.properties && Date.now() - cached.timestamp < CACHE_TTL) {
                return cached.properties;
            }
            const properties = await fetchFn();
            completionCache.set(cacheKey, { ...cached, properties, timestamp: Date.now() });
            return properties;
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
                const properties = await getCachedProperties(cacheKey, () => objectRef.getProperties());
                properties.forEach((prop: any, index: number) => {
                    if (prop.name && prop.name.startsWith(prefix)) {
                        suggestions.push({
                            label: {
                                label: prop.name,
                                detail: ` #${prop.definer}`,
                            },
                            kind: monaco.languages.CompletionItemKind.Property,
                            insertText: prop.name,
                            sortText: index.toString().padStart(4, "0"),
                            documentation:
                                `Property on ${contextLabel} (defined in #${prop.definer}, owner: #${prop.owner}, ${
                                    prop.r ? "readable" : "not readable"
                                }, ${prop.w ? "writable" : "read-only"})`,
                            range: {
                                startLineNumber: position.lineNumber,
                                endLineNumber: position.lineNumber,
                                startColumn,
                                endColumn: position.column,
                            },
                        });
                    }
                });
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
                const verbs = await getCachedVerbs(cacheKey, () => objectRef.getVerbs());
                let sortIndex = 0;
                verbs.forEach((verb: any) => {
                    if (verb.names && Array.isArray(verb.names)) {
                        verb.names.forEach((verbName: string) => {
                            if (verbName.startsWith(prefix)) {
                                suggestions.push({
                                    label: {
                                        label: verbName,
                                        detail: ` #${verb.location}`,
                                    },
                                    kind: monaco.languages.CompletionItemKind.Method,
                                    insertText: verbName,
                                    sortText: sortIndex.toString().padStart(4, "0"),
                                    documentation:
                                        `Verb on ${contextLabel} (defined in #${verb.location}, owner: #${verb.owner}, ${
                                            verb.r ? "readable" : "not readable"
                                        }, ${verb.x ? "executable" : "not executable"})`,
                                    range: {
                                        startLineNumber: position.lineNumber,
                                        endLineNumber: position.lineNumber,
                                        startColumn,
                                        endColumn: position.column,
                                    },
                                });
                                sortIndex++;
                            }
                        });
                    }
                });
            } catch (error) {
                console.warn(`Failed to fetch verbs for ${contextLabel}:`, error);
            }
        };

        // Add completion provider for MOO block structures and smart completions
        monaco.languages.registerCompletionItemProvider("moo", {
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
                } // If no smart completions matched, show block templates
                else {
                    const defaultRange = {
                        startLineNumber: position.lineNumber,
                        endLineNumber: position.lineNumber,
                        startColumn: position.column,
                        endColumn: position.column,
                    };

                    suggestions.push(
                        {
                            label: "if",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "if (${1:condition})\n\t${2}\nendif",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "if...endif block",
                            range: defaultRange,
                        },
                        {
                            label: "while",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "while (${1:condition})\n\t${2}\nendwhile",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "while...endwhile block",
                            range: defaultRange,
                        },
                        {
                            label: "for-in",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "for ${1:item} in (${2:collection})\n\t${3}\nendfor",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "for item in (collection) loop",
                            range: defaultRange,
                        },
                        {
                            label: "for-range",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "for ${1:i} in [${2:start}..${3:end}]\n\t${4}\nendfor",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "for i in [start..end] range loop",
                            range: defaultRange,
                        },
                        {
                            label: "try",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "try\n\t${1}\nexcept (${2:E_ANY})\n\t${3}\nendtry",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "try...endtry block",
                            range: defaultRange,
                        },
                        {
                            label: "fork",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "fork (${1:0})\n\t${2}\nendfork",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "fork...endfork block",
                            range: defaultRange,
                        },
                        {
                            label: "fn",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "fn ${1:name}(${2:args})\n\t${3}\nendfn",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "fn...endfn block",
                            range: defaultRange,
                        },
                        {
                            label: "begin",
                            kind: monaco.languages.CompletionItemKind.Snippet,
                            insertText: "begin\n\t${1}\nend",
                            insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                            documentation: "begin...end block",
                            range: defaultRange,
                        },
                    );
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
                    `/verbs/${objectCurie}/${encodeURIComponent(verbName)}`,
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
                if (result && typeof result === "object" && result.errors) {
                    // Parse compilation errors from nested structure
                    const compilationErrors: CompileError[] = [];

                    if (result.errors.ParseError) {
                        const parseError = result.errors.ParseError;
                        if (
                            parseError.error_position && parseError.error_position.line_col
                            && Array.isArray(parseError.error_position.line_col)
                        ) {
                            const lineCol = parseError.error_position.line_col;
                            const line = lineCol[0];
                            const column = lineCol[1];
                            compilationErrors.push({
                                type: "parse",
                                message: parseError.message || "Parse error",
                                line: line,
                                column: column,
                            });
                        } else {
                            compilationErrors.push({
                                type: "parse",
                                message: parseError.message || "Parse error",
                            });
                        }
                    } else if (result.errors) {
                        // Handle other error types if they exist
                        compilationErrors.push({
                            type: "other",
                            message: JSON.stringify(result.errors),
                        });
                    }

                    setErrors(compilationErrors);

                    // Set Monaco error markers
                    if (editorRef.current) {
                        const model = editorRef.current.getModel();
                        if (model) {
                            const markers = compilationErrors.map(error => {
                                const line = error.line || 1;
                                const column = error.column || 1;
                                // Make the error span a wider area to be more visible
                                const lineText = model.getLineContent(line);
                                const wordEnd = lineText.indexOf(" ", column - 1);
                                const endColumn = wordEnd !== -1 ? wordEnd + 1 : model.getLineMaxColumn(line);

                                return {
                                    severity: monaco.MarkerSeverity.Error,
                                    message: error.message,
                                    startLineNumber: line,
                                    startColumn: column,
                                    endLineNumber: line,
                                    endColumn: Math.max(column + 5, endColumn), // Span at least 5 characters or to end of word
                                };
                            });
                            monaco.editor.setModelMarkers(model, "moo-compiler", markers);

                            // Add more visible decorations
                            if (errorDecorationsRef.current) {
                                const decorations = compilationErrors.map(error => {
                                    const line = error.line || 1;
                                    const column = error.column || 1;
                                    const lineText = model.getLineContent(line);
                                    const wordEnd = lineText.indexOf(" ", column - 1);
                                    const endColumn = wordEnd !== -1 ? wordEnd + 1 : model.getLineMaxColumn(line);

                                    return {
                                        range: new monaco.Range(line, column, line, Math.max(column + 5, endColumn)),
                                        options: {
                                            className: "moo-error-decoration",
                                            inlineClassName: "moo-error-inline",
                                            hoverMessage: { value: error.message },
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
                <h3
                    id="verb-editor-title"
                    style={{
                        margin: 0,
                        color: "var(--color-text-primary)",
                        display: "flex",
                        alignItems: "baseline",
                        width: "100%",
                    }}
                >
                    <span style={{ fontWeight: "700" }}>
                        Verb editor
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
                    {/* Compile button */}
                    <button
                        onClick={(e) => {
                            e.stopPropagation(); // Prevent drag handler from firing
                            compileVerb();
                        }}
                        disabled={isCompiling}
                        aria-label="Compile verb"
                        title="Compile verb"
                        style={{
                            backgroundColor: isCompiling ? "var(--color-bg-secondary)" : "var(--color-button-primary)",
                            color: "white",
                            border: "none",
                            padding: "6px 12px",
                            borderRadius: "var(--radius-sm)",
                            cursor: isCompiling ? "not-allowed" : "pointer",
                            opacity: isCompiling ? 0.6 : 1,
                            fontSize: "12px",
                            fontWeight: "600",
                        }}
                    >
                        {isCompiling ? "⏳" : "▶"}
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
                    language="moo"
                    theme="vs-dark"
                    onChange={handleEditorChange}
                    beforeMount={handleEditorWillMount}
                    onMount={handleEditorDidMount}
                    options={{
                        minimap: { enabled: !isMobile },
                        fontSize: isMobile ? 16 : 12,
                        fontFamily:
                            "\"JetBrains Mono\", \"Fira Code\", \"Source Code Pro\", Consolas, \"Liberation Mono\", Monaco, Menlo, \"Courier New\", monospace",
                        automaticLayout: true,
                        colorDecorators: true,
                        dragAndDrop: false,
                        emptySelectionClipboard: false,
                        autoClosingDelete: "never",
                        wordWrap: isMobile ? "on" : "off",
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
