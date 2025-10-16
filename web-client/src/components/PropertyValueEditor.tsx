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

// Property value editor for MOO typed values (INT, STR, LIST, MAP, etc.)
// Phase 1 MVP: Simple input, multiline text, and MOO literal code editor

import { useState } from "react";
import { VarUnion } from "../generated/moor-var/var-union.js";
import { MoorVar } from "../lib/MoorVar.js";
import { updatePropertyFlatBuffer } from "../lib/rpc-fb.js";

// Editor modes
export type EditorMode =
    | "input" // Single input (STR, INT, NUM, OBJ, ERR, SYM)
    | "text" // Text editor: lines to \n for STR, lines to list elements for LIST of strings
    | "literal"; // MOO literal syntax editor

export interface PropertyValueEditorProps {
    authToken: string;
    objectCurie: string;
    propertyName: string;
    propertyValue: MoorVar;
    onSave: () => void;
    onCancel: () => void;
}

/**
 * Convert VarUnion type code to readable type name
 */
function typeCodeToName(typeCode: VarUnion): string {
    switch (typeCode) {
        case VarUnion.VarInt:
            return "INT";
        case VarUnion.VarFloat:
            return "FLOAT";
        case VarUnion.VarStr:
            return "STR";
        case VarUnion.VarObj:
            return "OBJ";
        case VarUnion.VarErr:
            return "ERR";
        case VarUnion.VarList:
            return "LIST";
        case VarUnion.VarMap:
            return "MAP";
        case VarUnion.VarSym:
            return "SYM";
        case VarUnion.VarBool:
            return "BOOL";
        case VarUnion.VarNone:
            return "NONE";
        default:
            return VarUnion[typeCode] || "UNKNOWN";
    }
}

/**
 * Check if a list contains only strings
 */
function isStringList(moorVar: MoorVar): boolean {
    if (moorVar.typeCode() !== VarUnion.VarList) {
        return false;
    }

    const list = moorVar.asList();
    if (!list) return false;

    for (const item of list) {
        if (item.typeCode() !== VarUnion.VarStr) {
            return false;
        }
    }

    return true;
}

/**
 * Detects the appropriate editor mode based on the property value
 */
function detectMode(moorVar: MoorVar): EditorMode {
    const typeCode = moorVar.typeCode();

    // Simple primitives (INT, FLOAT, OBJ, ERR, SYM)
    if ([VarUnion.VarInt, VarUnion.VarFloat, VarUnion.VarObj, VarUnion.VarErr, VarUnion.VarSym].includes(typeCode)) {
        return "input";
    }

    if (typeCode === VarUnion.VarStr) {
        const strValue = moorVar.asString() || "";
        // Text mode if contains newlines or is very long
        if (strValue.includes("\n") || strValue.length > 100) {
            return "text";
        }
        return "input";
    }

    // List of strings defaults to text mode
    if (isStringList(moorVar)) {
        return "text";
    }

    // Everything else (LIST, MAP, complex types) goes to literal editor
    return "literal";
}

/**
 * Convert property value to initial editor text based on mode
 */
function toEditorText(moorVar: MoorVar, mode: EditorMode): string {
    const typeCode = moorVar.typeCode();

    if (mode === "input") {
        // For input mode, show raw value without quotes or formatting
        if (typeCode === VarUnion.VarStr) {
            return moorVar.asString() || "";
        }
        // For other primitives, use literal representation (numbers, #obj, etc.)
        return moorVar.toLiteral();
    }

    if (mode === "text") {
        // For strings: show raw content (no quotes)
        if (typeCode === VarUnion.VarStr) {
            return moorVar.asString() || "";
        }

        // For list of strings: each string becomes a line
        if (typeCode === VarUnion.VarList && isStringList(moorVar)) {
            const list = moorVar.asList();
            if (!list) return "";

            const lines: string[] = [];
            for (const item of list) {
                lines.push(item.asString() || "");
            }
            return lines.join("\n");
        }
    }

    return moorVar.toLiteral();
}

/**
 * Convert editor text to MOO literal based on mode
 */
