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
    fetchServerFeatures,
    getPropertiesFlatBuffer,
    getPropertyFlatBuffer,
    getVerbCodeFlatBuffer,
    getVerbsFlatBuffer,
    listObjectsFlatBuffer,
    performEvalFlatBuffer,
} from "../lib/rpc-fb.js";
import type { ServerFeatureSet } from "../lib/rpc-fb.js";
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

interface CreateChildFormValues {
    owner: string;
    objectType: string;
    initArgs: string;
}

interface AddPropertyFormValues {
    name: string;
    value: string;
    owner: string;
    perms: string;
}

interface AddVerbFormValues {
    names: string;
    owner: string;
    perms: string;
    dobj: string;
    prep: string;
    iobj: string;
}

// Helper to convert ArgSpec enum to string
function argSpecToString(val: number): string {
    switch (val) {
        case 0:
            return "none";
        case 1:
            return "any";
        case 2:
            return "this";
        default:
            return "none";
    }
}

// Helper to convert PrepSpec value to string
function prepSpecToString(val: number): string {
    if (val === -2) return "any";
    if (val === -1) return "none";
    // Numeric preposition IDs map to specific prepositions
    const preps = [
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
    ];
    return preps[val] || "none";
}

// Helper to decode object flags to readable string
function formatObjectFlags(flags: number): string {
    const parts: string[] = [];
    if (flags & (1 << 0)) parts.push("u"); // User (player)
    if (flags & (1 << 1)) parts.push("p"); // Programmer
    if (flags & (1 << 2)) parts.push("w"); // Wizard
    if (flags & (1 << 4)) parts.push("r"); // Readable
    if (flags & (1 << 5)) parts.push("W"); // Writable (capital W to distinguish from wizard)
    if (flags & (1 << 7)) parts.push("f"); // Fertile
    return parts.length > 0 ? parts.join("") : "";
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

    // Sync selectedVerb when verbs array updates (e.g., after metadata save)
    useEffect(() => {
        if (selectedVerb) {
            const updatedVerb = verbs.find(v =>
                v.names[0] === selectedVerb.names[0] && v.location === selectedVerb.location
            );
            if (updatedVerb) {
                setSelectedVerb(updatedVerb);
            }
        }
    }, [verbs]); // eslint-disable-line react-hooks/exhaustive-deps

    // Editor split state
    const [editorSplitPosition, setEditorSplitPosition] = useState(0.5); // 0.5 = 50% top, 50% bottom
    const [isSplitDragging, setIsSplitDragging] = useState(false);
    const MIN_FONT_SIZE = 10;
    const MAX_FONT_SIZE = 20;
    const [fontSize, setFontSize] = useState(() => {
        const fallback = isMobile ? 14 : 12;
        if (typeof window === "undefined") {
            return fallback;
        }
        const stored = window.localStorage.getItem("moor-object-browser-font-size");
        if (!stored) {
            return fallback;
        }
        const parsed = parseInt(stored, 10);
        if (!Number.isFinite(parsed)) {
            return fallback;
        }
        return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, parsed));
    });
    const [showInheritedProperties, setShowInheritedProperties] = useState(() => {
        if (typeof window === "undefined") {
            return true;
        }
        const stored = window.localStorage.getItem("moor-object-browser-show-inherited-properties");
        return stored !== "false";
    });
    const [showInheritedVerbs, setShowInheritedVerbs] = useState(() => {
        if (typeof window === "undefined") {
            return true;
        }
        const stored = window.localStorage.getItem("moor-object-browser-show-inherited-verbs");
        return stored !== "false";
    });
    const [serverFeatures, setServerFeatures] = useState<ServerFeatureSet | null>(null);
    const [showCreateDialog, setShowCreateDialog] = useState(false);
    const [showRecycleDialog, setShowRecycleDialog] = useState(false);
    const [showAddPropertyDialog, setShowAddPropertyDialog] = useState(false);
    const [showDeletePropertyDialog, setShowDeletePropertyDialog] = useState(false);
    const [showAddVerbDialog, setShowAddVerbDialog] = useState(false);
    const [showDeleteVerbDialog, setShowDeleteVerbDialog] = useState(false);
    const [showEditFlagsDialog, setShowEditFlagsDialog] = useState(false);
    const [isSubmittingCreate, setIsSubmittingCreate] = useState(false);
    const [isSubmittingRecycle, setIsSubmittingRecycle] = useState(false);
    const [isSubmittingAddProperty, setIsSubmittingAddProperty] = useState(false);
    const [isSubmittingDeleteProperty, setIsSubmittingDeleteProperty] = useState(false);
    const [isSubmittingAddVerb, setIsSubmittingAddVerb] = useState(false);
    const [isSubmittingDeleteVerb, setIsSubmittingDeleteVerb] = useState(false);
    const [isSubmittingEditFlags, setIsSubmittingEditFlags] = useState(false);
    const [createDialogError, setCreateDialogError] = useState<string | null>(null);
    const [recycleDialogError, setRecycleDialogError] = useState<string | null>(null);
    const [addPropertyDialogError, setAddPropertyDialogError] = useState<string | null>(null);
    const [deletePropertyDialogError, setDeletePropertyDialogError] = useState<string | null>(null);
    const [addVerbDialogError, setAddVerbDialogError] = useState<string | null>(null);
    const [deleteVerbDialogError, setDeleteVerbDialogError] = useState<string | null>(null);
    const [editFlagsDialogError, setEditFlagsDialogError] = useState<string | null>(null);
    const [actionMessage, setActionMessage] = useState<string | null>(null);
    const [editingName, setEditingName] = useState<string>("");
    const [isSavingName, setIsSavingName] = useState(false);
    const [propertyToDelete, setPropertyToDelete] = useState<PropertyData | null>(null);
    const [verbToDelete, setVerbToDelete] = useState<VerbData | null>(null);
    const decreaseFontSize = useCallback(() => {
        setFontSize(prev => Math.max(MIN_FONT_SIZE, prev - 1));
    }, []);
    const increaseFontSize = useCallback(() => {
        setFontSize(prev => Math.min(MAX_FONT_SIZE, prev + 1));
    }, []);

    // Load objects on mount
    useEffect(() => {
        if (visible) {
            loadObjects();
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [visible, authToken]);

    useEffect(() => {
        if (!visible) {
            return;
        }
        let cancelled = false;
        fetchServerFeatures()
            .then((features) => {
                if (!cancelled) {
                    setServerFeatures(features);
                }
            })
            .catch((error) => {
                console.error("Failed to fetch server features:", error);
            });
        return () => {
            cancelled = true;
        };
    }, [visible]);

    useEffect(() => {
        if (!visible) {
            setShowCreateDialog(false);
            setShowRecycleDialog(false);
            setShowAddPropertyDialog(false);
            setShowDeletePropertyDialog(false);
        }
    }, [visible]);

    const loadObjects = async (): Promise<ObjectData[]> => {
        setIsLoading(true);
        let objectList: ObjectData[] = [];
        try {
            const reply = await listObjectsFlatBuffer(authToken);
            const objectsLength = reply.objectsLength();
            const result: ObjectData[] = [];

            for (let i = 0; i < objectsLength; i++) {
                const objInfo = reply.objects(i);
                if (!objInfo) continue;

                const obj = objInfo.obj();
                const name = objInfo.name();
                const parent = objInfo.parent();
                const owner = objInfo.owner();
                const location = objInfo.location();

                const objStr = objToString(obj) || "?";
                result.push({
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

            objectList = result;
            setObjects(result);
        } catch (error) {
            console.error("Failed to load objects:", error);
        } finally {
            setIsLoading(false);
        }
        return objectList;
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
            return propList; // Return the property list for immediate use
        } catch (error) {
            console.error("Failed to load properties/verbs:", error);
            return []; // Return empty array on error
        } finally {
            setIsLoading(false);
        }
    };

    const handleObjectSelect = (obj: ObjectData) => {
        setActionMessage(null);
        setSelectedObject(obj);
        setEditingName(obj.name);
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

    const handleNameSave = async () => {
        if (!selectedObject) return;

        setIsSavingName(true);
        try {
            const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
            if (!objectExpr || objectExpr === "#-1") {
                throw new Error("Invalid object reference");
            }

            // Escape the name string for MOO
            const escapedName = editingName.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
            const expr = `${objectExpr}.name = "${escapedName}"; return ${objectExpr}.name;`;

            await performEvalFlatBuffer(authToken, expr);

            // Update local state
            setSelectedObject({ ...selectedObject, name: editingName });

            // Reload the objects list to reflect the change
            const updated = await loadObjects();
            const updatedObj = updated.find(obj => obj.obj === selectedObject.obj);
            if (updatedObj) {
                setSelectedObject(updatedObj);
                setEditingName(updatedObj.name);
            }

            setActionMessage("Name updated successfully");
            setTimeout(() => setActionMessage(null), 3000);
        } catch (error) {
            console.error("Failed to save name:", error);
            setActionMessage("Failed to update name: " + (error instanceof Error ? error.message : String(error)));
            setTimeout(() => setActionMessage(null), 5000);
        } finally {
            setIsSavingName(false);
        }
    };

    const handleEditFlagsSubmit = async (flags: number) => {
        if (!selectedObject) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr) {
            setEditFlagsDialogError("Unable to determine object reference.");
            return;
        }

        // Extract new flag values
        const newUser = (flags & (1 << 0)) !== 0 ? 1 : 0;
        const newProgrammer = (flags & (1 << 1)) !== 0 ? 1 : 0;
        const newWizard = (flags & (1 << 2)) !== 0 ? 1 : 0;
        const newReadable = (flags & (1 << 4)) !== 0 ? 1 : 0;
        const newWritable = (flags & (1 << 5)) !== 0 ? 1 : 0;
        const newFertile = (flags & (1 << 7)) !== 0 ? 1 : 0;

        // Extract current flag values
        const currentUser = (selectedObject.flags & (1 << 0)) !== 0 ? 1 : 0;
        const currentProgrammer = (selectedObject.flags & (1 << 1)) !== 0 ? 1 : 0;
        const currentWizard = (selectedObject.flags & (1 << 2)) !== 0 ? 1 : 0;
        const currentReadable = (selectedObject.flags & (1 << 4)) !== 0 ? 1 : 0;
        const currentWritable = (selectedObject.flags & (1 << 5)) !== 0 ? 1 : 0;
        const currentFertile = (selectedObject.flags & (1 << 7)) !== 0 ? 1 : 0;

        // Build expression only for changed flags
        const assignments: string[] = [];
        if (newProgrammer !== currentProgrammer) {
            assignments.push(`${objectExpr}.programmer = ${newProgrammer}`);
        }
        if (newWizard !== currentWizard) {
            assignments.push(`${objectExpr}.wizard = ${newWizard}`);
        }
        if (newReadable !== currentReadable) {
            assignments.push(`${objectExpr}.r = ${newReadable}`);
        }
        if (newWritable !== currentWritable) {
            assignments.push(`${objectExpr}.w = ${newWritable}`);
        }
        if (newFertile !== currentFertile) {
            assignments.push(`${objectExpr}.f = ${newFertile}`);
        }

        // If nothing changed, just close the dialog
        if (assignments.length === 0 && newUser === currentUser) {
            setShowEditFlagsDialog(false);
            return;
        }

        setIsSubmittingEditFlags(true);
        setEditFlagsDialogError(null);
        try {
            // Handle player flag change if needed (requires set_player_flag builtin)
            if (newUser !== currentUser) {
                const userExpr = `return set_player_flag(${objectExpr}, ${newUser});`;
                console.debug("Evaluating set_player_flag expression:", userExpr);
                await performEvalFlatBuffer(authToken, userExpr);
            }

            // Handle other flag changes
            if (assignments.length > 0) {
                const expr = assignments.join("; ") + "; return 1;";
                console.debug("Evaluating set flags expression:", expr);
                await performEvalFlatBuffer(authToken, expr);
            }

            // Reload the objects list to reflect the change
            const updated = await loadObjects();
            const updatedObj = updated.find(obj => obj.obj === selectedObject.obj);
            if (updatedObj) {
                setSelectedObject(updatedObj);
            }

            setActionMessage("Flags updated successfully");
            setTimeout(() => setActionMessage(null), 3000);
            setShowEditFlagsDialog(false);
        } catch (error) {
            console.error("Failed to update flags:", error);
            setEditFlagsDialogError(
                "Failed to update flags: " + (error instanceof Error ? error.message : String(error)),
            );
        } finally {
            setIsSubmittingEditFlags(false);
        }
    };

    const handleCreateSubmit = async (form: CreateChildFormValues) => {
        if (!selectedObject) return;

        const parentExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!parentExpr) {
            setCreateDialogError("Unable to determine parent object reference.");
            return;
        }

        const ownerExpr = normalizeObjectInput(form.owner || "player") || "player";
        const trimmedInit = form.initArgs.trim();
        const includeType = form.objectType !== "server-default" || trimmedInit.length > 0;
        const typeExpr = includeType ? resolveObjectTypeValue(form.objectType) : "";

        const args: string[] = [parentExpr, ownerExpr];
        if (includeType) {
            args.push(typeExpr);
        }
        if (trimmedInit.length > 0) {
            args.push(trimmedInit);
        }

        const expr = `return create(${args.join(", ")});`;

        setIsSubmittingCreate(true);
        setCreateDialogError(null);
        try {
            console.debug("Evaluating create expression:", expr);
            const previousIds = new Set(objects.map(o => o.obj));
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "create() failed";
                throw new Error(msg);
            }

            const updated = await loadObjects();
            const newSelection = updated.find(obj => !previousIds.has(obj.obj))
                || (selectedObject ? updated.find(obj => obj.obj === selectedObject.obj) : null);
            if (newSelection && !previousIds.has(newSelection.obj)) {
                handleObjectSelect(newSelection);
            }

            setShowCreateDialog(false);
            if (newSelection && !previousIds.has(newSelection.obj)) {
                setActionMessage(`Created ${describeObject(newSelection)}`);
            } else {
                setActionMessage("Created new object.");
            }
        } catch (error) {
            setCreateDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingCreate(false);
        }
    };

    const handleRecycleConfirm = async () => {
        if (!selectedObject) return;
        const target = selectedObject;
        const objectExpr = normalizeObjectInput(target.obj ? `#${target.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setRecycleDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingRecycle(true);
        setRecycleDialogError(null);

        try {
            const recycleExpr = `return recycle(${objectExpr});`;
            console.debug("Evaluating recycle expression:", recycleExpr);
            const result = await performEvalFlatBuffer(authToken, recycleExpr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "recycle() failed";
                throw new Error(msg);
            }
            if (typeof result === "string") {
                const trimmed = result.trim();
                if (trimmed.length > 0) {
                    throw new Error(trimmed);
                }
            }

            const updated = await loadObjects();
            setShowRecycleDialog(false);

            const parentId = target.parent;
            let navigated = false;
            if (parentId) {
                const parentObj = updated.find(obj => obj.obj === parentId);
                if (parentObj) {
                    handleObjectSelect(parentObj);
                    navigated = true;
                }
            }
            if (!navigated) {
                setSelectedObject(null);
                setSelectedProperty(null);
                setSelectedVerb(null);
                setEditorVisible(false);
            }

            setActionMessage(`Recycled ${describeObject(target)}`);
        } catch (error) {
            setRecycleDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingRecycle(false);
        }
    };

    const handleAddPropertySubmit = async (form: AddPropertyFormValues) => {
        if (!selectedObject) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setAddPropertyDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingAddProperty(true);
        setAddPropertyDialogError(null);

        try {
            // Escape the property name and value for MOO
            const escapedName = form.name.trim();
            if (!escapedName) {
                throw new Error("Property name cannot be empty");
            }

            const ownerExpr = normalizeObjectInput(form.owner || "player") || "player";
            const perms = form.perms.trim() || "rw";

            // Validate perms string
            if (!/^[rwc]*$/.test(perms)) {
                throw new Error("Invalid permissions. Use r, w, and/or c");
            }

            // Build the add_property call
            // add_property(obj, 'name, value, {owner, "perms"})
            const expr =
                `return add_property(${objectExpr}, '${escapedName}, ${form.value}, {${ownerExpr}, "${perms}"});`;

            console.debug("Evaluating add_property expression:", expr);
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "add_property() failed";
                throw new Error(msg);
            }

            // Reload properties list
            await loadPropertiesAndVerbs(selectedObject);

            setShowAddPropertyDialog(false);
            setActionMessage(`Added property "${escapedName}" to ${describeObject(selectedObject)}`);
        } catch (error) {
            setAddPropertyDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingAddProperty(false);
        }
    };

    const handleAddVerbSubmit = async (form: AddVerbFormValues) => {
        if (!selectedObject) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setAddVerbDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingAddVerb(true);
        setAddVerbDialogError(null);

        try {
            // Validate and normalize verb names
            const verbNames = form.names.trim();
            if (!verbNames) {
                throw new Error("Verb names cannot be empty");
            }

            const ownerExpr = normalizeObjectInput(form.owner || "player") || "player";
            const perms = form.perms.trim() || "rxd";

            // Validate perms string for verbs (r, w, x, d)
            if (!/^[rwxd]*$/.test(perms)) {
                throw new Error("Invalid permissions. Use r, w, x, and/or d");
            }

            // Normalize argument specs
            const dobj = form.dobj.trim() || "this";
            const prep = form.prep.trim() || "none";
            const iobj = form.iobj.trim() || "none";

            // Build the add_verb call
            // add_verb(obj, {owner, "perms", "names"}, {"dobj", "prep", "iobj"})
            const expr =
                `return add_verb(${objectExpr}, {${ownerExpr}, "${perms}", "${verbNames}"}, {"${dobj}", "${prep}", "${iobj}"});`;

            console.debug("Evaluating add_verb expression:", expr);
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "add_verb() failed";
                throw new Error(msg);
            }

            // Reload verbs list
            await loadPropertiesAndVerbs(selectedObject);

            setShowAddVerbDialog(false);
            setActionMessage(`Added verb "${verbNames}" to ${describeObject(selectedObject)}`);
        } catch (error) {
            setAddVerbDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingAddVerb(false);
        }
    };

    const handleDeleteVerbConfirm = async () => {
        if (!selectedObject || !verbToDelete) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setDeleteVerbDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingDeleteVerb(true);
        setDeleteVerbDialogError(null);

        try {
            // delete_verb(obj, verbname)
            const verbName = verbToDelete.names[0];
            const expr = `return delete_verb(${objectExpr}, "${verbName}");`;

            console.debug("Evaluating delete_verb expression:", expr);
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "delete_verb() failed";
                throw new Error(msg);
            }

            // Reload verbs list
            await loadPropertiesAndVerbs(selectedObject);

            setShowDeleteVerbDialog(false);
            setVerbToDelete(null);
            setSelectedVerb(null);
            setEditorVisible(false);

            setActionMessage(`Removed verb "${verbName}" from ${describeObject(selectedObject)}`);
        } catch (error) {
            setDeleteVerbDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingDeleteVerb(false);
        }
    };

    const handleDeletePropertyConfirm = async () => {
        if (!selectedObject || !propertyToDelete) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setDeletePropertyDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingDeleteProperty(true);
        setDeletePropertyDialogError(null);

        try {
            // delete_property(obj, 'name)
            const expr = `return delete_property(${objectExpr}, '${propertyToDelete.name});`;

            console.debug("Evaluating delete_property expression:", expr);
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "delete_property() failed";
                throw new Error(msg);
            }

            // Reload properties list
            await loadPropertiesAndVerbs(selectedObject);

            setShowDeletePropertyDialog(false);
            setPropertyToDelete(null);
            setSelectedProperty(null);
            setEditorVisible(false);

            setActionMessage(`Deleted property "${propertyToDelete.name}" from ${describeObject(selectedObject)}`);
        } catch (error) {
            setDeletePropertyDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingDeleteProperty(false);
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
        let entries = Array.from(groups.entries()).sort((a, b) => {
            // Sort by definer ID (current object first)
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            return a[0].localeCompare(b[0]);
        });
        if (!showInheritedProperties && selectedObject) {
            const currentId = selectedObject.obj;
            entries = entries.filter(([definer]) => definer === currentId);
        }
        return entries;
    }, [properties, selectedObject, propertyFilter, showInheritedProperties]);

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
        let entries = Array.from(groups.entries()).sort((a, b) => {
            // Sort by location ID (current object first)
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            return a[0].localeCompare(b[0]);
        });
        if (!showInheritedVerbs && selectedObject) {
            const currentId = selectedObject.obj;
            entries = entries.filter(([location]) => location === currentId);
        }
        return entries;
    }, [verbs, selectedObject, verbFilter, showInheritedVerbs]);

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

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem("moor-object-browser-font-size", fontSize.toString());
        }
    }, [fontSize]);

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem(
                "moor-object-browser-show-inherited-properties",
                showInheritedProperties ? "true" : "false",
            );
        }
    }, [showInheritedProperties]);

    useEffect(() => {
        if (typeof window !== "undefined") {
            window.localStorage.setItem(
                "moor-object-browser-show-inherited-verbs",
                showInheritedVerbs ? "true" : "false",
            );
        }
    }, [showInheritedVerbs]);

    if (!visible) {
        return null;
    }

    const baseFontSize = fontSize;
    const secondaryFontSize = Math.max(8, fontSize - 1);
    const inheritedLabelStyle = {
        padding: "var(--space-xs) var(--space-sm)",
        backgroundColor: "var(--color-bg-secondary)",
        borderTop: "1px solid var(--color-border-medium)",
        borderBottom: "1px solid var(--color-border-light)",
        fontSize: `${secondaryFontSize}px`,
        fontWeight: 600,
        color: "var(--color-text-secondary)",
        fontFamily: "var(--font-mono)",
    } as const;
    const inheritedToggleButtonStyle = (active: boolean): React.CSSProperties => ({
        width: "22px",
        height: "22px",
        borderRadius: "50%",
        border: "1px solid var(--color-border-medium)",
        backgroundColor: active ? "var(--color-text-primary)" : "transparent",
        color: active ? "var(--color-bg-input)" : "var(--color-text-secondary)",
        cursor: "pointer",
        fontSize: "11px",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        transition: "background-color 0.2s ease",
    });

    const objectTypeOptions = (() => {
        const options: Array<{ value: string; label: string }> = [];
        options.push({
            value: "server-default",
            label: serverFeatures
                ? `Server default (${serverFeatures.useUuobjids ? "UUID" : "numbered"})`
                : "Server default",
        });
        options.push({ value: "numbered", label: "Numbered (# objects)" });
        if (serverFeatures?.useUuobjids) {
            options.push({ value: "uuid", label: "UUID objects" });
        }
        if (serverFeatures?.anonymousObjects) {
            options.push({ value: "anonymous", label: "Anonymous objects" });
        }
        return options;
    })();

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

    const normalizeObjectRef = (raw: string): { display: string; objectId: string | null } => {
        const value = raw?.trim();
        if (!value) {
            return { display: "none", objectId: null };
        }
        if (value === "nothing" || value === "-1") {
            return { display: "#-1", objectId: null };
        }
        if (value.startsWith("oid:")) {
            const id = value.substring(4);
            return { display: `#${id}`, objectId: id };
        }
        if (value.startsWith("uuid:")) {
            const id = value.substring(5);
            return { display: `#${id}`, objectId: id };
        }
        if (/^-?\d+$/.test(value)) {
            return { display: `#${value}`, objectId: value };
        }
        return { display: value, objectId: null };
    };

    const handleNavigateToObject = (objectId: string) => {
        const target = objects.find(obj => obj.obj === objectId);
        if (target) {
            handleObjectSelect(target);
        }
    };

    const normalizeObjectInput = (raw: string): string => {
        if (!raw) return "";
        const trimmed = raw.trim();
        if (!trimmed) return "";
        if (
            trimmed.startsWith("#") || trimmed.startsWith("$") || trimmed.startsWith("player")
            || trimmed.startsWith("caller")
        ) {
            return trimmed;
        }
        if (trimmed.startsWith("oid:")) {
            return `#${trimmed.substring(4)}`;
        }
        if (trimmed.startsWith("uuid:")) {
            return `#${trimmed.substring(5)}`;
        }
        if (/^-?\d+$/.test(trimmed)) {
            return `#${trimmed}`;
        }
        if (/^[0-9A-Za-z-]+$/.test(trimmed)) {
            return `#${trimmed}`;
        }
        return trimmed;
    };

    const defaultObjectTypeValue = () => (serverFeatures?.useUuobjids ? "2" : "0");

    const resolveObjectTypeValue = (selection: string): string => {
        switch (selection) {
            case "numbered":
                return "0";
            case "uuid":
                return "2";
            case "anonymous":
                return "1";
            case "server-default":
            default:
                return defaultObjectTypeValue();
        }
    };

    const describeObject = (obj: ObjectData): string => {
        const id = normalizeObjectInput(obj.obj) || "#?";
        return obj.name ? `${id} ("${obj.name}")` : id;
    };

    // Split mode styling
    const splitStyle = {
        width: "100%",
        height: "100%",
        backgroundColor: "var(--color-bg-input)",
        border: "1px solid var(--color-border-medium)",
        display: "flex",
        flexDirection: "column" as const,
        overflow: "hidden",
        fontSize: `${baseFontSize}px`,
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
        fontSize: `${baseFontSize}px`,
    };

    const isSplitDraggable = splitMode && typeof onSplitDrag === "function";
    const titleMouseDownHandler = isSplitDraggable ? onSplitDrag : (splitMode ? undefined : handleMouseDown);
    const titleTouchStartHandler = isSplitDraggable ? onSplitTouchStart : undefined;

    return (
        <>
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
                    onMouseDown={titleMouseDownHandler}
                    onTouchStart={titleTouchStartHandler}
                    style={{
                        padding: "var(--space-md)",
                        borderBottom: "1px solid var(--color-border-light)",
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        backgroundColor: "var(--color-bg-header)",
                        borderRadius: splitMode ? "0" : "var(--radius-lg) var(--radius-lg) 0 0",
                        cursor: isSplitDraggable
                            ? "row-resize"
                            : (splitMode ? "default" : (isDragging ? "grabbing" : "grab")),
                        touchAction: isSplitDraggable ? "none" : "auto",
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
                            onClick={(e) => e.stopPropagation()}
                        >
                            <button
                                onClick={decreaseFontSize}
                                aria-label="Decrease browser font size"
                                style={{
                                    background: "transparent",
                                    border: "none",
                                    color: "var(--color-text-secondary)",
                                    cursor: fontSize <= MIN_FONT_SIZE ? "not-allowed" : "pointer",
                                    opacity: fontSize <= MIN_FONT_SIZE ? 0.5 : 1,
                                    fontSize: `${secondaryFontSize}px`,
                                    padding: "2px 4px",
                                }}
                                disabled={fontSize <= MIN_FONT_SIZE}
                            >
                                –
                            </button>
                            <span
                                style={{
                                    fontFamily: "var(--font-mono)",
                                    fontSize: `${secondaryFontSize}px`,
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
                                aria-label="Increase browser font size"
                                style={{
                                    background: "transparent",
                                    border: "none",
                                    color: "var(--color-text-secondary)",
                                    cursor: fontSize >= MAX_FONT_SIZE ? "not-allowed" : "pointer",
                                    opacity: fontSize >= MAX_FONT_SIZE ? 0.5 : 1,
                                    fontSize: `${secondaryFontSize}px`,
                                    padding: "2px 4px",
                                }}
                                disabled={fontSize >= MAX_FONT_SIZE}
                            >
                                +
                            </button>
                        </div>
                        <div
                            style={{
                                display: "flex",
                                alignItems: "center",
                                gap: "6px",
                                backgroundColor: "var(--color-bg-secondary)",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "999px",
                                padding: "4px 8px",
                            }}
                            onClick={(e) => e.stopPropagation()}
                        >
                            <span
                                style={{
                                    fontSize: "10px",
                                    color: "var(--color-text-secondary)",
                                    textTransform: "uppercase",
                                    letterSpacing: "0.08em",
                                    fontWeight: 600,
                                    fontFamily: "var(--font-sans)",
                                    opacity: 0.8,
                                }}
                            >
                                Inherited
                            </span>
                            <button
                                type="button"
                                onClick={() => setShowInheritedProperties(prev => !prev)}
                                aria-label={showInheritedProperties
                                    ? "Hide inherited properties"
                                    : "Show inherited properties"}
                                title={showInheritedProperties
                                    ? "Hide inherited properties"
                                    : "Show inherited properties"}
                                style={inheritedToggleButtonStyle(showInheritedProperties)}
                            >
                                P
                            </button>
                            <button
                                type="button"
                                onClick={() => setShowInheritedVerbs(prev => !prev)}
                                aria-label={showInheritedVerbs ? "Hide inherited verbs" : "Show inherited verbs"}
                                title={showInheritedVerbs ? "Hide inherited verbs" : "Show inherited verbs"}
                                style={inheritedToggleButtonStyle(showInheritedVerbs)}
                            >
                                V
                            </button>
                        </div>
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
                                    fontSize: `${secondaryFontSize}px`,
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
                            flex: (editorVisible || selectedObject) ? editorSplitPosition : 1,
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
                                    padding: "var(--space-xs) var(--space-sm)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    borderBottom: "1px solid var(--color-border-light)",
                                    display: "flex",
                                    alignItems: "center",
                                    justifyContent: "space-between",
                                }}
                            >
                                <span
                                    style={{
                                        textTransform: "uppercase",
                                        letterSpacing: "0.08em",
                                        fontSize: `${secondaryFontSize}px`,
                                        color: "var(--color-text-secondary)",
                                        fontWeight: 600,
                                    }}
                                >
                                    Objects
                                </span>
                            </div>
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
                                        fontSize: `${baseFontSize}px`,
                                    }}
                                />
                            </div>
                            <div
                                style={{
                                    flex: 1,
                                    overflowY: "auto",
                                    fontSize: `${baseFontSize}px`,
                                }}
                            >
                                {isLoading
                                    ? (
                                        <div
                                            style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}
                                        >
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

                                            {/* Separator and UUID objects section */}
                                            {uuidObjects.length > 0 && (
                                                <>
                                                    <div
                                                        style={{
                                                            padding: "var(--space-xs) var(--space-sm)",
                                                            backgroundColor: "var(--color-bg-secondary)",
                                                            borderTop: "2px solid var(--color-border-medium)",
                                                            borderBottom: "1px solid var(--color-border-light)",
                                                            fontSize: `${secondaryFontSize}px`,
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
                                                                    e.currentTarget.style.backgroundColor =
                                                                        "transparent";
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
                                    padding: "var(--space-xs) var(--space-sm)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    borderBottom: "1px solid var(--color-border-light)",
                                    display: "flex",
                                    alignItems: "center",
                                    justifyContent: "space-between",
                                }}
                            >
                                <span
                                    style={{
                                        textTransform: "uppercase",
                                        letterSpacing: "0.08em",
                                        fontSize: `${secondaryFontSize}px`,
                                        color: "var(--color-text-secondary)",
                                        fontWeight: 600,
                                    }}
                                >
                                    Properties
                                </span>
                                {selectedObject && (
                                    <button
                                        type="button"
                                        onClick={() => {
                                            setAddPropertyDialogError(null);
                                            setActionMessage(null);
                                            setShowAddPropertyDialog(true);
                                        }}
                                        disabled={isSubmittingAddProperty}
                                        aria-label="Add property"
                                        title="Add property"
                                        style={{
                                            padding: "4px 8px",
                                            borderRadius: "var(--radius-sm)",
                                            border: "1px solid var(--color-border-medium)",
                                            backgroundColor: "var(--color-bg-secondary)",
                                            color: "var(--color-text-primary)",
                                            cursor: isSubmittingAddProperty ? "not-allowed" : "pointer",
                                            opacity: isSubmittingAddProperty ? 0.6 : 1,
                                            fontSize: `${secondaryFontSize}px`,
                                            fontWeight: 600,
                                        }}
                                    >
                                        + Add
                                    </button>
                                )}
                            </div>
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
                                        fontSize: `${baseFontSize}px`,
                                    }}
                                />
                            </div>
                            <div
                                style={{
                                    flex: 1,
                                    overflowY: "auto",
                                    fontSize: `${baseFontSize}px`,
                                }}
                            >
                                {!selectedObject
                                    ? (
                                        <div
                                            style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}
                                        >
                                            Select an object to view properties
                                        </div>
                                    )
                                    : properties.length === 0
                                    ? (
                                        <div
                                            style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}
                                        >
                                            No properties
                                        </div>
                                    )
                                    : (
                                        groupedProperties.map(([definer, props], groupIdx) => (
                                            <div key={definer}>
                                                {groupIdx > 0 && showInheritedProperties && (
                                                    <div style={inheritedLabelStyle}>
                                                        from {normalizeObjectRef(definer).display}
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
                                                                    fontSize: `${secondaryFontSize}px`,
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
                                    padding: "var(--space-xs) var(--space-sm)",
                                    backgroundColor: "var(--color-bg-secondary)",
                                    borderBottom: "1px solid var(--color-border-light)",
                                    display: "flex",
                                    alignItems: "center",
                                    justifyContent: "space-between",
                                }}
                            >
                                <span
                                    style={{
                                        textTransform: "uppercase",
                                        letterSpacing: "0.08em",
                                        fontSize: `${secondaryFontSize}px`,
                                        color: "var(--color-text-secondary)",
                                        fontWeight: 600,
                                    }}
                                >
                                    Verbs
                                </span>
                                {selectedObject && (
                                    <button
                                        type="button"
                                        onClick={() => {
                                            setAddVerbDialogError(null);
                                            setActionMessage(null);
                                            setShowAddVerbDialog(true);
                                        }}
                                        disabled={isSubmittingAddVerb}
                                        aria-label="Add verb"
                                        title="Add verb"
                                        style={{
                                            padding: "4px 8px",
                                            borderRadius: "var(--radius-sm)",
                                            border: "1px solid var(--color-border-medium)",
                                            backgroundColor: "var(--color-bg-secondary)",
                                            color: "var(--color-text-primary)",
                                            cursor: isSubmittingAddVerb ? "not-allowed" : "pointer",
                                            opacity: isSubmittingAddVerb ? 0.6 : 1,
                                            fontSize: `${secondaryFontSize}px`,
                                            fontWeight: 600,
                                        }}
                                    >
                                        + Add
                                    </button>
                                )}
                            </div>
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
                                        fontSize: `${baseFontSize}px`,
                                    }}
                                />
                            </div>
                            <div
                                style={{
                                    flex: 1,
                                    overflowY: "auto",
                                    fontSize: `${baseFontSize}px`,
                                }}
                            >
                                {!selectedObject
                                    ? (
                                        <div
                                            style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}
                                        >
                                            Select an object to view verbs
                                        </div>
                                    )
                                    : verbs.length === 0
                                    ? (
                                        <div
                                            style={{ padding: "var(--space-md)", color: "var(--color-text-secondary)" }}
                                        >
                                            No verbs
                                        </div>
                                    )
                                    : (
                                        groupedVerbs.map(([location, verbList], groupIdx) => (
                                            <div key={location}>
                                                {groupIdx > 0 && showInheritedVerbs && (
                                                    <div style={inheritedLabelStyle}>
                                                        from {normalizeObjectRef(location).display}
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
                                                                    fontSize: `${secondaryFontSize}px`,
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
                        </div>
                    </div>

                    {/* Draggable splitter bar */}
                    {(editorVisible || selectedObject) && (
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
                    {(editorVisible || selectedObject) && (
                        <div
                            style={{
                                flex: 1 - editorSplitPosition,
                                overflow: "hidden",
                                backgroundColor: "var(--color-bg-secondary)",
                            }}
                        >
                            {selectedObject && !selectedProperty && !selectedVerb && (
                                <ObjectInfoEditor
                                    object={selectedObject}
                                    objects={objects}
                                    authToken={authToken}
                                    onNavigate={handleNavigateToObject}
                                    normalizeObjectRef={normalizeObjectRef}
                                    normalizeObjectInput={normalizeObjectInput}
                                    onCreateChild={() => {
                                        setCreateDialogError(null);
                                        setActionMessage(null);
                                        setShowCreateDialog(true);
                                    }}
                                    onRecycle={() => {
                                        setRecycleDialogError(null);
                                        setActionMessage(null);
                                        setShowRecycleDialog(true);
                                    }}
                                    onEditFlags={() => {
                                        setEditFlagsDialogError(null);
                                        setActionMessage(null);
                                        setShowEditFlagsDialog(true);
                                    }}
                                    isSubmittingCreate={isSubmittingCreate}
                                    isSubmittingRecycle={isSubmittingRecycle}
                                    editingName={editingName}
                                    onNameChange={setEditingName}
                                    onNameSave={handleNameSave}
                                    isSavingName={isSavingName}
                                    actionMessage={actionMessage}
                                />
                            )}
                            {selectedProperty && selectedProperty.moorVar && selectedObject && (
                                <PropertyValueEditor
                                    authToken={authToken}
                                    objectCurie={selectedProperty.definer.includes(":")
                                        ? selectedProperty.definer
                                        : `oid:${selectedProperty.definer}`}
                                    propertyName={selectedProperty.name}
                                    propertyValue={selectedProperty.moorVar}
                                    onSave={async () => {
                                        // Reload properties list to get updated metadata, then reload property value
                                        if (selectedObject) {
                                            const freshProps = await loadPropertiesAndVerbs(selectedObject);
                                            // Find the updated property in the freshly loaded list
                                            const updatedProp = freshProps.find(p => p.name === selectedProperty.name);
                                            if (updatedProp) {
                                                await handlePropertySelect(updatedProp);
                                            }
                                        }
                                    }}
                                    onCancel={() => {
                                        setSelectedProperty(null);
                                        setEditorVisible(false);
                                    }}
                                    onDelete={selectedProperty.definer === selectedObject.obj
                                        ? () => {
                                            setPropertyToDelete(selectedProperty);
                                            setDeletePropertyDialogError(null);
                                            setActionMessage(null);
                                            setShowDeletePropertyDialog(true);
                                        }
                                        : undefined}
                                    owner={selectedProperty.owner}
                                    definer={selectedProperty.definer}
                                    permissions={{
                                        readable: selectedProperty.readable,
                                        writable: selectedProperty.writable,
                                    }}
                                    onNavigateToObject={handleNavigateToObject}
                                    normalizeObjectInput={normalizeObjectInput}
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
                                    owner={selectedVerb.owner}
                                    definer={selectedVerb.location}
                                    permissions={{
                                        readable: selectedVerb.readable,
                                        writable: selectedVerb.writable,
                                        executable: selectedVerb.executable,
                                        debug: false, // TODO: Need to add debug field to VerbData
                                    }}
                                    argspec={{
                                        dobj: argSpecToString(selectedVerb.dobj),
                                        prep: prepSpecToString(selectedVerb.prep),
                                        iobj: argSpecToString(selectedVerb.iobj),
                                    }}
                                    onSave={() => {
                                        // Reload verbs list in background to update the list
                                        if (selectedObject) {
                                            loadPropertiesAndVerbs(selectedObject);
                                        }
                                    }}
                                    onDelete={() => {
                                        setVerbToDelete(selectedVerb);
                                        setShowDeleteVerbDialog(true);
                                    }}
                                    normalizeObjectInput={normalizeObjectInput}
                                />
                            )}
                        </div>
                    )}
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
                        aria-label="Resize browser window"
                        onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                handleResizeMouseDown({
                                    ...e,
                                    clientX: size.width + position.x,
                                    clientY: size.height + position.y,
                                    button: 0,
                                } as unknown as React.MouseEvent<HTMLDivElement>);
                            }
                        }}
                        style={{
                            position: "absolute",
                            bottom: 0,
                            right: 0,
                            width: "22px",
                            height: "22px",
                            cursor: "nwse-resize",
                            borderBottomRightRadius: "var(--radius-lg)",
                            borderTopLeftRadius: "6px",
                            backgroundColor: "var(--color-surface-raised)",
                            borderTop: "1px solid var(--color-border-medium)",
                            borderLeft: "1px solid var(--color-border-medium)",
                            boxShadow: "inset 0 0 0 1px rgba(0, 0, 0, 0.1)",
                            zIndex: 5,
                        }}
                    >
                        <div
                            style={{
                                position: "absolute",
                                inset: "4px",
                                borderBottom: "2px solid var(--color-border-strong)",
                                borderRight: "2px solid var(--color-border-strong)",
                                borderBottomRightRadius: "4px",
                                pointerEvents: "none",
                            }}
                        />
                        <div
                            style={{
                                position: "absolute",
                                right: "6px",
                                bottom: "6px",
                                width: "10px",
                                height: "10px",
                                clipPath: "polygon(0 100%, 100% 0, 100% 100%)",
                                background:
                                    "linear-gradient(135deg, transparent 0%, transparent 30%, var(--color-border-strong) 30%, var(--color-border-strong) 50%, transparent 50%)",
                                pointerEvents: "none",
                            }}
                        />
                        <span
                            aria-hidden="true"
                            style={{
                                position: "absolute",
                                right: "4px",
                                bottom: "2px",
                                fontSize: "14px",
                                color: "var(--color-border-strong)",
                                lineHeight: 1,
                                pointerEvents: "none",
                                userSelect: "none",
                            }}
                        >
                            ↘
                        </span>
                    </div>
                )}
            </div>
            {showCreateDialog && selectedObject && (
                <CreateChildDialog
                    key={selectedObject.obj}
                    parentLabel={describeObject(selectedObject)}
                    defaultOwner={normalizeObjectInput(selectedObject.owner ? `#${selectedObject.owner}` : "")
                        || "player"}
                    objectTypeOptions={objectTypeOptions}
                    onCancel={() => setShowCreateDialog(false)}
                    onSubmit={handleCreateSubmit}
                    isSubmitting={isSubmittingCreate}
                    errorMessage={createDialogError}
                />
            )}
            {showRecycleDialog && selectedObject && (
                <RecycleObjectDialog
                    key={`recycle-${selectedObject.obj}`}
                    objectLabel={describeObject(selectedObject)}
                    onCancel={() => setShowRecycleDialog(false)}
                    onConfirm={handleRecycleConfirm}
                    isSubmitting={isSubmittingRecycle}
                    errorMessage={recycleDialogError}
                />
            )}
            {showAddPropertyDialog && selectedObject && (
                <AddPropertyDialog
                    key={`add-property-${selectedObject.obj}`}
                    objectLabel={describeObject(selectedObject)}
                    defaultOwner={normalizeObjectInput(selectedObject.owner ? `#${selectedObject.owner}` : "")
                        || "player"}
                    onCancel={() => setShowAddPropertyDialog(false)}
                    onSubmit={handleAddPropertySubmit}
                    isSubmitting={isSubmittingAddProperty}
                    errorMessage={addPropertyDialogError}
                />
            )}
            {showDeletePropertyDialog && propertyToDelete && selectedObject && (
                <DeletePropertyDialog
                    key={`delete-property-${propertyToDelete.name}`}
                    propertyName={propertyToDelete.name}
                    objectLabel={describeObject(selectedObject)}
                    onCancel={() => {
                        setShowDeletePropertyDialog(false);
                        setPropertyToDelete(null);
                    }}
                    onConfirm={handleDeletePropertyConfirm}
                    isSubmitting={isSubmittingDeleteProperty}
                    errorMessage={deletePropertyDialogError}
                />
            )}
            {showAddVerbDialog && selectedObject && (
                <AddVerbDialog
                    key={`add-verb-${selectedObject.obj}`}
                    objectLabel={describeObject(selectedObject)}
                    defaultOwner={normalizeObjectInput(selectedObject.owner ? `#${selectedObject.owner}` : "")
                        || "player"}
                    onCancel={() => setShowAddVerbDialog(false)}
                    onSubmit={handleAddVerbSubmit}
                    isSubmitting={isSubmittingAddVerb}
                    errorMessage={addVerbDialogError}
                />
            )}
            {showDeleteVerbDialog && verbToDelete && selectedObject && (
                <DeleteVerbDialog
                    key={`delete-verb-${verbToDelete.names[0]}`}
                    verbName={verbToDelete.names.join(" ")}
                    objectLabel={describeObject(selectedObject)}
                    onCancel={() => {
                        setShowDeleteVerbDialog(false);
                        setVerbToDelete(null);
                    }}
                    onConfirm={handleDeleteVerbConfirm}
                    isSubmitting={isSubmittingDeleteVerb}
                    errorMessage={deleteVerbDialogError}
                />
            )}
            {showEditFlagsDialog && selectedObject && (
                <EditFlagsDialog
                    key={`edit-flags-${selectedObject.obj}`}
                    objectLabel={describeObject(selectedObject)}
                    currentFlags={selectedObject.flags}
                    onCancel={() => setShowEditFlagsDialog(false)}
                    onSubmit={handleEditFlagsSubmit}
                    isSubmitting={isSubmittingEditFlags}
                    errorMessage={editFlagsDialogError}
                />
            )}
        </>
    );
};

interface CreateChildDialogProps {
    parentLabel: string;
    defaultOwner: string;
    objectTypeOptions: Array<{ value: string; label: string }>;
    onCancel: () => void;
    onSubmit: (form: CreateChildFormValues) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const CreateChildDialog: React.FC<CreateChildDialogProps> = ({
    parentLabel,
    defaultOwner,
    objectTypeOptions,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [owner, setOwner] = useState(defaultOwner);
    const [objectType, setObjectType] = useState<string>("server-default");
    const [initArgs, setInitArgs] = useState<string>("");

    useEffect(() => {
        setOwner(defaultOwner);
        setObjectType("server-default");
        setInitArgs("");
    }, [defaultOwner]);

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        onSubmit({ owner, objectType, initArgs });
    };

    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "520px" }}
                role="dialog"
                aria-modal="true"
                aria-labelledby="create-object-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="create-object-title">Create Child Object</h2>
                </div>
                <form onSubmit={handleSubmit} className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <p style={{ margin: 0, color: "var(--color-text-secondary)" }}>
                        The new object will be created as a child of <strong>{parentLabel}</strong>.
                    </p>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Owner (MOO expression)</span>
                        <input
                            type="text"
                            value={owner}
                            onChange={(e) => setOwner(e.target.value)}
                            placeholder="player"
                            autoFocus
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Object type</span>
                        <select
                            value={objectType}
                            onChange={(e) => setObjectType(e.target.value)}
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {objectTypeOptions.map((option) => (
                                <option key={option.value} value={option.value}>
                                    {option.label}
                                </option>
                            ))}
                        </select>
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Initialization arguments</span>
                        <textarea
                            value={initArgs}
                            onChange={(e) => setInitArgs(e.target.value)}
                            placeholder="{}"
                            rows={3}
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                                resize: "vertical",
                            }}
                        />
                        <span style={{ color: "var(--color-text-secondary)", fontSize: "0.85em" }}>
                            Provide a MOO list literal (for example <code>{"{}"}</code> or{" "}
                            <code>{"{"}player{"}"}</code>). Leave blank to skip initialization arguments.
                        </span>
                    </label>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-accent)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Creating…" : "Create"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
};

