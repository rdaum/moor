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
import { usePersistentState } from "../hooks/usePersistentState";
import { MoorVar } from "../lib/MoorVar.js";
import { performEvalFlatBuffer, updatePropertyFlatBuffer } from "../lib/rpc-fb.js";
import { useTitleBarDrag } from "./EditorWindow";

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
    permissions?: { readable: boolean; writable: boolean; chown: boolean }; // Property permissions
    onNavigateToObject?: (objId: string) => void; // Callback for clicking object references
    normalizeObjectInput?: (raw: string) => string; // Utility to convert various object formats to MOO expressions
    getDollarName?: (objId: string) => string | null; // Get $ name for an object ID
    // Window mode controls
    splitMode?: boolean; // When true, renders as embedded split component instead of modal
    onToggleSplitMode?: () => void; // Handler to toggle between split and floating modes
    isInSplitMode?: boolean; // Whether currently in split mode (for icon display)
    isTouchDevice?: boolean; // Whether this is a touch device (hides split toggle)
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
    getDollarName,
    splitMode: _splitMode = false,
    onToggleSplitMode,
    isInSplitMode = false,
    isTouchDevice = false,
}: PropertyValueEditorProps) {
    const titleBarDragProps = useTitleBarDrag();
    const [mode, setMode] = useState<EditorMode>(() => detectMode(propertyValue));
    const [value, setValue] = useState<string>(() => toEditorText(propertyValue, detectMode(propertyValue)));
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Property metadata editing state
    const [isEditingOwner, setIsEditingOwner] = useState(false);
    const [editOwnerValue, setEditOwnerValue] = useState(owner ? `#${owner}` : "");
    const [editPermissions, setEditPermissions] = useState(
        permissions || { readable: false, writable: false, chown: false },
    );
    const [isSavingMetadata, setIsSavingMetadata] = useState(false);
    const [metadataSaveSuccess, setMetadataSaveSuccess] = useState(false);

    // Sync local state when props change (after save refresh)
    useEffect(() => {
        setEditOwnerValue(owner ? `#${owner}` : "");
        setEditPermissions(permissions || { readable: false, writable: false, chown: false });
    }, [owner, permissions]);
    const FONT_SIZE_STORAGE_KEY = "moor-code-editor-font-size";
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 24;
    const clampFontSize = (size: number) => Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, size));
    const [fontSize, setFontSize] = usePersistentState<number>(
        FONT_SIZE_STORAGE_KEY,
        () => 12,
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
            const permsStr = `${editPermissions.readable ? "r" : ""}${editPermissions.writable ? "w" : ""}${
                editPermissions.chown ? "c" : ""
            }`;

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

    const handleTogglePermission = (perm: "readable" | "writable" | "chown") => {
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
        || permissions?.writable !== editPermissions.writable
        || permissions?.chown !== editPermissions.chown;

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
            className="flex-col"
            style={{
                height: "100%",
                backgroundColor: "var(--color-bg-input)",
                border: "1px solid var(--color-border-medium)",
            }}
        >
            {/* Title bar */}
            <div className="editor-title-bar" {...titleBarDragProps}>
                <h3 className="editor-title">
                    <span className="editor-title-label">
                        Property editor{hasUnsavedChanges && (
                            <span className="editor-title-indicator">
                                ●
                            </span>
                        )}
                    </span>
                    <span className="editor-title-path">
                        {propertyName} ({typeName})
                    </span>
                </h3>
                <div className="editor-toolbar">
                    {/* Remove button - only shown if onDelete handler provided */}
                    {onDelete && (
                        <button
                            onClick={onDelete}
                            aria-label="Remove property"
                            title="Remove property"
                            className="editor-delete-button"
                        >
                            Remove
                        </button>
                    )}
                    <div className="font-size-control">
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
                    {/* Mode switcher - show based on type */}
                    {propertyValue.typeCode() === VarUnion.VarStr && (
                        <>
                            <button
                                className={`btn-toggle ${mode === "input" ? "active" : ""}`}
                                onClick={() => handleModeChange("input")}
                                aria-label="Input mode"
                                title="Single-line input mode"
                            >
                                Input
                            </button>
                            <button
                                className={`btn-toggle ${mode === "text" ? "active" : ""}`}
                                onClick={() => handleModeChange("text")}
                                aria-label="Text editor mode"
                                title="Text editor mode (lines with \n)"
                            >
                                Text
                            </button>
                            <button
                                className={`btn-toggle ${mode === "literal" ? "active" : ""}`}
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
                            >
                                Literal
                            </button>
                        </>
                    )}
                    {isStringList(propertyValue) && (
                        <>
                            <button
                                className={`btn-toggle ${mode === "text" ? "active" : ""}`}
                                onClick={() => handleModeChange("text")}
                                aria-label="Text editor mode"
                                title="Text editor mode (one string per line)"
                            >
                                Text
                            </button>
                            <button
                                className={`btn-toggle ${mode === "literal" ? "active" : ""}`}
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
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
                                className={`btn-toggle ${mode === "literal" ? "active" : ""}`}
                                onClick={() => handleModeChange("literal")}
                                aria-label="MOO literal mode"
                                title="MOO literal syntax mode"
                            >
                                Literal
                            </button>
                        )}
                    {/* Save button */}
                    <button
                        className="btn editor-btn-save"
                        onClick={handleSave}
                        disabled={isSaving}
                        aria-label="Save property"
                        title="Save property"
                    >
                        {isSaving ? "💾" : "💾"}
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
                            className="editor-btn-toggle-split"
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}
                    {/* Close button */}
                    {!_splitMode && (
                        <button
                            onClick={onCancel}
                            aria-label="Close property editor"
                            className="editor-btn-close"
                        >
                            <span aria-hidden="true">×</span>
                        </button>
                    )}
                </div>
            </div>

            {/* Error message */}
            {error && (
                <div className="editor-error">
                    <pre className="editor-error-text">
                        {error}
                    </pre>
                </div>
            )}

            {/* Property metadata info panel */}
            {(owner || definer || permissions) && (
                <div className="editor-metadata-panel">
                    {/* Definer - read-only, visually separated */}
                    {definer && (
                        <div className="metadata-item metadata-definer">
                            <span className="metadata-label">
                                Definer:
                            </span>
                            {onNavigateToObject
                                ? (
                                    <button
                                        onClick={() => onNavigateToObject(definer)}
                                        className="metadata-object-button"
                                    >
                                        {(() => {
                                            const dollarName = getDollarName?.(definer);
                                            return dollarName ? `$${dollarName} / #${definer}` : `#${definer}`;
                                        })()}
                                    </button>
                                )
                                : (
                                    <span className="metadata-object-badge">
                                        {(() => {
                                            const dollarName = getDollarName?.(definer);
                                            return dollarName ? `$${dollarName} / #${definer}` : `#${definer}`;
                                        })()}
                                    </span>
                                )}
                        </div>
                    )}

                    {/* Separator bar */}
                    {definer && (owner || permissions) && <div className="metadata-separator" />}

                    {/* Owner - editable */}
                    {owner && (
                        <div className="metadata-item">
                            <span className="metadata-label">
                                Owner:
                            </span>
                            {isEditingOwner
                                ? (
                                    <input
                                        type="text"
                                        value={editOwnerValue}
                                        onChange={(e) => setEditOwnerValue(e.target.value)}
                                        className="metadata-object-input"
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
                                        className="metadata-object-button"
                                        title="Click to edit owner"
                                    >
                                        {(() => {
                                            const dollarName = getDollarName?.(owner);
                                            return dollarName ? `$${dollarName} / #${owner}` : `#${owner}`;
                                        })()}
                                    </button>
                                )}
                        </div>
                    )}

                    {/* Permissions - toggle checkboxes */}
                    {permissions && (
                        <div className="metadata-item">
                            <span className="metadata-label">
                                Perms:
                            </span>
                            <div className="metadata-permissions-group">
                                <label className="metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.readable}
                                        onChange={() => handleTogglePermission("readable")}
                                    />
                                    r
                                </label>
                                <label className="metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.writable}
                                        onChange={() => handleTogglePermission("writable")}
                                    />
                                    w
                                </label>
                                <label className="metadata-permission-label">
                                    <input
                                        type="checkbox"
                                        checked={editPermissions.chown}
                                        onChange={() => handleTogglePermission("chown")}
                                    />
                                    c
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
                                className={`metadata-action-button save ${metadataSaveSuccess ? "success" : ""} ${
                                    isSavingMetadata ? "disabled" : ""
                                }`}
                            >
                                {isSavingMetadata ? "Saving..." : metadataSaveSuccess ? "Saved ✓" : "Save"}
                            </button>
                            {hasMetadataChanges && !metadataSaveSuccess && (
                                <button
                                    onClick={() => {
                                        setEditOwnerValue(owner ? `#${owner}` : "");
                                        setEditPermissions(
                                            permissions || { readable: false, writable: false, chown: false },
                                        );
                                        setIsEditingOwner(false);
                                    }}
                                    disabled={isSavingMetadata}
                                    className="metadata-action-button cancel"
                                >
                                    Cancel
                                </button>
                            )}
                        </>
                    )}
                </div>
            )}

            {/* Editor content */}
            <div className="editor-content">
                {mode === "input" && (
                    <input
                        type="text"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        className="editor-input"
                        style={{ fontSize: `${fontSize}px` }}
                        placeholder={`Enter ${typeName} value...`}
                    />
                )}

                {mode === "text" && (
                    <div className="editor-content-wrapper">
                        <textarea
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            className="editor-textarea"
                            style={{ fontSize: `${fontSize}px` }}
                            placeholder={propertyValue.typeCode() === VarUnion.VarStr
                                ? "Enter text (newlines will be preserved as \\n)..."
                                : "Enter one string per line..."}
                        />
                        <div className="editor-mode-hint">
                            {propertyValue.typeCode() === VarUnion.VarStr
                                ? "Each line becomes part of the string with \\n separators"
                                : "Each line becomes a separate string in the list"}
                        </div>
                    </div>
                )}

                {mode === "literal" && (
                    <div className="editor-content-wrapper">
                        <textarea
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            className="editor-textarea"
                            style={{ fontSize: `${fontSize}px` }}
                            placeholder="Enter MOO literal value (e.g., {1, 2, 3} or [1 -> &quot;a&quot;, 2 -> &quot;b&quot;])..."
                        />
                        <div className="editor-mode-hint">
                            Use MOO literal syntax: {"{...}"} for lists, {"[k -> v, ...]"} for maps
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
