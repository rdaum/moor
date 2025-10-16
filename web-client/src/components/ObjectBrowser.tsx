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

import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery.js";
import { MoorVar } from "../lib/MoorVar.js";
import {
    getPropertiesFlatBuffer,
    getPropertyFlatBuffer,
    getVerbCodeFlatBuffer,
    getVerbsFlatBuffer,
    listObjectsFlatBuffer,
} from "../lib/rpc-fb.js";
import { objToString } from "../lib/var.js";
import { PropertyValueEditor } from "./PropertyValueEditor.js";
import { VerbEditor } from "./VerbEditor.js";

interface ObjectBrowserProps {
    visible: boolean;
    onClose: () => void;
    authToken: string;
    splitMode?: boolean;
    onSplitDrag?: (e: React.MouseEvent) => void;
    onSplitTouchStart?: (e: React.TouchEvent) => void;
    onToggleSplitMode?: () => void;
    isInSplitMode?: boolean;
}

interface ObjectData {
    obj: string; // Object ID as string
    name: string;
    parent: string;
    owner: string;
    flags: number;
    location: string;
    verbsCount: number;
    propertiesCount: number;
}

interface PropertyData {
    name: string;
    value: unknown; // JavaScript value from toJS()
    moorVar?: MoorVar; // Original MoorVar for proper formatting
    owner: string;
    definer: string;
    readable: boolean;
    writable: boolean;
}

interface VerbData {
    names: string[];
    owner: string;
    location: string;
    readable: boolean;
    writable: boolean;
    executable: boolean;
    dobj: number; // ArgSpec enum value
    prep: number; // PrepSpec value
    iobj: number; // ArgSpec enum value
}

// Helper to decode object flags to readable string
function formatObjectFlags(flags: number): string {
    const parts: string[] = [];
    if (flags & 8) parts.push("r"); // Read
    if (flags & 16) parts.push("w"); // Write
    if (flags & 32) parts.push("f"); // Fertile
    return parts.length > 0 ? parts.join("") : "";
}

// Helper to format ArgSpec enum value
function formatArgSpec(argSpec: number): string {
    switch (argSpec) {
        case 0:
            return "none";
        case 1:
            return "any";
        case 2:
            return "this";
        default:
            return "?";
    }
}

// Helper to format PrepSpec value (just the numeric preposition ID for now)
function formatPrepSpec(prep: number): string {
    return prep === 0 ? "none" : prep.toString();
}