interface RecycleObjectDialogProps {
    objectLabel: string;
    onCancel: () => void;
    onConfirm: () => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const RecycleObjectDialog: React.FC<RecycleObjectDialogProps> = ({
    objectLabel,
    onCancel,
    onConfirm,
    isSubmitting,
    errorMessage,
}) => {
    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "480px" }}
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="recycle-object-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="recycle-object-title">Recycle Object?</h2>
                </div>
                <div className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <div
                        style={{
                            padding: "0.75em",
                            borderRadius: "var(--radius-sm)",
                            border: "1px solid var(--color-text-error)",
                            backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                            color: "var(--color-text-primary)",
                            fontFamily: "inherit",
                        }}
                    >
                        <p style={{ margin: 0 }}>
                            Recycling <strong>{objectLabel}</strong> is irreversible. Its contents will move to{" "}
                            <code>#-1</code>
                            and <code>:recycle</code> will be invoked if defined.
                        </p>
                    </div>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="button"
                            onClick={onConfirm}
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-error)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Recycling…" : "Recycle"}
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
};

interface AddPropertyDialogProps {
    objectLabel: string;
    defaultOwner: string;
    onCancel: () => void;
    onSubmit: (form: AddPropertyFormValues) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const AddPropertyDialog: React.FC<AddPropertyDialogProps> = ({
    objectLabel,
    defaultOwner,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [name, setName] = useState("");
    const [value, setValue] = useState("0");
    const [owner, setOwner] = useState(defaultOwner);
    const [perms, setPerms] = useState("r");

    useEffect(() => {
        setName("");
        setValue("0");
        setOwner(defaultOwner);
        setPerms("r");
    }, [defaultOwner]);

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        onSubmit({ name, value, owner, perms });
    };

    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "520px" }}
                role="dialog"
                aria-modal="true"
                aria-labelledby="add-property-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="add-property-title">Add Property</h2>
                </div>
                <form onSubmit={handleSubmit} className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <p style={{ margin: 0, color: "var(--color-text-secondary)" }}>
                        Add a new property to <strong>{objectLabel}</strong>.
                    </p>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Property name</span>
                        <input
                            type="text"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            placeholder="prop_name"
                            autoFocus
                            required
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Initial value (MOO expression)</span>
                        <input
                            type="text"
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            placeholder="0"
                            required
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                        <span style={{ color: "var(--color-text-secondary)", fontSize: "0.85em" }}>
                            Examples: <code>0</code>, <code>""</code>, <code>{"{}"}</code>, <code>player</code>
                        </span>
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Owner (MOO expression)</span>
                        <input
                            type="text"
                            value={owner}
                            onChange={(e) => setOwner(e.target.value)}
                            placeholder="player"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                    </label>
                    <div style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Permissions</span>
                        <span style={{ color: "var(--color-text-secondary)", fontSize: "0.85em" }}>
                            r=read, w=write, c=chown
                        </span>
                        <div
                            style={{
                                display: "flex",
                                gap: "0.75em",
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                            }}
                        >
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("r")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "r");
                                        } else {
                                            setPerms(perms.replace("r", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>r</span>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("w")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "w");
                                        } else {
                                            setPerms(perms.replace("w", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>w</span>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("c")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "c");
                                        } else {
                                            setPerms(perms.replace("c", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>c</span>
                            </label>
                        </div>
                    </div>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-accent)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Adding…" : "Add Property"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
};

interface AddVerbDialogProps {
    objectLabel: string;
    defaultOwner: string;
    onCancel: () => void;
    onSubmit: (form: AddVerbFormValues) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const AddVerbDialog: React.FC<AddVerbDialogProps> = ({
    objectLabel,
    defaultOwner,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [verbType, setVerbType] = useState<"method" | "command">("method");
    const [names, setNames] = useState("");
    const [owner, setOwner] = useState(defaultOwner);
    const [perms, setPerms] = useState("rxd");
    const [dobj, setDobj] = useState("this");
    const [prep, setPrep] = useState("none");
    const [iobj, setIobj] = useState("this");

    useEffect(() => {
        setNames("");
        setOwner(defaultOwner);
        setVerbType("method");
        setPerms("rxd");
        setDobj("this");
        setPrep("none");
        setIobj("this");
    }, [defaultOwner]);

    // Update argspec and perms when verb type changes
    const handleVerbTypeChange = (type: "method" | "command") => {
        setVerbType(type);
        if (type === "method") {
            setPerms("rxd");
            setDobj("this");
            setPrep("none");
            setIobj("this");
        } else {
            setPerms("rd");
            setDobj("this");
            setPrep("none");
            setIobj("none");
        }
    };

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        onSubmit({ names, owner, perms, dobj, prep, iobj });
    };

    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "520px" }}
                role="dialog"
                aria-modal="true"
                aria-labelledby="add-verb-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="add-verb-title">Add Verb</h2>
                </div>
                <form onSubmit={handleSubmit} className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <p style={{ margin: 0, color: "var(--color-text-secondary)" }}>
                        Add a new verb to <strong>{objectLabel}</strong>.
                    </p>
                    <div style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Verb type</span>
                        <div
                            style={{
                                display: "flex",
                                gap: "1.5em",
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                            }}
                        >
                            <label style={{ display: "flex", alignItems: "center", gap: "0.5em", cursor: "pointer" }}>
                                <input
                                    type="radio"
                                    name="verbType"
                                    checked={verbType === "method"}
                                    onChange={() => handleVerbTypeChange("method")}
                                />
                                <div style={{ display: "flex", flexDirection: "column" }}>
                                    <span style={{ fontWeight: 600 }}>Method</span>
                                    <span style={{ fontSize: "0.85em", color: "var(--color-text-secondary)" }}>
                                        Called from code (<code>this none this</code>, with <code>x</code>)
                                    </span>
                                </div>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.5em", cursor: "pointer" }}>
                                <input
                                    type="radio"
                                    name="verbType"
                                    checked={verbType === "command"}
                                    onChange={() => handleVerbTypeChange("command")}
                                />
                                <div style={{ display: "flex", flexDirection: "column" }}>
                                    <span style={{ fontWeight: 600 }}>Command</span>
                                    <span style={{ fontSize: "0.85em", color: "var(--color-text-secondary)" }}>
                                        Player command (e.g. <code>this none none</code>, no <code>x</code>)
                                    </span>
                                </div>
                            </label>
                        </div>
                    </div>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Verb names (space-separated)</span>
                        <input
                            type="text"
                            value={names}
                            onChange={(e) => setNames(e.target.value)}
                            placeholder="get take grab"
                            autoFocus
                            required
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                        <span style={{ color: "var(--color-text-secondary)", fontSize: "0.85em" }}>
                            Example: <code>get take grab</code> creates aliases for the same verb
                        </span>
                    </label>
                    <label style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Owner (MOO expression)</span>
                        <input
                            type="text"
                            value={owner}
                            onChange={(e) => setOwner(e.target.value)}
                            placeholder="player"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                fontFamily: "var(--font-mono)",
                            }}
                        />
                    </label>
                    <div style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Permissions</span>
                        <span style={{ color: "var(--color-text-secondary)", fontSize: "0.85em" }}>
                            r=read, w=write, x=exec, d=raise errors (usually keep on)
                        </span>
                        <div
                            style={{
                                display: "flex",
                                gap: "0.75em",
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                            }}
                        >
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("r")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "r");
                                        } else {
                                            setPerms(perms.replace("r", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>r</span>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("w")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "w");
                                        } else {
                                            setPerms(perms.replace("w", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>w</span>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("x")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "x");
                                        } else {
                                            setPerms(perms.replace("x", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>x</span>
                            </label>
                            <label style={{ display: "flex", alignItems: "center", gap: "0.35em", cursor: "pointer" }}>
                                <input
                                    type="checkbox"
                                    checked={perms.includes("d")}
                                    onChange={(e) => {
                                        if (e.target.checked) {
                                            setPerms(perms + "d");
                                        } else {
                                            setPerms(perms.replace("d", ""));
                                        }
                                    }}
                                />
                                <span style={{ fontFamily: "var(--font-mono)" }}>d</span>
                            </label>
                        </div>
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: "0.35em" }}>
                        <span style={{ fontWeight: 600 }}>Verb argument specification</span>
                        <div style={{ display: "flex", gap: "0.5em", alignItems: "center" }}>
                            <label style={{ display: "flex", flexDirection: "column", gap: "0.25em", flex: 1 }}>
                                <span style={{ fontSize: "0.85em", color: "var(--color-text-secondary)" }}>dobj</span>
                                <select
                                    value={dobj}
                                    onChange={(e) => setDobj(e.target.value)}
                                    style={{
                                        padding: "0.5em",
                                        borderRadius: "var(--radius-sm)",
                                        border: "1px solid var(--color-border-medium)",
                                        fontFamily: "var(--font-mono)",
                                        backgroundColor: "var(--color-bg-input)",
                                        color: "var(--color-text-primary)",
                                    }}
                                >
                                    <option value="none">none</option>
                                    <option value="any">any</option>
                                    <option value="this">this</option>
                                </select>
                            </label>
                            <label style={{ display: "flex", flexDirection: "column", gap: "0.25em", flex: 1 }}>
                                <span style={{ fontSize: "0.85em", color: "var(--color-text-secondary)" }}>prep</span>
                                <select
                                    value={prep}
                                    onChange={(e) => setPrep(e.target.value)}
                                    style={{
                                        padding: "0.5em",
                                        borderRadius: "var(--radius-sm)",
                                        border: "1px solid var(--color-border-medium)",
                                        fontFamily: "var(--font-mono)",
                                        backgroundColor: "var(--color-bg-input)",
                                        color: "var(--color-text-primary)",
                                    }}
                                >
                                    <option value="none">none</option>
                                    <option value="any">any</option>
                                    <option value="with">with</option>
                                    <option value="at">at</option>
                                    <option value="in-front-of">in-front-of</option>
                                    <option value="in">in</option>
                                    <option value="on">on</option>
                                    <option value="from">from (out of)</option>
                                    <option value="over">over</option>
                                    <option value="through">through</option>
                                    <option value="under">under</option>
                                    <option value="behind">behind</option>
                                    <option value="beside">beside</option>
                                    <option value="for">for</option>
                                    <option value="is">is</option>
                                    <option value="as">as</option>
                                    <option value="off">off</option>
                                </select>
                            </label>
                            <label style={{ display: "flex", flexDirection: "column", gap: "0.25em", flex: 1 }}>
                                <span style={{ fontSize: "0.85em", color: "var(--color-text-secondary)" }}>iobj</span>
                                <select
                                    value={iobj}
                                    onChange={(e) => setIobj(e.target.value)}
                                    style={{
                                        padding: "0.5em",
                                        borderRadius: "var(--radius-sm)",
                                        border: "1px solid var(--color-border-medium)",
                                        fontFamily: "var(--font-mono)",
                                        backgroundColor: "var(--color-bg-input)",
                                        color: "var(--color-text-primary)",
                                    }}
                                >
                                    <option value="none">none</option>
                                    <option value="any">any</option>
                                    <option value="this">this</option>
                                </select>
                            </label>
                        </div>
                    </div>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-accent)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Adding…" : "Add Verb"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
};

interface DeleteVerbDialogProps {
    verbName: string;
    objectLabel: string;
    onCancel: () => void;
    onConfirm: () => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const DeleteVerbDialog: React.FC<DeleteVerbDialogProps> = ({
    verbName,
    objectLabel,
    onCancel,
    onConfirm,
    isSubmitting,
    errorMessage,
}) => {
    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "480px" }}
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="delete-verb-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="delete-verb-title">Remove Verb?</h2>
                </div>
                <div className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <div
                        style={{
                            padding: "0.75em",
                            borderRadius: "var(--radius-sm)",
                            border: "1px solid var(--color-text-error)",
                            backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                            color: "var(--color-text-primary)",
                            fontFamily: "inherit",
                        }}
                    >
                        <p style={{ margin: 0 }}>
                            Remove verb <code>{verbName}</code> from{" "}
                            <strong>{objectLabel}</strong>? This action cannot be undone.
                        </p>
                    </div>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="button"
                            onClick={onConfirm}
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-error)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Removing…" : "Remove Verb"}
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
};

