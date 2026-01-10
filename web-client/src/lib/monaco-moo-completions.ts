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

// Monaco editor completion provider for MOO language

import type { Monaco } from "@monaco-editor/react";
import type * as monaco from "monaco-editor";
import { performEvalFlatBuffer } from "./rpc-fb";
import { objToString } from "./var";

// Shared cache for verb/property/builtin lookups across all editor instances
const completionCache = new Map<
    string,
    { verbs?: any; properties?: any; builtins?: any; timestamp: number }
>();
const CACHE_TTL = 30000; // 30 seconds

// Editor context for completion lookups
interface EditorContext {
    authToken: string;
    objectCurie?: string;
    uploadAction?: string;
}

// Singleton manager for MOO completion provider
class MooCompletionManager {
    private static instance: MooCompletionManager | null = null;
    private providerDisposable: monaco.IDisposable | null = null;
    private editorContexts = new Map<string, EditorContext>();
    private monacoInstance: Monaco | null = null;

    private constructor() {}

    static getInstance(): MooCompletionManager {
        if (!MooCompletionManager.instance) {
            MooCompletionManager.instance = new MooCompletionManager();
        }
        return MooCompletionManager.instance;
    }

    /**
     * Register an editor's context for completions.
     * @param modelUri The Monaco model URI (unique identifier for the editor)
     * @param context The editor's context (auth token, object curie, etc.)
     * @param monaco The Monaco instance (needed for first registration)
     */
    register(modelUri: string, context: EditorContext, monaco: Monaco): void {
        this.editorContexts.set(modelUri, context);

        // Register the global provider only once
        if (!this.providerDisposable && !this.monacoInstance) {
            this.monacoInstance = monaco;
            this.providerDisposable = this.createProvider(monaco);
        }
    }

    /**
     * Update an editor's context (e.g., when props change).
     */
    updateContext(modelUri: string, context: EditorContext): void {
        if (this.editorContexts.has(modelUri)) {
            this.editorContexts.set(modelUri, context);
        }
    }

    /**
     * Unregister an editor when it unmounts.
     */
    unregister(modelUri: string): void {
        this.editorContexts.delete(modelUri);

        // If no more editors, dispose the provider
        if (this.editorContexts.size === 0 && this.providerDisposable) {
            this.providerDisposable.dispose();
            this.providerDisposable = null;
            this.monacoInstance = null;
        }
    }

    /**
     * Get context for a specific model URI.
     */
    getContext(modelUri: string): EditorContext | undefined {
        return this.editorContexts.get(modelUri);
    }

    private createProvider(monaco: Monaco): monaco.IDisposable {
        return monaco.languages.registerCompletionItemProvider("moo", {
            provideCompletionItems: async (model, position) => {
                const modelUri = model.uri.toString();
                const context = this.editorContexts.get(modelUri);

                if (!context) {
                    // No context for this editor, return empty suggestions
                    return { suggestions: [] };
                }

                return provideCompletionsForContext(monaco, model, position, context);
            },
        });
    }
}

// Export the singleton instance
export const mooCompletionManager = MooCompletionManager.getInstance();

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
    if (cached && cached.builtins && Date.now() - cached.timestamp < CACHE_TTL) {
        return cached.builtins;
    }
    const builtins = await fetchFn();
    completionCache.set(cacheKey, { ...cached, builtins, timestamp: Date.now() });
    return builtins;
};

// Type code to string mapping for builtin function arguments
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

