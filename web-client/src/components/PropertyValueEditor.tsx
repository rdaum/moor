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

import { useCallback, useEffect, useState } from "react";
import { VarUnion } from "../generated/moor-var/var-union.js";
import { MoorVar } from "../lib/MoorVar.js";
import { performEvalFlatBuffer, updatePropertyFlatBuffer } from "../lib/rpc-fb.js";

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
    onDelete?: () => void; // Optional delete handler - only shown if property is locally defined
    // Property metadata
    owner?: string; // Object ID or CURIE of property owner
    definer?: string; // Object ID or CURIE of property definer
    permissions?: { readable: boolean; writable: boolean }; // Property permissions
    onNavigateToObject?: (objId: string) => void; // Callback for clicking object references
    normalizeObjectInput?: (raw: string) => string; // Utility to convert various object formats to MOO expressions
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
    onDelete,
    owner,
    definer,
    permissions,
    onNavigateToObject,
    normalizeObjectInput,
}: PropertyValueEditorProps) {
    const [mode, setMode] = useState<EditorMode>(() => detectMode(propertyValue));
    const [value, setValue] = useState<string>(() => toEditorText(propertyValue, detectMode(propertyValue)));
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Property metadata editing state
    const [isEditingOwner, setIsEditingOwner] = useState(false);
    const [editOwnerValue, setEditOwnerValue] = useState(owner ? `#${owner}` : "");
    const [editPermissions, setEditPermissions] = useState(permissions || { readable: false, writable: false });
    const [isSavingMetadata, setIsSavingMetadata] = useState(false);
    const [metadataSaveSuccess, setMetadataSaveSuccess] = useState(false);

    // Sync local state when props change (after save refresh)
    useEffect(() => {
        setEditOwnerValue(owner ? `#${owner}` : "");
        setEditPermissions(permissions || { readable: false, writable: false });
    }, [owner, permissions]);
    const FONT_SIZE_STORAGE_KEY = "moor-code-editor-font-size";
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 24;
    const [fontSize, setFontSize] = useState(() => {
        const fallback = 12;
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

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem(FONT_SIZE_STORAGE_KEY, fontSize.toString());
        }
    }, [fontSize]);

    const decreaseFontSize = useCallback(() => {
        setFontSize(prev => Math.max(MIN_FONT_SIZE, prev - 1));
    }, []);

    const increaseFontSize = useCallback(() => {
        setFontSize(prev => Math.min(MAX_FONT_SIZE, prev + 1));
    }, []);

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

    const handleSaveMetadata = async () => {
        if (!normalizeObjectInput) {
            setError("Cannot save: normalizeObjectInput function not provided");
            return;
        }

        setIsSavingMetadata(true);
        setError(null);
        setMetadataSaveSuccess(false);
        try {
            const permsStr = `${editPermissions.readable ? "r" : ""}${editPermissions.writable ? "w" : ""}`;

            // Use the provided utility to normalize object references
            const objExpr = normalizeObjectInput(objectCurie);
            const ownerExpr = normalizeObjectInput(editOwnerValue);

            if (!objExpr || !ownerExpr) {
                throw new Error("Invalid object reference");
            }

            // Call: set_property_info(obj, 'propname, {owner, "perms"})
            const expr = `return set_property_info(${objExpr}, '${propertyName}, {${ownerExpr}, "${permsStr}"});`;
            await performEvalFlatBuffer(authToken, expr);
            setIsSavingMetadata(false);
            setIsEditingOwner(false);
            setMetadataSaveSuccess(true);

            // Clear success message and reset button visibility after 2 seconds
            setTimeout(() => {
                setMetadataSaveSuccess(false);
            }, 2000);

            // Notify parent to refresh (this will update owner/permissions props)
            onSave();
        } catch (err) {
            setError(err instanceof Error ? err.message : "Failed to save property metadata");
            setIsSavingMetadata(false);
        }
    };

    const handleTogglePermission = (perm: "readable" | "writable") => {
        setEditPermissions((prev) => ({
            ...prev,
            [perm]: !prev[perm],
        }));
    };

    // Track if content has changed from original
    const hasUnsavedChanges = value !== toEditorText(propertyValue, mode);

    // Track if metadata has changed (compare normalized values) or is being edited
    const hasMetadataChanges = isEditingOwner
        || (owner ? `#${owner}` : "") !== editOwnerValue
        || permissions?.readable !== editPermissions.readable
        || permissions?.writable !== editPermissions.writable;

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
                    {/* Remove button - only shown if onDelete handler provided */}
                    {onDelete && (
                        <button
                            onClick={onDelete}
                            aria-label="Remove property"
                            title="Remove property"
                            style={{
                                backgroundColor:
                                    "color-mix(in srgb, var(--color-text-error) 20%, var(--color-bg-secondary))",
                                color: "var(--color-text-primary)",
                                border: "1px solid var(--color-border-medium)",
                                padding: "6px 12px",
                                borderRadius: "var(--radius-sm)",
                                cursor: "pointer",
                                fontSize: "12px",
                                fontWeight: "600",
                            }}
                        >
                            Remove
                        </button>
                    )}
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "4px",
                            backgroundColor: "var(--color-bg-secondary)",
                            border: "1px solid var(--color-border-medium)",
                            borderRadius: "var(--radius-sm)",
                            padding: "2px 6px",
                        }}
                    >
                        <button
                            onClick={decreaseFontSize}
                            aria-label="Decrease editor font size"
                            style={{
                                background: "transparent",
                                border: "none",
                                color: "var(--color-text-secondary)",
                                cursor: fontSize <= MIN_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize <= MIN_FONT_SIZE ? 0.5 : 1,
                                fontSize: "14px",
                                padding: "2px 4px",
                            }}
                            disabled={fontSize <= MIN_FONT_SIZE}
                        >
                            –
                        </button>
                        <span
                            style={{
                                fontFamily: "var(--font-mono)",
                                fontSize: "12px",
                                color: "var(--color-text-secondary)",
                                minWidth: "38px",
                                textAlign: "center",
                            }}
                            aria-live="polite"
                        >
                            {fontSize}px
                        </span>
                        <button
                            onClick={increaseFontSize}
                            aria-label="Increase editor font size"
                            style={{
                                background: "transparent",
                                border: "none",
                                color: "var(--color-text-secondary)",
                                cursor: fontSize >= MAX_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize >= MAX_FONT_SIZE ? 0.5 : 1,
                                fontSize: "14px",
                                padding: "2px 4px",
                            }}
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>
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

            {/* Property metadata info panel */}
            {(owner || definer || permissions) && (
                <div
                    style={{
                        padding: "var(--space-sm) var(--space-md)",
                        backgroundColor: "var(--color-bg-tertiary)",
                        borderBottom: "1px solid var(--color-border-light)",
                        fontSize: "0.9em",
                        display: "flex",
                        gap: "var(--space-md)",
                        flexWrap: "wrap",
                        alignItems: "center",
                    }}
                >
                    {/* Definer - read-only, visually separated */}
                    {definer && (
                        <div style={{ display: "flex", alignItems: "center", gap: "6px", opacity: 0.6 }}>
                            <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                                Definer:
                            </span>
                            {onNavigateToObject
                                ? (
                                    <button
                                        onClick={() => onNavigateToObject(definer)}
                                        style={{
                                            background: "none",
                                            border: "1px solid var(--color-border-medium)",
                                            borderRadius: "var(--radius-sm)",
                                            color: "var(--color-text-accent)",
                                            cursor: "pointer",
                                            padding: "2px 6px",
                                            fontFamily: "var(--font-mono)",
                                            fontSize: "0.95em",
                                        }}
                                    >
                                        #{definer}
                                    </button>
                                )
                                : (
                                    <span
                                        style={{
                                            fontFamily: "var(--font-mono)",
                                            border: "1px solid var(--color-border-medium)",
                                            borderRadius: "var(--radius-sm)",
                                            padding: "2px 6px",
                                            fontSize: "0.95em",
                                        }}
                                    >
                                        #{definer}
                                    </span>
                                )}
                        </div>
                    )}

                    {/* Separator bar */}
                    {definer && (owner || permissions) && (
                        <div
                            style={{
                                width: "1px",
                                height: "20px",
                                backgroundColor: "var(--color-border-medium)",
                            }}
                        />
                    )}

                    {/* Owner - editable */}
                    {owner && (
                        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                            <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                                Owner:
                            </span>
                            {isEditingOwner
                                ? (
                                    <input
                                        type="text"
                                        value={editOwnerValue}
                                        onChange={(e) => setEditOwnerValue(e.target.value)}
                                        style={{
                                            fontFamily: "var(--font-mono)",
                                            border: "1px solid var(--color-border-medium)",
                                            borderRadius: "var(--radius-sm)",
                                            padding: "2px 6px",
                                            fontSize: "0.95em",
                                            width: "80px",
                                            backgroundColor: "var(--color-bg-input)",
                                        }}
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
                                        style={{
                                            background: "none",
                                            border: "1px solid var(--color-border-medium)",
                                            borderRadius: "var(--radius-sm)",
                                            color: "var(--color-text-primary)",
                                            cursor: "pointer",
                                            padding: "2px 6px",
                                            fontFamily: "var(--font-mono)",
                                            fontSize: "0.95em",
                                        }}
                                        title="Click to edit owner"
                                    >
                                        #{owner}
                                    </button>
                                )}
                        </div>
                    )}

                    {/* Permissions - toggle checkboxes */}
                    {permissions && (
                        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                            <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                                Perms:
                            </span>
                            <div
                                style={{
                                    display: "flex",
                                    gap: "4px",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    padding: "2px 4px",
                                }}
                            >
                                <label
                                    style={{
                                        display: "flex",
                                        alignItems: "center",
                                        gap: "2px",
                                        cursor: "pointer",
                                        fontFamily: "var(--font-mono)",
                                        fontSize: "0.95em",
                                    }}
                                >
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.readable}
                                        onChange={() => handleTogglePermission("readable")}
                                        style={{ cursor: "pointer" }}
                                    />
                                    r
                                </label>
                                <label
                                    style={{
                                        display: "flex",
                                        alignItems: "center",
                                        gap: "2px",
                                        cursor: "pointer",
                                        fontFamily: "var(--font-mono)",
                                        fontSize: "0.95em",
                                    }}
                                >
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.writable}
                                        onChange={() => handleTogglePermission("writable")}
                                        style={{ cursor: "pointer" }}
                                    />
                                    w
                                </label>
                            </div>
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
                                        setEditPermissions(permissions || { readable: false, writable: false });
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
                            fontSize: `${fontSize}px`,
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
                                fontSize: `${fontSize}px`,
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
                                fontSize: `${fontSize}px`,
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