function fromEditorText(text: string, mode: EditorMode, originalTypeCode: VarUnion): string {
    if (mode === "input") {
        // For strings: add quotes and escape special characters
        if (originalTypeCode === VarUnion.VarStr) {
            const escaped = text
                .replace(/\\/g, "\\\\")
                .replace(/"/g, "\\\"");
            return `"${escaped}"`;
        }
        // For other primitives, text is already a literal (numbers, #obj, etc.)
        return text;
    }

    if (mode === "text") {
        // For strings: return as quoted string literal
        if (originalTypeCode === VarUnion.VarStr) {
            // Escape quotes and backslashes, preserve newlines as \n
            const escaped = text
                .replace(/\\/g, "\\\\")
                .replace(/"/g, "\\\"");
            return `"${escaped}"`;
        }

        // For list of strings: convert each line to a quoted string in a list
        if (originalTypeCode === VarUnion.VarList) {
            const lines = text.split("\n");
            const quotedLines = lines.map(line => {
                const escaped = line
                    .replace(/\\/g, "\\\\")
                    .replace(/"/g, "\\\"");
                return `"${escaped}"`;
            });
            return `{${quotedLines.join(", ")}}`;
        }
    }

    // For literal mode, return text as-is (it's already a MOO literal)
    return text;
}