// Generic property completion for any object reference
const addPropertyCompletions = async (
    monacoInstance: Monaco,
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
                kind: monacoInstance.languages.CompletionItemKind.Property,
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
    monacoInstance: Monaco,
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
                    kind: monacoInstance.languages.CompletionItemKind.Method,
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
    monacoInstance: Monaco,
    authToken: string,
    prefix: string,
    startColumn: number,
    position: any,
    suggestions: monaco.languages.CompletionItem[],
) => {
    try {
        const builtins = await getCachedBuiltins("builtins", async () => {
            const result = await performEvalFlatBuffer(authToken, "return function_info();");
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

        let sortIndex = 0;
        for (const builtin of builtins) {
            if (!builtin.name.startsWith(prefix)) continue;

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
                kind: monacoInstance.languages.CompletionItemKind.Function,
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
        console.warn("Failed to fetch builtin completions:", error);
    }
};

/**
 * Provide completions for a specific editor context.
 * This is the core completion logic, extracted to work with the singleton manager.
 */
async function provideCompletionsForContext(
    monacoInstance: Monaco,
    model: monaco.editor.ITextModel,
    position: monaco.Position,
    context: EditorContext,
): Promise<monaco.languages.CompletionList> {
    const { authToken, objectCurie, uploadAction } = context;
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
    if (thisVerbMatch && objectCurie) {
        const { MoorRemoteObject, curieORef } = await import("./rpc");
        const { oidRef } = await import("./var");
        const currentObject = actualObjectId
            ? new MoorRemoteObject(oidRef(actualObjectId), authToken)
            : new MoorRemoteObject(curieORef(objectCurie), authToken);
        const cacheKey = actualObjectId ? `#${actualObjectId}:verbs` : `this:verbs`;

        await addVerbCompletions(
            monacoInstance,
            currentObject,
            cacheKey,
            "this object",
            thisVerbMatch[1],
            position.column - thisVerbMatch[1].length,
            position,
            suggestions,
        );
    } else if (thisPropMatch && objectCurie) {
        // Smart completion for this. properties
        const { MoorRemoteObject, curieORef } = await import("./rpc");
        const { oidRef } = await import("./var");
        const currentObject = actualObjectId
            ? new MoorRemoteObject(oidRef(actualObjectId), authToken)
            : new MoorRemoteObject(curieORef(objectCurie), authToken);
        const cacheKey = actualObjectId ? `#${actualObjectId}:properties` : `this:properties`;

        await addPropertyCompletions(
            monacoInstance,
            currentObject,
            cacheKey,
            "this object",
            thisPropMatch[1],
            position.column - thisPropMatch[1].length,
            position,
            suggestions,
        );
    } else if (objVerbMatch) {
        // Smart completion for #123: object verb calls
        const { MoorRemoteObject } = await import("./rpc");
        const { oidRef } = await import("./var");
        const objectId = parseInt(objVerbMatch[1]);
        const targetObject = new MoorRemoteObject(oidRef(objectId), authToken);

        await addVerbCompletions(
            monacoInstance,
            targetObject,
            `#${objectId}:verbs`,
            `object #${objectId}`,
            objVerbMatch[2],
            position.column - objVerbMatch[2].length,
            position,
            suggestions,
        );
    } else if (objPropMatch) {
        // Smart completion for #123. object property access
        const { MoorRemoteObject } = await import("./rpc");
        const { oidRef } = await import("./var");
        const objectId = parseInt(objPropMatch[1]);
        const targetObject = new MoorRemoteObject(oidRef(objectId), authToken);

        await addPropertyCompletions(
            monacoInstance,
            targetObject,
            `#${objectId}:properties`,
            `object #${objectId}`,
            objPropMatch[2],
            position.column - objPropMatch[2].length,
            position,
            suggestions,
        );
    } else if (sysPropMatch) {
        // Smart completion for $thing. property access
        const { MoorRemoteObject } = await import("./rpc");
        const { sysobjRef } = await import("./var");
        const targetObject = new MoorRemoteObject(sysobjRef([sysPropMatch[1]]), authToken);

        await addPropertyCompletions(
            monacoInstance,
            targetObject,
            `$${sysPropMatch[1]}:properties`,
            `$${sysPropMatch[1]}`,
            sysPropMatch[2],
            position.column - sysPropMatch[2].length,
            position,
            suggestions,
        );
    } else if (sysVerbMatch) {
        // Smart completion for $thing: verb calls
        const { MoorRemoteObject } = await import("./rpc");
        const { sysobjRef } = await import("./var");
        const targetObject = new MoorRemoteObject(sysobjRef([sysVerbMatch[1]]), authToken);

        await addVerbCompletions(
            monacoInstance,
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
                    insertText: `let {\${1:${variableName}}, \${2:?optional = default}, \${3:@rest}} = \${4:expr};`,
                    documentation: "Unpack a list (or map) into variables, with optional and rest bindings.",
                    detail: "Unpack a collection",
                    sortText: "10",
                },
            ];

            for (const snippet of letSnippets) {
                suggestions.push({
                    label: snippet.label,
                    kind: monacoInstance.languages.CompletionItemKind.Snippet,
                    insertText: snippet.insertText,
                    insertTextRules: monacoInstance.languages.CompletionItemInsertTextRule.InsertAsSnippet,
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
                    insertText: `const {\${1:${constantName}}, \${2:?optional = default}, \${3:@rest}} = \${4:expr};`,
                    documentation: "Unpack values into constant bindings; the rest binding remains a list.",
                    detail: "Unpack to constants",
                    sortText: "10",
                },
            ];

            for (const snippet of constSnippets) {
                suggestions.push({
                    label: snippet.label,
                    kind: monacoInstance.languages.CompletionItemKind.Snippet,
                    insertText: snippet.insertText,
                    insertTextRules: monacoInstance.languages.CompletionItemInsertTextRule.InsertAsSnippet,
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
        const isAfterObjectOp = beforeWord.match(/[:.]$/);

        if (!isAfterObjectOp && word.word.length > 0) {
            await addBuiltinCompletions(
                monacoInstance,
                authToken,
                word.word,
                word.startColumn,
                position,
                suggestions,
            );
        }
    }

    // If no smart completions matched, show block templates
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
                documentation: "Inline error handling: value ! error_pattern => fallback_value",
                sortText: "60",
                detailText: "Inline error handler",
                filterText: "try",
            },
            {
                label: "fork/endfork",
                insertText: "fork (${1:seconds})\n\t${2}\nendfork",
                documentation: "Schedule code to run asynchronously after a delay.",
                sortText: "70",
                detailText: "Async execution",
                filterText: "fork",
            },
        ];

        for (const snippet of blockSnippets) {
            suggestions.push({
                label: snippet.label,
                kind: monacoInstance.languages.CompletionItemKind.Snippet,
                insertText: snippet.insertText,
                insertTextRules: monacoInstance.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                documentation: snippet.documentation,
                detail: snippet.detailText,
                range: defaultRange,
                sortText: snippet.sortText,
                filterText: snippet.filterText,
            });
        }
    }

    return { suggestions };
}