interface DeletePropertyDialogProps {
    propertyName: string;
    objectLabel: string;
    onCancel: () => void;
    onConfirm: () => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const DeletePropertyDialog: React.FC<DeletePropertyDialogProps> = ({
    propertyName,
    objectLabel,
    onCancel,
    onConfirm,
    isSubmitting,
    errorMessage,
}) => {
    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "480px" }}
                role="alertdialog"
                aria-modal="true"
                aria-labelledby="delete-property-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="delete-property-title">Delete Property?</h2>
                </div>
                <div className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <div
                        style={{
                            padding: "0.75em",
                            borderRadius: "var(--radius-sm)",
                            border: "1px solid var(--color-text-error)",
                            backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                            color: "var(--color-text-primary)",
                            fontFamily: "inherit",
                        }}
                    >
                        <p style={{ margin: 0 }}>
                            Delete property <code>{propertyName}</code> from{" "}
                            <strong>{objectLabel}</strong>? This action cannot be undone.
                        </p>
                    </div>
                    {errorMessage && (
                        <div
                            role="alert"
                            style={{
                                padding: "0.5em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-text-error)",
                                backgroundColor: "color-mix(in srgb, var(--color-text-error) 15%, transparent)",
                                color: "var(--color-text-primary)",
                                fontFamily: "var(--font-mono)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}
                    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                                fontWeight: 600,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="button"
                            onClick={onConfirm}
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-text-error)",
                                color: "var(--color-bg-base)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Deleting…" : "Delete Property"}
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
};

interface EditFlagsDialogProps {
    objectLabel: string;
    currentFlags: number;
    onCancel: () => void;
    onSubmit: (flags: number) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const EditFlagsDialog: React.FC<EditFlagsDialogProps> = ({
    objectLabel,
    currentFlags,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [user, setUser] = useState((currentFlags & (1 << 0)) !== 0);
    const [programmer, setProgrammer] = useState((currentFlags & (1 << 1)) !== 0);
    const [wizard, setWizard] = useState((currentFlags & (1 << 2)) !== 0);
    const [readable, setReadable] = useState((currentFlags & (1 << 4)) !== 0);
    const [writable, setWritable] = useState((currentFlags & (1 << 5)) !== 0);
    const [fertile, setFertile] = useState((currentFlags & (1 << 7)) !== 0);

    useEffect(() => {
        setUser((currentFlags & (1 << 0)) !== 0);
        setProgrammer((currentFlags & (1 << 1)) !== 0);
        setWizard((currentFlags & (1 << 2)) !== 0);
        setReadable((currentFlags & (1 << 4)) !== 0);
        setWritable((currentFlags & (1 << 5)) !== 0);
        setFertile((currentFlags & (1 << 7)) !== 0);
    }, [currentFlags]);

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        let flags = 0;
        if (user) flags |= 1 << 0;
        if (programmer) flags |= 1 << 1;
        if (wizard) flags |= 1 << 2;
        if (readable) flags |= 1 << 4;
        if (writable) flags |= 1 << 5;
        if (fertile) flags |= 1 << 7;
        onSubmit(flags);
    };

    const renderCheckbox = (
        label: string,
        description: string,
        checked: boolean,
        onChange: (checked: boolean) => void,
        flagChar: string,
    ) => (
        <div style={{ display: "flex", alignItems: "flex-start", gap: "8px", marginBottom: "12px" }}>
            <input
                type="checkbox"
                checked={checked}
                onChange={(e) => onChange(e.target.checked)}
                disabled={isSubmitting}
                style={{ marginTop: "2px", cursor: isSubmitting ? "not-allowed" : "pointer" }}
            />
            <div style={{ flex: 1 }}>
                <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                    <strong>{label}</strong>
                    <code
                        style={{
                            fontFamily: "var(--font-mono)",
                            fontSize: "0.9em",
                            padding: "1px 4px",
                            backgroundColor: "var(--color-bg-secondary)",
                            borderRadius: "var(--radius-sm)",
                        }}
                    >
                        {flagChar}
                    </code>
                </div>
                <div style={{ fontSize: "0.9em", color: "var(--color-text-secondary)", marginTop: "2px" }}>
                    {description}
                </div>
            </div>
        </div>
    );

    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                className="dialog-sheet"
                style={{ maxWidth: "520px" }}
                role="dialog"
                aria-modal="true"
                aria-labelledby="edit-flags-title"
            >
                <div className="dialog-sheet-header">
                    <h2 id="edit-flags-title">Edit Object Flags</h2>
                </div>
                <form onSubmit={handleSubmit} className="dialog-sheet-content" style={{ gap: "1em" }}>
                    <p style={{ margin: 0, color: "var(--color-text-secondary)" }}>
                        Editing flags for <strong>{objectLabel}</strong>
                    </p>

                    {renderCheckbox(
                        "Player",
                        "Object is a player/user object",
                        user,
                        setUser,
                        "u",
                    )}

                    {renderCheckbox(
                        "Programmer",
                        "Object has programmer rights",
                        programmer,
                        setProgrammer,
                        "p",
                    )}

                    {renderCheckbox(
                        "Wizard",
                        "Object has wizard rights",
                        wizard,
                        setWizard,
                        "w",
                    )}

                    {renderCheckbox(
                        "Readable",
                        "Object is publicly readable",
                        readable,
                        setReadable,
                        "r",
                    )}

                    {renderCheckbox(
                        "Writable",
                        "Object is publicly writable",
                        writable,
                        setWritable,
                        "W",
                    )}

                    {renderCheckbox(
                        "Fertile",
                        "Object can be used as a parent for new objects",
                        fertile,
                        setFertile,
                        "f",
                    )}

                    {errorMessage && (
                        <div
                            style={{
                                padding: "0.75em",
                                backgroundColor: "var(--color-bg-error)",
                                border: "1px solid var(--color-text-error)",
                                borderRadius: "var(--radius-sm)",
                                color: "var(--color-text-error)",
                            }}
                        >
                            {errorMessage}
                        </div>
                    )}

                    <div style={{ display: "flex", gap: "0.5em", justifyContent: "flex-end", marginTop: "1em" }}>
                        <button
                            type="button"
                            onClick={onCancel}
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: "var(--color-bg-secondary)",
                                color: "var(--color-text-primary)",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                            }}
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            style={{
                                padding: "0.5em 1em",
                                borderRadius: "var(--radius-sm)",
                                border: "none",
                                backgroundColor: "var(--color-button-primary)",
                                color: "white",
                                cursor: isSubmitting ? "not-allowed" : "pointer",
                                opacity: isSubmitting ? 0.6 : 1,
                                fontWeight: 700,
                            }}
                        >
                            {isSubmitting ? "Saving…" : "Save Flags"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
};

interface ObjectInfoEditorProps {
    object: ObjectData;
    objects: ObjectData[];
    authToken: string;
    onNavigate: (objectId: string) => void;
    normalizeObjectRef: (raw: string) => { display: string; objectId: string | null };
    normalizeObjectInput: (raw: string) => string;
    onCreateChild: () => void;
    onRecycle: () => void;
    onEditFlags: () => void;
    isSubmittingCreate: boolean;
    isSubmittingRecycle: boolean;
    editingName: string;
    onNameChange: (name: string) => void;
    onNameSave: () => void;
    isSavingName: boolean;
    actionMessage: string | null;
}

const ObjectInfoEditor: React.FC<ObjectInfoEditorProps> = ({
    object,
    objects,
    authToken,
    onNavigate,
    normalizeObjectRef,
    normalizeObjectInput: _normalizeObjectInput,
    onCreateChild,
    onRecycle,
    onEditFlags,
    isSubmittingCreate,
    isSubmittingRecycle,
    editingName,
    onNameChange,
    onNameSave,
    isSavingName,
    actionMessage,
}) => {
    const [children, setChildren] = useState<string[]>([]);
    const [ancestors, setAncestors] = useState<string[]>([]);
    const [descendants, setDescendants] = useState<string[]>([]);
    const [contents, setContents] = useState<string[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [childrenExpanded, setChildrenExpanded] = useState(true);
    const [ancestorsExpanded, setAncestorsExpanded] = useState(true);
    const [descendantsExpanded, setDescendantsExpanded] = useState(true);
    const [contentsExpanded, setContentsExpanded] = useState(true);

    // Helper to extract object ID from FlatBuffer result
    const extractObjectId = (obj: unknown): string | null => {
        if (!obj) return null;

        // The result from performEvalFlatBuffer is already converted via toJS()
        // For objects, this returns { oid?: number; uuid?: string }
        if (typeof obj === "object" && obj !== null) {
            // Check for oid (numbered objects)
            if ("oid" in obj && obj.oid !== undefined && obj.oid !== null) {
                return String(obj.oid);
            }
            // Check for uuid (UUID objects)
            if ("uuid" in obj && obj.uuid !== undefined && obj.uuid !== null) {
                return String(obj.uuid);
            }
            // Fallback checks for other formats
            if ("id" in obj) return String(obj.id);
            if ("objid" in obj) return String(obj.objid);
        }

        // Try as number
        if (typeof obj === "number") {
            return String(obj);
        }

        // Try as string (but not "[object Object]")
        if (typeof obj === "string" && obj !== "[object Object]") {
            return obj;
        }

        return null;
    };

    // Load hierarchy data when object changes
    useEffect(() => {
        const loadHierarchy = async () => {
            setIsLoading(true);
            try {
                const objectRef = `#${object.obj}`;

                // Load children
                const childrenExpr = `return children(${objectRef});`;
                const childrenResult = await performEvalFlatBuffer(authToken, childrenExpr);
                if (Array.isArray(childrenResult)) {
                    const ids = childrenResult.map(extractObjectId).filter((id): id is string => id !== null);
                    setChildren(ids);
                } else {
                    setChildren([]);
                }

                // Load ancestors
                const ancestorsExpr = `return ancestors(${objectRef});`;
                const ancestorsResult = await performEvalFlatBuffer(authToken, ancestorsExpr);
                if (Array.isArray(ancestorsResult)) {
                    const ids = ancestorsResult.map(extractObjectId).filter((id): id is string => id !== null);
                    setAncestors(ids);
                } else {
                    setAncestors([]);
                }

                // Load descendants
                const descendantsExpr = `return descendants(${objectRef});`;
                const descendantsResult = await performEvalFlatBuffer(authToken, descendantsExpr);
                if (Array.isArray(descendantsResult)) {
                    const ids = descendantsResult.map(extractObjectId).filter((id): id is string => id !== null);
                    setDescendants(ids);
                } else {
                    setDescendants([]);
                }

                // Load contents
                const contentsExpr = `return ${objectRef}.contents;`;
                const contentsResult = await performEvalFlatBuffer(authToken, contentsExpr);
                if (Array.isArray(contentsResult)) {
                    const ids = contentsResult.map(extractObjectId).filter((id): id is string => id !== null);
                    setContents(ids);
                } else {
                    setContents([]);
                }
            } catch (error) {
                console.error("Failed to load hierarchy:", error);
            } finally {
                setIsLoading(false);
            }
        };

        loadHierarchy();
    }, [object.obj, authToken]);

    const renderObjectLink = (objId: string) => {
        const { display, objectId } = normalizeObjectRef(objId);

        // Look up object name from the objects list
        const objData = objects.find(o => o.obj === objectId);
        const displayText = objData && objData.name
            ? `${display} ("${objData.name}")`
            : display;

        if (!objectId) {
            return (
                <span
                    style={{
                        fontFamily: "var(--font-mono)",
                        color: "var(--color-text-secondary)",
                    }}
                >
                    {displayText}
                </span>
            );
        }
        return (
            <button
                type="button"
                onClick={() => onNavigate(objectId)}
                style={{
                    background: "none",
                    border: "none",
                    padding: "2px 4px",
                    margin: "0",
                    fontSize: "11px",
                    fontFamily: "var(--font-mono)",
                    color: "var(--color-text-accent)",
                    cursor: "pointer",
                    textDecoration: "underline",
                    borderRadius: "var(--radius-sm)",
                    lineHeight: "1.3",
                }}
                onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                }}
                onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = "transparent";
                }}
            >
                {displayText}
            </button>
        );
    };

    const sectionStyle = {
        marginBottom: "6px",
        border: "1px solid var(--color-border-medium)",
        borderRadius: "var(--radius-sm)",
        backgroundColor: "var(--color-bg-input)",
        fontSize: "11px",
    } as const;

    const sectionHeaderStyle = {
        fontWeight: 600,
        color: "var(--color-text-primary)",
        textTransform: "uppercase" as const,
        letterSpacing: "0.08em",
        fontSize: "10px",
        padding: "4px 8px",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: "4px",
        userSelect: "none" as const,
        backgroundColor: "var(--color-bg-secondary)",
        borderBottom: "1px solid var(--color-border-light)",
    } as const;

    const sectionContentStyle = {
        padding: "6px 8px",
    } as const;

    const listStyle = {
        display: "flex",
        flexWrap: "wrap" as const,
        gap: "4px",
        alignItems: "center",
        lineHeight: "1.3",
    } as const;

    const renderCollapsibleSection = (
        title: string,
        count: number,
        isExpanded: boolean,
        setExpanded: (val: boolean) => void,
        content: React.ReactNode,
    ) => (
        <div style={sectionStyle}>
            <div
                style={sectionHeaderStyle}
                onClick={() => setExpanded(!isExpanded)}
                onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                }}
                onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = "var(--color-bg-secondary)";
                }}
            >
                <span style={{ fontSize: "9px" }}>{isExpanded ? "▼" : "▶"}</span>
                <span>{title} ({count})</span>
            </div>
            {isExpanded && <div style={sectionContentStyle}>{content}</div>}
        </div>
    );

    const renderObjectRefSimple = (raw: string): React.ReactNode => {
        const { display, objectId } = normalizeObjectRef(raw);
        if (!objectId) {
            return (
                <span
                    style={{
                        fontFamily: "var(--font-mono)",
                        border: "1px solid var(--color-border-medium)",
                        borderRadius: "var(--radius-sm)",
                        padding: "2px 6px",
                        fontSize: "0.95em",
                        color: "var(--color-text-secondary)",
                    }}
                >
                    {display}
                </span>
            );
        }
        return (
            <button
                type="button"
                onClick={() => onNavigate(objectId)}
                style={{
                    background: "none",
                    border: "1px solid var(--color-border-medium)",
                    borderRadius: "var(--radius-sm)",
                    padding: "2px 6px",
                    fontSize: "0.95em",
                    fontFamily: "var(--font-mono)",
                    color: "var(--color-text-accent)",
                    cursor: "pointer",
                }}
            >
                {display}
            </button>
        );
    };

    return (
        <div
            style={{
                height: "100%",
                display: "flex",
                flexDirection: "column",
                backgroundColor: "var(--color-bg-secondary)",
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
                        flex: 1,
                    }}
                >
                    <span style={{ fontWeight: "700" }}>Object info</span>
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
                        {object.name
                            ? `${normalizeObjectRef(object.obj).display} ("${object.name}")`
                            : normalizeObjectRef(object.obj).display}
                    </span>
                </h3>
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
                    <button
                        type="button"
                        onClick={onCreateChild}
                        disabled={!object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle}
                        style={{
                            backgroundColor:
                                "color-mix(in srgb, var(--color-text-success) 20%, var(--color-bg-secondary))",
                            color: "var(--color-text-primary)",
                            border: "1px solid var(--color-border-medium)",
                            padding: "6px 12px",
                            borderRadius: "var(--radius-sm)",
                            cursor: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? "not-allowed"
                                : "pointer",
                            opacity: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? 0.6
                                : 1,
                            fontSize: "12px",
                            fontWeight: "600",
                        }}
                    >
                        Create Child
                    </button>
                    <button
                        type="button"
                        onClick={onRecycle}
                        disabled={!object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle}
                        style={{
                            backgroundColor:
                                "color-mix(in srgb, var(--color-text-error) 20%, var(--color-bg-secondary))",
                            color: "var(--color-text-primary)",
                            border: "1px solid var(--color-border-medium)",
                            padding: "6px 12px",
                            borderRadius: "var(--radius-sm)",
                            cursor: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? "not-allowed"
                                : "pointer",
                            opacity: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? 0.6
                                : 1,
                            fontSize: "12px",
                            fontWeight: "600",
                        }}
                    >
                        Recycle
                    </button>
                </div>
            </div>

            {/* Content area with metadata and hierarchy */}
            <div style={{ flex: 1, overflow: "auto" }}>
                {/* Object metadata section */}
                <div
                    style={{
                        padding: "var(--space-sm) var(--space-md)",
                        backgroundColor: "var(--color-bg-tertiary)",
                        borderTop: "1px solid var(--color-border-light)",
                        borderBottom: "1px solid var(--color-border-light)",
                        fontSize: "0.9em",
                        display: "flex",
                        gap: "var(--space-md)",
                        flexWrap: "wrap",
                        alignItems: "center",
                    }}
                >
                    {/* Name editor */}
                    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                            Name:
                        </span>
                        <input
                            type="text"
                            value={editingName}
                            onChange={(e) => onNameChange(e.target.value)}
                            disabled={isSavingName}
                            style={{
                                fontFamily: "var(--font-mono)",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "var(--radius-sm)",
                                padding: "2px 6px",
                                fontSize: "0.95em",
                                minWidth: "120px",
                                backgroundColor: "var(--color-bg-input)",
                                color: "var(--color-text-primary)",
                            }}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                    onNameSave();
                                } else if (e.key === "Escape") {
                                    onNameChange(object.name);
                                }
                            }}
                        />
                        <button
                            type="button"
                            onClick={onNameSave}
                            disabled={isSavingName || editingName === object.name}
                            style={{
                                padding: "2px 8px",
                                fontSize: "0.9em",
                                fontWeight: 600,
                                borderRadius: "var(--radius-sm)",
                                border: "1px solid var(--color-border-medium)",
                                backgroundColor: isSavingName || editingName === object.name
                                    ? "var(--color-bg-secondary)"
                                    : "var(--color-button-primary)",
                                color: isSavingName || editingName === object.name
                                    ? "var(--color-text-secondary)"
                                    : "white",
                                cursor: isSavingName || editingName === object.name ? "not-allowed" : "pointer",
                                opacity: isSavingName || editingName === object.name ? 0.6 : 1,
                            }}
                        >
                            {isSavingName ? "💾" : "💾"}
                        </button>
                    </div>

                    {/* Separator bar */}
                    <div
                        style={{
                            width: "1px",
                            height: "20px",
                            backgroundColor: "var(--color-border-medium)",
                        }}
                    />

                    {/* Flags */}
                    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                            Flags:
                        </span>
                        <button
                            type="button"
                            onClick={onEditFlags}
                            style={{
                                background: "none",
                                fontFamily: "var(--font-mono)",
                                border: "1px solid var(--color-border-medium)",
                                borderRadius: "var(--radius-sm)",
                                padding: "2px 6px",
                                fontSize: "0.95em",
                                color: "var(--color-text-primary)",
                                cursor: "pointer",
                            }}
                        >
                            {formatObjectFlags(object.flags) || "none"}
                        </button>
                    </div>

                    {/* Separator bar */}
                    <div
                        style={{
                            width: "1px",
                            height: "20px",
                            backgroundColor: "var(--color-border-medium)",
                        }}
                    />

                    {/* Owner */}
                    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                            Owner:
                        </span>
                        {renderObjectRefSimple(object.owner)}
                    </div>

                    {/* Separator bar */}
                    <div
                        style={{
                            width: "1px",
                            height: "20px",
                            backgroundColor: "var(--color-border-medium)",
                        }}
                    />

                    {/* Location */}
                    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <span style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-ui)" }}>
                            Location:
                        </span>
                        {renderObjectRefSimple(object.location)}
                    </div>
                </div>

                {/* Action message */}
                {actionMessage && (
                    <div
                        style={{
                            margin: "8px",
                            marginBottom: "8px",
                            padding: "6px 8px",
                            borderRadius: "var(--radius-sm)",
                            backgroundColor: "rgba(16, 185, 129, 0.15)",
                            border: "1px solid rgba(16, 185, 129, 0.35)",
                            color: "var(--color-text-primary)",
                            fontSize: "11px",
                        }}
                    >
                        {actionMessage}
                    </div>
                )}

                {/* Hierarchy sections */}
                <div style={{ padding: "8px", fontSize: "11px" }}>
                    {isLoading ? <div style={{ color: "var(--color-text-secondary)" }}>Loading hierarchy...</div> : (
                        <>
                            {/* Contents Section */}
                            {contents.length > 0 && renderCollapsibleSection(
                                "Contents",
                                contents.length,
                                contentsExpanded,
                                setContentsExpanded,
                                <div style={listStyle}>
                                    {contents.map((contentId, idx) => (
                                        <React.Fragment key={`content-${contentId}-${idx}`}>
                                            {renderObjectLink(contentId)}
                                        </React.Fragment>
                                    ))}
                                </div>,
                            )}

                            {/* Parent & Children Section */}
                            <div style={sectionStyle}>
                                <div
                                    style={{
                                        ...sectionHeaderStyle,
                                        cursor: "default",
                                        backgroundColor: "var(--color-bg-secondary)",
                                    }}
                                >
                                    <span>Parent & Children</span>
                                </div>
                                <div style={sectionContentStyle}>
                                    <div style={{ marginBottom: "4px" }}>
                                        <strong style={{ marginRight: "4px" }}>Parent:</strong>
                                        {renderObjectLink(object.parent)}
                                    </div>
                                    <div>
                                        <button
                                            type="button"
                                            onClick={() => setChildrenExpanded(!childrenExpanded)}
                                            style={{
                                                background: "none",
                                                border: "none",
                                                padding: "0",
                                                cursor: "pointer",
                                                display: "inline-flex",
                                                alignItems: "center",
                                                gap: "4px",
                                                color: "var(--color-text-primary)",
                                                fontWeight: 600,
                                                fontSize: "11px",
                                            }}
                                        >
                                            <span style={{ fontSize: "9px" }}>{childrenExpanded ? "▼" : "▶"}</span>
                                            <span>Children ({children.length})</span>
                                        </button>
                                        {childrenExpanded && (
                                            <div style={{ ...listStyle, marginTop: "4px" }}>
                                                {children.length === 0
                                                    ? (
                                                        <span
                                                            style={{
                                                                color: "var(--color-text-secondary)",
                                                                fontStyle: "italic",
                                                            }}
                                                        >
                                                            none
                                                        </span>
                                                    )
                                                    : (
                                                        children.map((childId, idx) => (
                                                            <React.Fragment key={`child-${childId}-${idx}`}>
                                                                {renderObjectLink(childId)}
                                                            </React.Fragment>
                                                        ))
                                                    )}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </div>

                            {/* Ancestors Section */}
                            {ancestors.length > 0 && renderCollapsibleSection(
                                "Ancestors",
                                ancestors.length,
                                ancestorsExpanded,
                                setAncestorsExpanded,
                                <div style={listStyle}>
                                    {ancestors.map((ancestorId, idx) => (
                                        <React.Fragment key={`ancestor-${ancestorId}-${idx}`}>
                                            {renderObjectLink(ancestorId)}
                                        </React.Fragment>
                                    ))}
                                </div>,
                            )}

                            {/* Descendants Section */}
                            {descendants.length > 0 && renderCollapsibleSection(
                                "Descendants",
                                descendants.length,
                                descendantsExpanded,
                                setDescendantsExpanded,
                                <div style={listStyle}>
                                    {descendants.map((descendantId, idx) => (
                                        <React.Fragment key={`descendant-${descendantId}-${idx}`}>
                                            {renderObjectLink(descendantId)}
                                        </React.Fragment>
                                    ))}
                                </div>,
                            )}
                        </>
                    )}
                </div>
            </div>
        </div>
    );
};