export const ObjectBrowser: React.FC<ObjectBrowserProps> = ({
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
    const [objects, setObjects] = useState<ObjectData[]>([]);
    const [selectedObject, setSelectedObject] = useState<ObjectData | null>(null);
    const [properties, setProperties] = useState<PropertyData[]>([]);
    const [verbs, setVerbs] = useState<VerbData[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [filter, setFilter] = useState("");
    const [propertyFilter, setPropertyFilter] = useState("");
    const [verbFilter, setVerbFilter] = useState("");
    const [position, setPosition] = useState({ x: 50, y: 50 });
    const [size, setSize] = useState({ width: 1000, height: 700 });
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
    const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
    const containerRef = useRef<HTMLDivElement | null>(null);

    // Editor state
    const [selectedProperty, setSelectedProperty] = useState<PropertyData | null>(null);
    const [selectedVerb, setSelectedVerb] = useState<VerbData | null>(null);
    const [verbCode, setVerbCode] = useState<string>("");
    const [editorVisible, setEditorVisible] = useState(false);

    // Editor split state
    const [editorSplitPosition, setEditorSplitPosition] = useState(0.5); // 0.5 = 50% top, 50% bottom
    const [isSplitDragging, setIsSplitDragging] = useState(false);

    // Load objects on mount
    useEffect(() => {
        if (visible) {
            loadObjects();
        }
    }, [visible, authToken]);

    const loadObjects = async () => {
        setIsLoading(true);
        try {
            const reply = await listObjectsFlatBuffer(authToken);
            const objectsLength = reply.objectsLength();
            const objectList: ObjectData[] = [];

            for (let i = 0; i < objectsLength; i++) {
                const objInfo = reply.objects(i);
                if (!objInfo) continue;

                const obj = objInfo.obj();
                const name = objInfo.name();
                const parent = objInfo.parent();
                const owner = objInfo.owner();
                const location = objInfo.location();

                const objStr = objToString(obj) || "?";
                objectList.push({
                    obj: objStr,
                    name: name?.value() || "",
                    parent: objToString(parent) || "",
                    owner: objToString(owner) || "",
                    flags: objInfo.flags(),
                    location: objToString(location) || "",
                    verbsCount: objInfo.verbsCount(),
                    propertiesCount: objInfo.propertiesCount(),
                });
            }

            setObjects(objectList);
        } catch (error) {
            console.error("Failed to load objects:", error);
        } finally {
            setIsLoading(false);
        }
    };

    const loadPropertiesAndVerbs = async (obj: ObjectData) => {
        setIsLoading(true);
        try {
            // Convert obj.obj to CURIE format
            // obj.obj could be "#123" or already a CURIE like "oid:123"
            let objectCurie = obj.obj;
            if (obj.obj.startsWith("#")) {
                objectCurie = `oid:${obj.obj.substring(1)}`;
            } else if (!obj.obj.includes(":")) {
                // Just a raw number like "123"
                objectCurie = `oid:${obj.obj}`;
            }

            console.log("Loading properties/verbs for:", obj.obj, "→", objectCurie);

            // Load properties
            const propsReply = await getPropertiesFlatBuffer(authToken, objectCurie, true);
            const propsLength = propsReply.propertiesLength();
            const propList: PropertyData[] = [];

            for (let i = 0; i < propsLength; i++) {
                const propInfo = propsReply.properties(i);
                if (!propInfo) continue;

                const nameSymbol = propInfo.name();
                const definer = propInfo.definer();
                const owner = propInfo.owner();

                propList.push({
                    name: nameSymbol?.value() || "",
                    value: null, // TODO: Fetch actual property value
                    owner: objToString(owner) || "",
                    definer: objToString(definer) || "",
                    readable: propInfo.r(),
                    writable: propInfo.w(),
                });
            }

            setProperties(propList);

            // Load verbs
            const verbsReply = await getVerbsFlatBuffer(authToken, objectCurie, true);
            const verbsLength = verbsReply.verbsLength();
            const verbList: VerbData[] = [];

            for (let i = 0; i < verbsLength; i++) {
                const verbInfo = verbsReply.verbs(i);
                if (!verbInfo) continue;

                const namesLength = verbInfo.namesLength();
                const names: string[] = [];
                for (let j = 0; j < namesLength; j++) {
                    const nameSymbol = verbInfo.names(j);
                    const name = nameSymbol?.value();
                    if (name) {
                        names.push(name);
                    }
                }

                const location = verbInfo.location();
                const owner = verbInfo.owner();

                // arg_spec is a vector of 3 symbols: [dobj, prep, iobj]
                const argSpecLength = verbInfo.argSpecLength();
                const dobjStr = argSpecLength > 0 ? verbInfo.argSpec(0)?.value() || "none" : "none";
                const prepStr = argSpecLength > 1 ? verbInfo.argSpec(1)?.value() || "none" : "none";
                const iobjStr = argSpecLength > 2 ? verbInfo.argSpec(2)?.value() || "none" : "none";

                // Convert dobj/iobj strings to numbers for storage
                const dobjNum = dobjStr === "this" ? 2 : (dobjStr === "any" ? 1 : 0);
                const iobjNum = iobjStr === "this" ? 2 : (iobjStr === "any" ? 1 : 0);

                verbList.push({
                    names,
                    owner: objToString(owner) || "",
                    location: objToString(location) || "",
                    readable: verbInfo.r(),
                    writable: verbInfo.w(),
                    executable: verbInfo.x(),
                    dobj: dobjNum,
                    prep: parseInt(prepStr) || 0, // prep is a numeric preposition ID
                    iobj: iobjNum,
                });
            }

            setVerbs(verbList);
        } catch (error) {
            console.error("Failed to load properties/verbs:", error);
        } finally {
            setIsLoading(false);
        }
    };

    const handleObjectSelect = (obj: ObjectData) => {
        setSelectedObject(obj);
        setSelectedProperty(null);
        setSelectedVerb(null);
        setEditorVisible(false);
        loadPropertiesAndVerbs(obj);
    };

    const handlePropertySelect = async (prop: PropertyData) => {
        setSelectedProperty(prop);
        setSelectedVerb(null);
        setEditorVisible(true);

        // Fetch property value from the object where the property is defined (prop.definer)
        if (!selectedObject) return;

        try {
            const objectCurie = prop.definer.includes(":")
                ? prop.definer
                : `oid:${prop.definer}`;
            const propValue = await getPropertyFlatBuffer(authToken, objectCurie, prop.name);
            const varValue = propValue.value();
            if (varValue) {
                const moorVar = new MoorVar(varValue);
                const jsValue = moorVar.toJS();
                // Update the property with both JS value and MoorVar
                setSelectedProperty({ ...prop, value: jsValue, moorVar });
                console.log(`Property ${prop.name} value:`, jsValue);
            }
        } catch (error) {
            console.error("Failed to load property value:", error);
        }
    };

    const handleVerbSelect = async (verb: VerbData) => {
        setSelectedVerb(verb);
        setSelectedProperty(null);
        setEditorVisible(true);

        // Fetch verb code from the object where the verb is defined (verb.location)
        try {
            const objectCurie = verb.location.includes(":")
                ? verb.location
                : `oid:${verb.location}`;
            const verbValue = await getVerbCodeFlatBuffer(authToken, objectCurie, verb.names[0]);
            const codeLength = verbValue.codeLength();
            const lines: string[] = [];
            for (let i = 0; i < codeLength; i++) {
                const line = verbValue.code(i);
                if (line) lines.push(line);
            }
            setVerbCode(lines.join("\n"));
        } catch (error) {
            console.error("Failed to load verb code:", error);
            setVerbCode("// Failed to load verb code");
        }
    };

    // Mouse event handlers for dragging
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

            const newWidth = Math.max(600, resizeStart.width + deltaX);
            const newHeight = Math.max(400, resizeStart.height + deltaY);

            setSize({ width: newWidth, height: newHeight });
        } else if (isSplitDragging && containerRef.current) {
            const rect = containerRef.current.getBoundingClientRect();
            const relativeY = e.clientY - rect.top;
            const containerHeight = rect.height;
            const newPosition = Math.max(0.2, Math.min(0.8, relativeY / containerHeight));
            setEditorSplitPosition(newPosition);
        }
    }, [isDragging, isResizing, isSplitDragging, dragStart, resizeStart, size]);

    const handleMouseUp = useCallback(() => {
        setIsDragging(false);
        setIsResizing(false);
        setIsSplitDragging(false);
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

    // Group properties by definer
    const groupedProperties = React.useMemo(() => {
        const filterLower = propertyFilter.toLowerCase();
        const filteredProps = properties.filter(prop => prop.name.toLowerCase().includes(filterLower));

        const groups = new Map<string, PropertyData[]>();
        for (const prop of filteredProps) {
            const definer = prop.definer;
            if (!groups.has(definer)) {
                groups.set(definer, []);
            }
            groups.get(definer)!.push(prop);
        }
        return Array.from(groups.entries()).sort((a, b) => {
            // Sort by definer ID (current object first)
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            return a[0].localeCompare(b[0]);
        });
    }, [properties, selectedObject, propertyFilter]);

    // Group verbs by location
    const groupedVerbs = React.useMemo(() => {
        const filterLower = verbFilter.toLowerCase();
        const filteredVerbs = verbs.filter(verb => verb.names.some(name => name.toLowerCase().includes(filterLower)));

        const groups = new Map<string, VerbData[]>();
        for (const verb of filteredVerbs) {
            const location = verb.location;
            if (!groups.has(location)) {
                groups.set(location, []);
            }
            groups.get(location)!.push(verb);
        }
        return Array.from(groups.entries()).sort((a, b) => {
            // Sort by location ID (current object first)
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            return a[0].localeCompare(b[0]);
        });
    }, [verbs, selectedObject, verbFilter]);

    // Add global mouse event listeners
    useEffect(() => {
        if (isDragging || isResizing || isSplitDragging) {
            document.addEventListener("mousemove", handleMouseMove);
            document.addEventListener("mouseup", handleMouseUp);
            document.body.style.userSelect = "none";

            return () => {
                document.removeEventListener("mousemove", handleMouseMove);
                document.removeEventListener("mouseup", handleMouseUp);
                document.body.style.userSelect = "";
            };
        }
    }, [isDragging, isResizing, isSplitDragging, handleMouseMove, handleMouseUp]);

    if (!visible) {
        return null;
    }

    // Helper to check if object ID is UUID-based
    const isUuidObject = (objId: string): boolean => {
        return objId.includes("-");
    };

    // Filter and group objects by type
    const filteredObjects = objects
        .filter(obj =>
            obj.name.toLowerCase().includes(filter.toLowerCase())
            || obj.obj.includes(filter)
        );

    // Separate numeric OIDs from UUIDs
    const numericObjects = filteredObjects
        .filter(obj => !isUuidObject(obj.obj))
        .sort((a, b) => {
            // Sort by object ID numerically
            const aNum = parseInt(a.obj);
            const bNum = parseInt(b.obj);
            return aNum - bNum;
        });

    const uuidObjects = filteredObjects
        .filter(obj => isUuidObject(obj.obj))
        .sort((a, b) => a.obj.localeCompare(b.obj));

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

    return (
        <div
            ref={containerRef}
            className="object_browser_container"
            role={splitMode ? "region" : "dialog"}
            aria-modal={splitMode ? undefined : "true"}
            aria-labelledby="object-browser-title"
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
                    touchAction: splitMode ? "none" : "auto",
                }}
            >
                <h3
                    id="object-browser-title"
                    style={{
                        margin: 0,
                        color: "var(--color-text-primary)",
                        fontWeight: "700",
                    }}
                >
                    Object Browser
                </h3>
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
                    {/* Split/Float toggle button - only on desktop */}
                    {!isMobile && onToggleSplitMode && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
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
                        aria-label="Close object browser"
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

            {/* Main content area - 3 panes + editor */}
            <div
                style={{
                    flex: 1,
                    display: "flex",
                    flexDirection: "column",
                    overflow: "hidden",
                }}
            >
                {/* Top area - 3 panes */}
                <div
                    style={{
                        flex: editorVisible ? editorSplitPosition : 1,
                        display: "flex",
                        overflow: "hidden",
                    }}
                >
                    {/* Objects pane */}
                    <div
                        style={{
                            width: "33.33%",
                            borderRight: "1px solid var(--color-border-light)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                        }}
                    >
                        <div
                            style={{
                                padding: "var(--space-sm)",
                                borderBottom: "1px solid var(--color-border-light)",
                                backgroundColor: "var(--color-bg-secondary)",
                            }}
                        >
                            <input
                                type="text"
                                placeholder="Filter objects..."
                                value={filter}
                                onChange={(e) => setFilter(e.target.value)}
                                style={{
                                    width: "100%",
                                    padding: "var(--space-xs)",
                                    backgroundColor: "var(--color-bg-input)",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    color: "var(--color-text-primary)",
                                    fontSize: "12px",
                                }}
                            />
                        </div>
                        <div
                            style={{
                                flex: 1,
                                overflowY: "auto",
                                fontSize: "12px",
                            }}
                        >
                            {isLoading
                                ? (
                                    <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                        Loading objects...
                                    </div>
                                )
                                : (
                                    <>
                                        {/* Numeric OID objects */}
                                        {numericObjects.map((obj) => (
                                            <div
                                                key={obj.obj}
                                                onClick={() => handleObjectSelect(obj)}
                                                style={{
                                                    padding: "var(--space-xs) var(--space-sm)",
                                                    cursor: "pointer",
                                                    backgroundColor: selectedObject?.obj === obj.obj
                                                        ? "var(--color-text-primary)"
                                                        : "transparent",
                                                    color: selectedObject?.obj === obj.obj
                                                        ? "var(--color-bg-input)"
                                                        : "inherit",
                                                    borderBottom: "1px solid var(--color-border-light)",
                                                    fontFamily: "var(--font-mono)",
                                                }}
                                                onMouseEnter={(e) => {
                                                    if (selectedObject?.obj !== obj.obj) {
                                                        e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                                                    }
                                                }}
                                                onMouseLeave={(e) => {
                                                    if (selectedObject?.obj !== obj.obj) {
                                                        e.currentTarget.style.backgroundColor = "transparent";
                                                    }
                                                }}
                                            >
                                                <div style={{ fontWeight: "600" }}>
                                                    #{obj.obj} {obj.name && `("${obj.name}")`}{" "}
                                                    {formatObjectFlags(obj.flags) && (
                                                        <span
                                                            style={{
                                                                opacity: selectedObject?.obj === obj.obj ? "0.7" : "1",
                                                                color: selectedObject?.obj === obj.obj
                                                                    ? "inherit"
                                                                    : "var(--color-text-secondary)",
                                                                fontWeight: "400",
                                                            }}
                                                        >
                                                            ({formatObjectFlags(obj.flags)})
                                                        </span>
                                                    )}
                                                </div>
                                            </div>
                                        ))}

                                        {/* Separator and UUID objects section */}
                                        {uuidObjects.length > 0 && (
                                            <>
                                                <div
                                                    style={{
                                                        padding: "var(--space-xs) var(--space-sm)",
                                                        backgroundColor: "var(--color-bg-secondary)",
                                                        borderTop: "2px solid var(--color-border-medium)",
                                                        borderBottom: "1px solid var(--color-border-light)",
                                                        fontSize: "11px",
                                                        fontWeight: "600",
                                                        color: "var(--color-text-secondary)",
                                                        fontFamily: "var(--font-mono)",
                                                    }}
                                                >
                                                    UUID Objects
                                                </div>
                                                {uuidObjects.map((obj) => (
                                                    <div
                                                        key={obj.obj}
                                                        onClick={() => handleObjectSelect(obj)}
                                                        style={{
                                                            padding: "var(--space-xs) var(--space-sm)",
                                                            cursor: "pointer",
                                                            backgroundColor: selectedObject?.obj === obj.obj
                                                                ? "var(--color-text-primary)"
                                                                : "transparent",
                                                            color: selectedObject?.obj === obj.obj
                                                                ? "var(--color-bg-input)"
                                                                : "inherit",
                                                            borderBottom: "1px solid var(--color-border-light)",
                                                            fontFamily: "var(--font-mono)",
                                                        }}
                                                        onMouseEnter={(e) => {
                                                            if (selectedObject?.obj !== obj.obj) {
                                                                e.currentTarget.style.backgroundColor =
                                                                    "var(--color-bg-hover)";
                                                            }
                                                        }}
                                                        onMouseLeave={(e) => {
                                                            if (selectedObject?.obj !== obj.obj) {
                                                                e.currentTarget.style.backgroundColor = "transparent";
                                                            }
                                                        }}
                                                    >
                                                        <div style={{ fontWeight: "600" }}>
                                                            #{obj.obj} {obj.name && `("${obj.name}")`}{" "}
                                                            {formatObjectFlags(obj.flags) && (
                                                                <span
                                                                    style={{
                                                                        opacity: selectedObject?.obj === obj.obj
                                                                            ? "0.7"
                                                                            : "1",
                                                                        color: selectedObject?.obj === obj.obj
                                                                            ? "inherit"
                                                                            : "var(--color-text-secondary)",
                                                                        fontWeight: "400",
                                                                    }}
                                                                >
                                                                    ({formatObjectFlags(obj.flags)})
                                                                </span>
                                                            )}
                                                        </div>
                                                    </div>
                                                ))}
                                            </>
                                        )}
                                    </>
                                )}
                        </div>
                        {/* Object info panel */}
                        {selectedObject && (
                            <div
                                style={{
                                    padding: "var(--space-sm)",
                                    borderTop: "1px solid var(--color-border-light)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    fontSize: "11px",
                                    fontFamily: "var(--font-mono)",
                                    color: "var(--color-text-secondary)",
                                    minHeight: "100px",
                                }}
                            >
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Flags:</strong> {formatObjectFlags(selectedObject.flags) || "none"}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Parent:</strong> #{selectedObject.parent || "none"}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Owner:</strong> #{selectedObject.owner}
                                </div>
                                <div>
                                    <strong>Location:</strong> #{selectedObject.location || "none"}
                                </div>
                            </div>
                        )}
                    </div>

                    {/* Properties pane */}
                    <div
                        style={{
                            width: "33.33%",
                            borderRight: "1px solid var(--color-border-light)",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                        }}
                    >
                        <div
                            style={{
                                padding: "var(--space-sm)",
                                borderBottom: "1px solid var(--color-border-light)",
                                backgroundColor: "var(--color-bg-secondary)",
                            }}
                        >
                            <input
                                type="text"
                                placeholder="Filter properties..."
                                value={propertyFilter}
                                onChange={(e) => setPropertyFilter(e.target.value)}
                                style={{
                                    width: "100%",
                                    padding: "var(--space-xs)",
                                    backgroundColor: "var(--color-bg-input)",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    color: "var(--color-text-primary)",
                                    fontSize: "12px",
                                }}
                            />
                        </div>
                        <div
                            style={{
                                flex: 1,
                                overflowY: "auto",
                                fontSize: "12px",
                            }}
                        >
                            {!selectedObject
                                ? (
                                    <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                        Select an object to view properties
                                    </div>
                                )
                                : properties.length === 0
                                ? (
                                    <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                        No properties
                                    </div>
                                )
                                : (
                                    groupedProperties.map(([definer, props], groupIdx) => (
                                        <div key={definer}>
                                            {groupIdx > 0 && (
                                                <div
                                                    style={{
                                                        padding: "var(--space-xs) var(--space-sm)",
                                                        backgroundColor: "var(--color-bg-secondary)",
                                                        borderTop: "2px solid var(--color-border-medium)",
                                                        borderBottom: "1px solid var(--color-border-light)",
                                                        fontSize: "11px",
                                                        fontWeight: "600",
                                                        color: "var(--color-text-secondary)",
                                                        fontFamily: "var(--font-mono)",
                                                    }}
                                                >
                                                    from #{definer}
                                                </div>
                                            )}
                                            {props.map((prop, idx) => (
                                                <div
                                                    key={`${definer}-${idx}`}
                                                    onClick={() => handlePropertySelect(prop)}
                                                    style={{
                                                        padding: "var(--space-xs) var(--space-sm)",
                                                        cursor: "pointer",
                                                        backgroundColor: selectedProperty?.name === prop.name
                                                            ? "var(--color-text-primary)"
                                                            : "transparent",
                                                        color: selectedProperty?.name === prop.name
                                                            ? "var(--color-bg-input)"
                                                            : "inherit",
                                                        borderBottom: "1px solid var(--color-border-light)",
                                                        fontFamily: "var(--font-mono)",
                                                    }}
                                                    onMouseEnter={(e) => {
                                                        if (selectedProperty?.name !== prop.name) {
                                                            e.currentTarget.style.backgroundColor =
                                                                "var(--color-bg-hover)";
                                                        }
                                                    }}
                                                    onMouseLeave={(e) => {
                                                        if (selectedProperty?.name !== prop.name) {
                                                            e.currentTarget.style.backgroundColor = "transparent";
                                                        }
                                                    }}
                                                >
                                                    <div style={{ fontWeight: "600" }}>
                                                        {prop.name}{" "}
                                                        <span
                                                            style={{
                                                                opacity: selectedProperty?.name === prop.name
                                                                    ? "0.7"
                                                                    : "1",
                                                                color: selectedProperty?.name === prop.name
                                                                    ? "inherit"
                                                                    : "var(--color-text-secondary)",
                                                                fontWeight: "400",
                                                                fontSize: "11px",
                                                            }}
                                                        >
                                                            ({prop.readable ? "r" : ""}
                                                            {prop.writable ? "w" : ""})
                                                        </span>
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    ))
                                )}
                        </div>
                        {/* Property info panel */}
                        {selectedProperty && (
                            <div
                                style={{
                                    padding: "var(--space-sm)",
                                    borderTop: "1px solid var(--color-border-light)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    fontSize: "11px",
                                    fontFamily: "var(--font-mono)",
                                    color: "var(--color-text-secondary)",
                                    minHeight: "100px",
                                    maxHeight: "150px",
                                    overflowY: "auto",
                                }}
                            >
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Owner:</strong> #{selectedProperty.owner}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Definer:</strong> #{selectedProperty.definer}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Perms:</strong> {selectedProperty.readable ? "r" : ""}
                                    {selectedProperty.writable ? "w" : ""}
                                </div>
                                {selectedProperty.moorVar && (
                                    <div
                                        style={{
                                            marginTop: "var(--space-xs)",
                                            paddingTop: "var(--space-xs)",
                                            borderTop: "1px solid var(--color-border-light)",
                                        }}
                                    >
                                        <strong>Value:</strong>
                                        <div
                                            style={{
                                                marginTop: "2px",
                                                wordBreak: "break-word",
                                                maxHeight: "60px",
                                                overflowY: "auto",
                                            }}
                                        >
                                            {selectedProperty.moorVar.toLiteral()}
                                        </div>
                                    </div>
                                )}
                            </div>
                        )}
                    </div>

                    {/* Verbs pane */}
                    <div
                        style={{
                            width: "33.33%",
                            display: "flex",
                            flexDirection: "column",
                            overflow: "hidden",
                        }}
                    >
                        <div
                            style={{
                                padding: "var(--space-sm)",
                                borderBottom: "1px solid var(--color-border-light)",
                                backgroundColor: "var(--color-bg-secondary)",
                            }}
                        >
                            <input
                                type="text"
                                placeholder="Filter verbs..."
                                value={verbFilter}
                                onChange={(e) => setVerbFilter(e.target.value)}
                                style={{
                                    width: "100%",
                                    padding: "var(--space-xs)",
                                    backgroundColor: "var(--color-bg-input)",
                                    border: "1px solid var(--color-border-medium)",
                                    borderRadius: "var(--radius-sm)",
                                    color: "var(--color-text-primary)",
                                    fontSize: "12px",
                                }}
                            />
                        </div>
                        <div
                            style={{
                                flex: 1,
                                overflowY: "auto",
                                fontSize: "12px",
                            }}
                        >
                            {!selectedObject
                                ? (
                                    <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                        Select an object to view verbs
                                    </div>
                                )
                                : verbs.length === 0
                                ? (
                                    <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                        No verbs
                                    </div>
                                )
                                : (
                                    groupedVerbs.map(([location, verbList], groupIdx) => (
                                        <div key={location}>
                                            {groupIdx > 0 && (
                                                <div
                                                    style={{
                                                        padding: "var(--space-xs) var(--space-sm)",
                                                        backgroundColor: "var(--color-bg-secondary)",
                                                        borderTop: "2px solid var(--color-border-medium)",
                                                        borderBottom: "1px solid var(--color-border-light)",
                                                        fontSize: "11px",
                                                        fontWeight: "600",
                                                        color: "var(--color-text-secondary)",
                                                        fontFamily: "var(--font-mono)",
                                                    }}
                                                >
                                                    from #{location}
                                                </div>
                                            )}
                                            {verbList.map((verb, idx) => (
                                                <div
                                                    key={`${location}-${idx}`}
                                                    onClick={() => handleVerbSelect(verb)}
                                                    style={{
                                                        padding: "var(--space-xs) var(--space-sm)",
                                                        cursor: "pointer",
                                                        backgroundColor: selectedVerb?.names[0] === verb.names[0]
                                                            ? "var(--color-text-primary)"
                                                            : "transparent",
                                                        color: selectedVerb?.names[0] === verb.names[0]
                                                            ? "var(--color-bg-input)"
                                                            : "inherit",
                                                        borderBottom: "1px solid var(--color-border-light)",
                                                        fontFamily: "var(--font-mono)",
                                                    }}
                                                    onMouseEnter={(e) => {
                                                        if (selectedVerb?.names[0] !== verb.names[0]) {
                                                            e.currentTarget.style.backgroundColor =
                                                                "var(--color-bg-hover)";
                                                        }
                                                    }}
                                                    onMouseLeave={(e) => {
                                                        if (selectedVerb?.names[0] !== verb.names[0]) {
                                                            e.currentTarget.style.backgroundColor = "transparent";
                                                        }
                                                    }}
                                                >
                                                    <div style={{ fontWeight: "600" }}>
                                                        {verb.names.join(" ")}{" "}
                                                        <span
                                                            style={{
                                                                opacity: selectedVerb?.names[0] === verb.names[0]
                                                                    ? "0.7"
                                                                    : "1",
                                                                color: selectedVerb?.names[0] === verb.names[0]
                                                                    ? "inherit"
                                                                    : "var(--color-text-secondary)",
                                                                fontWeight: "400",
                                                                fontSize: "11px",
                                                            }}
                                                        >
                                                            ({verb.readable ? "r" : ""}
                                                            {verb.writable ? "w" : ""}
                                                            {verb.executable ? "x" : ""})
                                                        </span>
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    ))
                                )}
                        </div>
                        {/* Verb info panel */}
                        {selectedVerb && (
                            <div
                                style={{
                                    padding: "var(--space-sm)",
                                    borderTop: "1px solid var(--color-border-light)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    fontSize: "11px",
                                    fontFamily: "var(--font-mono)",
                                    color: "var(--color-text-secondary)",
                                    minHeight: "100px",
                                }}
                            >
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Names:</strong> {selectedVerb.names.join(" ")}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Args:</strong> {formatArgSpec(selectedVerb.dobj)} /{" "}
                                    {formatPrepSpec(selectedVerb.prep)} / {formatArgSpec(selectedVerb.iobj)}
                                </div>
                                <div style={{ marginBottom: "var(--space-xs)" }}>
                                    <strong>Owner:</strong> #{selectedVerb.owner}
                                </div>
                                <div>
                                    <strong>Perms:</strong> {selectedVerb.readable ? "r" : ""}
                                    {selectedVerb.writable ? "w" : ""}
                                    {selectedVerb.executable ? "x" : ""}
                                </div>
                            </div>
                        )}
                    </div>
                </div>

                {/* Draggable splitter bar */}
                {editorVisible && (
                    <div
                        onMouseDown={handleSplitDragStart}
                        style={{
                            height: "4px",
                            backgroundColor: "var(--color-border-medium)",
                            cursor: "row-resize",
                            position: "relative",
                            zIndex: 10,
                        }}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.backgroundColor = "var(--color-text-primary)";
                        }}
                        onMouseLeave={(e) => {
                            if (!isSplitDragging) {
                                e.currentTarget.style.backgroundColor = "var(--color-border-medium)";
                            }
                        }}
                    />
                )}

                {/* Bottom editor area */}
                {editorVisible && (
                    <div
                        style={{
                            flex: 1 - editorSplitPosition,
                            overflow: "hidden",
                            backgroundColor: "var(--color-bg-secondary)",
                        }}
                    >
                        {selectedProperty && selectedProperty.moorVar && selectedObject && (
                            <PropertyValueEditor
                                authToken={authToken}
                                objectCurie={selectedProperty.definer.includes(":")
                                    ? selectedProperty.definer
                                    : `oid:${selectedProperty.definer}`}
                                propertyName={selectedProperty.name}
                                propertyValue={selectedProperty.moorVar}
                                onSave={() => {
                                    // Reload property value after save
                                    handlePropertySelect(selectedProperty);
                                }}
                                onCancel={() => {
                                    setSelectedProperty(null);
                                    setEditorVisible(false);
                                }}
                            />
                        )}
                        {selectedProperty && !selectedProperty.moorVar && (
                            <div style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}>
                                Loading property value...
                            </div>
                        )}
                        {selectedVerb && (
                            <VerbEditor
                                visible={true}
                                onClose={() => {
                                    setSelectedVerb(null);
                                    setEditorVisible(false);
                                }}
                                title={`#${selectedVerb.location}:${selectedVerb.names.join(" ")}${
                                    selectedObject && selectedVerb.location !== selectedObject.obj
                                        ? ` (inherited from #${selectedVerb.location})`
                                        : ""
                                }`}
                                objectCurie={selectedVerb.location.includes(":")
                                    ? selectedVerb.location
                                    : `oid:${selectedVerb.location}`}
                                verbName={selectedVerb.names[0]}
                                initialContent={verbCode}
                                authToken={authToken}
                                splitMode={true}
                            />
                        )}
                    </div>
                )}
            </div>

            {/* Resize handle - only in modal mode */}
            {!splitMode && (
                <div
                    onMouseDown={handleResizeMouseDown}
                    tabIndex={0}
                    role="button"
                    aria-label="Resize browser window"
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