export function PropertyValueEditor({
    authToken,
    objectCurie,
    propertyName,
    propertyValue,
    onSave,
    onCancel,
}: PropertyValueEditorProps) {
    const [mode, setMode] = useState<EditorMode>(() => detectMode(propertyValue));
    const [value, setValue] = useState<string>(() => toEditorText(propertyValue, detectMode(propertyValue)));
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSave = async () => {
        setIsSaving(true);
        setError(null);
        try {
            // Convert editor text to MOO literal based on mode
            const literal = fromEditorText(value, mode, propertyValue.typeCode());

            // Send the literal string value to the backend
            // Backend will parse and validate it
            await updatePropertyFlatBuffer(authToken, objectCurie, propertyName, literal);
            setIsSaving(false);
            onSave();
        } catch (err) {
            setError(err instanceof Error ? err.message : "Failed to save property");
            setIsSaving(false);
        }
    };

    // Track if content has changed from original
    const hasUnsavedChanges = value !== toEditorText(propertyValue, mode);

    // Handle mode changes - warn about unsaved content
    const handleModeChange = (newMode: EditorMode) => {
        if (mode === newMode) return;

        // Warn if there are unsaved changes
        if (hasUnsavedChanges) {
            const proceed = window.confirm(
                "You have unsaved changes. Switching modes will reload the original value. Continue?",
            );
            if (!proceed) return;
        }

        // Reload the original value in the new mode
        setValue(toEditorText(propertyValue, newMode));
        setMode(newMode);
    };

    const typeName = typeCodeToName(propertyValue.typeCode());

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                height: "100%",
                backgroundColor: "var(--color-bg-input)",
                border: "1px solid var(--color-border-medium)",
            }}
        >
            {/* Title bar */}
            <div
                style={{
                    padding: "var(--space-md)",
                    borderBottom: "1px solid var(--color-border-light)",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    backgroundColor: "var(--color-bg-header)",
                }}
            >
                <h3
                    style={{
                        margin: 0,
                        color: "var(--color-text-primary)",
                        display: "flex",
                        alignItems: "baseline",
                        width: "100%",
                    }}
                >
                    <span style={{ fontWeight: "700" }}>
                        Property editor{hasUnsavedChanges && (
                            <span
                                style={{ color: "var(--color-text-secondary)", marginLeft: "4px", fontSize: "0.8em" }}
                            >
                                ●
                            </span>
                        )}
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
                        {propertyName} ({typeName})
                    </span>
                </h3>
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
                    {/* Mode switcher - show based on type */}
                    {propertyValue.typeCode() === VarUnion.VarStr && (
                        <>
                            <button
                                onClick={() => handleModeChange("input")}
                                aria-label="Input mode"
                                title="Single-line input mode"
                                style={{
                                    background: mode === "input" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "input"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "input" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Input
                            </button>
                            <button
                                onClick={() => handleModeChange("text")}
                                aria-label="Text editor mode"
                                title="Text editor mode (lines with \n)"
                                style={{
                                    background: mode === "text" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "text"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "text" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Text
                            </button>
                            <button
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
                                style={{
                                    background: mode === "literal" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "literal"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "literal" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Literal
                            </button>
                        </>
                    )}
                    {isStringList(propertyValue) && (
                        <>
                            <button
                                onClick={() => handleModeChange("text")}
                                aria-label="Text editor mode"
                                title="Text editor mode (one string per line)"
                                style={{
                                    background: mode === "text" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "text"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "text" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Text
                            </button>
                            <button
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
                                style={{
                                    background: mode === "literal" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "literal"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "literal" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Literal
                            </button>
                        </>
                    )}
                    {!isStringList(propertyValue) && propertyValue.typeCode() !== VarUnion.VarStr
                        && ![VarUnion.VarInt, VarUnion.VarFloat, VarUnion.VarObj, VarUnion.VarErr, VarUnion.VarSym]
                            .includes(propertyValue.typeCode())
                        && (
                            <button
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
                                style={{
                                    background: mode === "literal" ? "var(--color-bg-button-hover)" : "transparent",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    cursor: "pointer",
                                    color: mode === "literal"
                                        ? "var(--color-text-primary)"
                                        : "var(--color-text-secondary)",
                                    fontWeight: mode === "literal" ? "600" : "normal",
                                    padding: "4px 8px",
                                    fontSize: "11px",
                                }}
                            >
                                Literal
                            </button>
                        )}
                    {/* Save button */}
                    <button
                        onClick={handleSave}
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
                    {/* Close button */}
                    <button
                        onClick={onCancel}
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

            {/* Error message */}
            {error && (
                <div
                    style={{
                        padding: "var(--space-sm)",
                        backgroundColor: "var(--color-bg-error)",
                        borderTop: "1px solid var(--color-border-light)",
                        borderBottom: "1px solid var(--color-border-light)",
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
                        {error}
                    </pre>
                </div>
            )}

            {/* Editor content */}
            <div
                style={{
                    flex: 1,
                    minHeight: 0,
                    position: "relative",
                    overflow: "hidden",
                    padding: "var(--space-md)",
                }}
            >
                {mode === "input" && (
                    <input
                        type="text"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        style={{
                            width: "100%",
                            padding: "var(--space-sm)",
                            fontSize: "13px",
                            fontFamily: "var(--font-mono)",
                            backgroundColor: "var(--color-bg-input)",
                            border: "1px solid var(--color-border-medium)",
                            borderRadius: "var(--radius-sm)",
                            color: "var(--color-text-primary)",
                        }}
                        placeholder={`Enter ${typeName} value...`}
                    />
                )}

                {mode === "text" && (
                    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
                        <textarea
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            style={{
                                width: "100%",
                                flex: 1,
                                minHeight: "200px",
                                padding: "var(--space-sm)",
                                fontSize: "13px",
                                fontFamily: "var(--font-mono)",
                                backgroundColor: "var(--color-bg-input)",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "var(--radius-sm)",
                                color: "var(--color-text-primary)",
                                resize: "vertical",
                            }}
                            placeholder={propertyValue.typeCode() === VarUnion.VarStr
                                ? "Enter text (newlines will be preserved as \\n)..."
                                : "Enter one string per line..."}
                        />
                        <div
                            style={{
                                fontSize: "11px",
                                color: "var(--color-text-secondary)",
                                marginTop: "var(--space-xs)",
                            }}
                        >
                            {propertyValue.typeCode() === VarUnion.VarStr
                                ? "Each line becomes part of the string with \\n separators"
                                : "Each line becomes a separate string in the list"}
                        </div>
                    </div>
                )}

                {mode === "literal" && (
                    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
                        <textarea
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            style={{
                                width: "100%",
                                flex: 1,
                                minHeight: "200px",
                                padding: "var(--space-sm)",
                                fontSize: "13px",
                                fontFamily: "var(--font-mono)",
                                backgroundColor: "var(--color-bg-input)",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "var(--radius-sm)",
                                color: "var(--color-text-primary)",
                                resize: "vertical",
                            }}
                            placeholder="Enter MOO literal value (e.g., {1, 2, 3} or [1 -> &quot;a&quot;, 2 -> &quot;b&quot;])..."
                        />
                        <div
                            style={{
                                fontSize: "11px",
                                color: "var(--color-text-secondary)",
                                marginTop: "var(--space-xs)",
                            }}
                        >
                            Use MOO literal syntax: {"{...}"} for lists, {"[k -> v, ...]"} for maps
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
