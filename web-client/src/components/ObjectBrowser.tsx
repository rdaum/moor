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

import React, { useCallback, useEffect, useRef, useState } from "react";
import { useMediaQuery } from "../hooks/useMediaQuery.js";
import { usePersistentState } from "../hooks/usePersistentState.js";
import { useTouchDevice } from "../hooks/useTouchDevice.js";
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
import { objToString, stringToCurie, uuObjIdToString } from "../lib/var.js";
import { DialogSheet } from "./DialogSheet.js";
import { EditorWindow, useTitleBarDrag } from "./EditorWindow.js";
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
    focusedObjectCurie?: string; // Focus on specific object when presentation opens it
    onOpenVerbInEditor?: (title: string, objectCurie: string, verbName: string, content: string) => void;
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
    chown: boolean;
}

interface VerbData {
    names: string[];
    owner: string;
    location: string;
    readable: boolean;
    writable: boolean;
    executable: boolean;
    debug: boolean;
    dobj: string; // ArgSpec string (none/any/this)
    prep: string; // PrepSpec string (none/any/with/at/etc.)
    iobj: string; // ArgSpec string (none/any/this)
    indexInLocation?: number; // Position of this verb within its location object
}

interface CreateChildFormValues {
    parent: string;
    owner: string;
    objectType: string;
    initArgs: string;
    name: string;
    flags: number;
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

interface ReloadObjectFormValues {
    objdefFile: File;
    constantsFile: File | null;
    confirmation: string;
}

interface TestResult {
    verb: string;
    location: string;
    success: boolean;
    result?: string;
    error?: string;
}

const isTestVerb = (name: string): boolean => name.startsWith("test_");

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

const MIN_FONT_SIZE = 10;
const MAX_FONT_SIZE = 20;

const persistNonNull = <T,>(value: T | null): boolean => value !== null;

const clampFontSize = (value: number): number => {
    return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, value));
};

const deserializeFontSize = (raw: string): number | null => {
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
        return null;
    }
    return clampFontSize(parsed);
};

const deserializeStoredString = (raw: string): string | null => {
    if (raw.length === 0) {
        return "";
    }
    try {
        return JSON.parse(raw);
    } catch {
        return raw;
    }
};

const deserializeEditorType = (raw: string): "property" | "verb" | null => {
    const value = deserializeStoredString(raw);
    return value === "property" || value === "verb" ? value : null;
};

const deserializePropertyName = (raw: string): string | null => {
    return deserializeStoredString(raw);
};

const deserializeVerbIndex = (raw: string): number | null => {
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
        return null;
    }
    return parsed;
};

const escapeMooString = (value: string): string => {
    return value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
};

const listToMooLiteral = (items: string[]): string => {
    const parts = items.map(item => `"${escapeMooString(item)}"`);
    return `{${parts.join(", ")}}`;
};

export const ObjectBrowser: React.FC<ObjectBrowserProps> = ({
    visible,
    onClose,
    authToken,
    splitMode = false,
    onSplitDrag,
    onSplitTouchStart,
    onToggleSplitMode,
    isInSplitMode = false,
    focusedObjectCurie,
    onOpenVerbInEditor,
}) => {
    const isMobile = useMediaQuery("(max-width: 768px)");
    const isTouchDevice = useTouchDevice();
    // Use tabbed layout on touch devices with mobile-sized screens
    // The split pane with draggable divider doesn't work well on touch
    const useTabLayout = isMobile && isTouchDevice;
    const [activeTab, setActiveTab] = useState<"objects" | "properties" | "verbs">("objects");
    const [isFullscreen, setIsFullscreen] = useState(useTabLayout); // Start fullscreen on mobile
    const [objects, setObjects] = useState<ObjectData[]>([]);
    const [selectedObject, setSelectedObject] = usePersistentState<ObjectData | null>(
        "moor-object-browser-selected-object",
        null,
        { shouldPersist: persistNonNull },
    );
    const [properties, setProperties] = useState<PropertyData[]>([]);
    const [verbs, setVerbs] = useState<VerbData[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [filter, setFilter] = useState("");
    const [propertyFilter, setPropertyFilter] = useState("");
    const [verbFilter, setVerbFilter] = useState("");
    const containerRef = useRef<HTMLDivElement | null>(null);
    const objectsPaneRef = useRef<HTMLDivElement | null>(null);

    // Editor state
    const [selectedProperty, setSelectedProperty] = useState<PropertyData | null>(null);
    const [selectedVerb, setSelectedVerb] = useState<VerbData | null>(null);
    const [verbCode, setVerbCode] = useState<string>("");
    const [editorVisible, setEditorVisible] = useState(false);

    // Track what type of editor was open (for restoration)
    const [lastEditorType, setLastEditorType] = usePersistentState<"property" | "verb" | null>(
        "moor-object-browser-editor-type",
        null,
        {
            shouldPersist: persistNonNull,
            deserialize: deserializeEditorType,
        },
    );
    const [lastPropertyName, setLastPropertyName] = usePersistentState<string | null>(
        "moor-object-browser-property-name",
        null,
        {
            shouldPersist: persistNonNull,
            deserialize: deserializePropertyName,
        },
    );
    const [lastVerbIndex, setLastVerbIndex] = usePersistentState<number | null>(
        "moor-object-browser-verb-index",
        null,
        {
            shouldPersist: persistNonNull,
            deserialize: deserializeVerbIndex,
        },
    );
    const [lastVerbLocation, setLastVerbLocation] = usePersistentState<string | null>(
        "moor-object-browser-verb-location",
        null,
        {
            shouldPersist: persistNonNull,
            deserialize: deserializeStoredString,
        },
    );

    // Sync selectedVerb when verbs array updates (e.g., after metadata save)
    useEffect(() => {
        if (selectedVerb) {
            const updatedVerb = verbs.find(v =>
                v.location === selectedVerb.location && v.indexInLocation === selectedVerb.indexInLocation
            );
            if (updatedVerb) {
                setSelectedVerb(updatedVerb);
            }
        }
    }, [verbs]); // eslint-disable-line react-hooks/exhaustive-deps

    // Restore verb selection when verbs are loaded (after component remount)
    useEffect(() => {
        if (
            lastEditorType === "verb" && lastVerbIndex !== null && lastVerbLocation !== null && verbs.length > 0
            && !selectedVerb && selectedObject
        ) {
            const verb = verbs.find(v => v.location === lastVerbLocation && v.indexInLocation === lastVerbIndex);
            if (verb) {
                handleVerbSelect(verb);
                // Clear the restoration flags so we don't keep re-selecting
                setLastEditorType(null);
                setLastVerbIndex(null);
                setLastVerbLocation(null);
            }
        }
    }, [verbs, lastEditorType, lastVerbIndex, lastVerbLocation, selectedVerb, selectedObject]); // eslint-disable-line react-hooks/exhaustive-deps

    // Focus on a specific object when presentation opens the browser
    useEffect(() => {
        if (focusedObjectCurie && objects.length > 0) {
            // Use stringToCurie to normalize both the focused CURIE and object strings for comparison
            const normalizedFocusCurie = stringToCurie(focusedObjectCurie);
            const objectToFocus = objects.find(obj => stringToCurie(obj.obj) === normalizedFocusCurie);
            if (objectToFocus) {
                setSelectedObject(objectToFocus);
                setEditingName(objectToFocus.name);
                setSelectedProperty(null);
                setSelectedVerb(null);
                setEditorVisible(false);
                loadPropertiesAndVerbs(objectToFocus);
            }
        }
    }, [focusedObjectCurie, objects]); // eslint-disable-line react-hooks/exhaustive-deps

    // Scroll to selected object when it changes
    useEffect(() => {
        if (selectedObject && objectsPaneRef.current) {
            const selectedElement = objectsPaneRef.current.querySelector(
                `.browser-item[data-obj-id="${selectedObject.obj}"]`,
            );
            if (selectedElement) {
                selectedElement.scrollIntoView({ behavior: "smooth", block: "nearest" });
            }
        }
    }, [selectedObject]);

    const [browserPaneHeight, setBrowserPaneHeight] = useState(350); // Fixed pixel height for browser pane
    const [isSplitDragging, setIsSplitDragging] = useState(false);
    const [fontSize, setFontSize] = usePersistentState(
        "moor-object-browser-font-size",
        () => (isMobile ? 14 : 12),
        { deserialize: deserializeFontSize },
    );
    const [showInheritedProperties, setShowInheritedProperties] = usePersistentState(
        "moor-object-browser-show-inherited-properties",
        true,
    );
    const [showInheritedVerbs, setShowInheritedVerbs] = usePersistentState(
        "moor-object-browser-show-inherited-verbs",
        true,
    );
    const [showTests, setShowTests] = usePersistentState(
        "moor-object-browser-show-tests",
        true,
    );
    const [serverFeatures, setServerFeatures] = useState<ServerFeatureSet | null>(null);
    const [dollarNames, setDollarNames] = useState<Map<string, string>>(new Map());
    const [showCreateDialog, setShowCreateDialog] = useState(false);
    const [showRecycleDialog, setShowRecycleDialog] = useState(false);
    const [showAddPropertyDialog, setShowAddPropertyDialog] = useState(false);
    const [showDeletePropertyDialog, setShowDeletePropertyDialog] = useState(false);
    const [showAddVerbDialog, setShowAddVerbDialog] = useState(false);
    const [showDeleteVerbDialog, setShowDeleteVerbDialog] = useState(false);
    const [showEditFlagsDialog, setShowEditFlagsDialog] = useState(false);
    const [showReloadDialog, setShowReloadDialog] = useState(false);
    const [showTestResultsDialog, setShowTestResultsDialog] = useState(false);
    const [testResults, setTestResults] = useState<TestResult[]>([]);
    const [isRunningTests, setIsRunningTests] = useState(false);
    const [isSubmittingCreate, setIsSubmittingCreate] = useState(false);
    const [isSubmittingRecycle, setIsSubmittingRecycle] = useState(false);
    const [isSubmittingAddProperty, setIsSubmittingAddProperty] = useState(false);
    const [isSubmittingDeleteProperty, setIsSubmittingDeleteProperty] = useState(false);
    const [isSubmittingAddVerb, setIsSubmittingAddVerb] = useState(false);
    const [isSubmittingDeleteVerb, setIsSubmittingDeleteVerb] = useState(false);
    const [isSubmittingEditFlags, setIsSubmittingEditFlags] = useState(false);
    const [isSubmittingReload, setIsSubmittingReload] = useState(false);
    const [createDialogError, setCreateDialogError] = useState<string | null>(null);
    const [recycleDialogError, setRecycleDialogError] = useState<string | null>(null);
    const [addPropertyDialogError, setAddPropertyDialogError] = useState<string | null>(null);
    const [deletePropertyDialogError, setDeletePropertyDialogError] = useState<string | null>(null);
    const [addVerbDialogError, setAddVerbDialogError] = useState<string | null>(null);
    const [deleteVerbDialogError, setDeleteVerbDialogError] = useState<string | null>(null);
    const [editFlagsDialogError, setEditFlagsDialogError] = useState<string | null>(null);
    const [reloadDialogError, setReloadDialogError] = useState<string | null>(null);
    const [actionMessage, setActionMessage] = useState<string | null>(null);
    const [editingName, setEditingName] = useState<string>("");
    const [isSavingName, setIsSavingName] = useState(false);
    const [propertyToDelete, setPropertyToDelete] = useState<PropertyData | null>(null);
    const [verbToDelete, setVerbToDelete] = useState<VerbData | null>(null);
    const decreaseFontSize = useCallback(() => {
        setFontSize(prev => clampFontSize(prev - 1));
    }, [setFontSize]);
    const increaseFontSize = useCallback(() => {
        setFontSize(prev => clampFontSize(prev + 1));
    }, [setFontSize]);

    useEffect(() => {
        if (selectedProperty && editorVisible) {
            setLastEditorType("property");
            setLastPropertyName(selectedProperty.name);
        }
    }, [editorVisible, selectedProperty, setLastEditorType, setLastPropertyName]);

    useEffect(() => {
        if (selectedVerb && editorVisible && selectedVerb.indexInLocation !== undefined) {
            setLastEditorType("verb");
            setLastVerbIndex(selectedVerb.indexInLocation);
            setLastVerbLocation(selectedVerb.location);
        }
    }, [editorVisible, selectedVerb, setLastEditorType, setLastVerbIndex, setLastVerbLocation]);

    // Track previous editorVisible to detect transitions
    const prevEditorVisibleRef = useRef<boolean | undefined>(undefined);
    useEffect(() => {
        const prevVisible = prevEditorVisibleRef.current;
        prevEditorVisibleRef.current = editorVisible;

        // Only clear restoration state when editor closes (true -> false transition)
        // Don't clear on initial mount when editorVisible is false
        if (prevVisible === true && !editorVisible) {
            setLastEditorType(null);
            setLastPropertyName(null);
            setLastVerbIndex(null);
            setLastVerbLocation(null);
        }
    }, [editorVisible, setLastEditorType, setLastPropertyName, setLastVerbIndex, setLastVerbLocation]);

    // Load objects on mount
    useEffect(() => {
        if (visible) {
            loadObjects().then((loadedObjects) => {
                // If we have a saved selection, restore it
                if (selectedObject) {
                    // Find the object in the loaded list
                    const matchingObj = loadedObjects.find(obj => obj.obj === selectedObject.obj);
                    if (matchingObj) {
                        // Reload properties and verbs for the restored selection
                        loadPropertiesAndVerbs(matchingObj).then((loadedProps) => {
                            setEditingName(matchingObj.name);

                            // Restore property selection if we had one
                            if (lastEditorType === "property" && lastPropertyName) {
                                const prop = loadedProps.find(p => p.name === lastPropertyName);
                                if (prop) {
                                    handlePropertySelect(prop);
                                    // Clear the restoration flags
                                    setLastEditorType(null);
                                    setLastPropertyName(null);
                                }
                            }
                            // Verb restoration happens in separate effect after verbs load
                        });
                    } else {
                        // Object no longer exists, clear selection
                        setSelectedObject(null);
                    }
                }
            });
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

    // Fetch $ name mappings from #0 properties
    useEffect(() => {
        if (!visible) {
            return;
        }
        let cancelled = false;
        const fetchDollarNames = async () => {
            try {
                // Evaluate MOO expression to get all property names and their values from #0
                const expr = "return {{x, #0.(x)} for x in (properties(#0))};";
                const result = await performEvalFlatBuffer(authToken, expr);

                if (cancelled) return;

                const nameMap = new Map<string, string>();

                // Handle different possible return formats
                if (Array.isArray(result)) {
                    // If it's an array of [key, value] pairs
                    for (const entry of result) {
                        if (Array.isArray(entry) && entry.length === 2) {
                            const [propName, objRef] = entry;
                            if (typeof propName === "string" && objRef && typeof objRef === "object") {
                                let objId: string | null = null;
                                if ("oid" in objRef && typeof objRef.oid === "number") {
                                    objId = String(objRef.oid);
                                } else if ("uuid" in objRef && typeof objRef.uuid === "string") {
                                    // UUID comes as packed bigint string, need to convert to formatted string
                                    objId = uuObjIdToString(BigInt(objRef.uuid));
                                }
                                if (objId) {
                                    nameMap.set(objId, propName);
                                }
                            }
                        }
                    }
                } else if (result && typeof result === "object") {
                    // If it's an object/map with property names as keys
                    for (const [propName, objRef] of Object.entries(result)) {
                        let objId: string | null = null;
                        if (objRef && typeof objRef === "object") {
                            if ("oid" in objRef && typeof objRef.oid === "number") {
                                objId = String(objRef.oid);
                            } else if ("uuid" in objRef && typeof objRef.uuid === "string") {
                                // UUID comes as packed bigint string, need to convert to formatted string
                                objId = uuObjIdToString(BigInt(objRef.uuid));
                            }
                        }
                        if (objId) {
                            nameMap.set(objId, propName);
                        }
                    }
                }

                setDollarNames(nameMap);
            } catch (error) {
                console.error("Failed to fetch $ names from #0:", error);
            }
        };

        fetchDollarNames();

        return () => {
            cancelled = true;
        };
    }, [visible, authToken]);

    useEffect(() => {
        if (!visible) {
            setShowCreateDialog(false);
            setShowRecycleDialog(false);
            setShowAddPropertyDialog(false);
            setShowDeletePropertyDialog(false);
            setShowReloadDialog(false);
        }
    }, [visible]);

    const loadObjects = async (): Promise<ObjectData[]> => {
        setIsLoading(true);
        let objectList: ObjectData[] = [];
        try {
            const reply = await listObjectsFlatBuffer(authToken);
            const objectsLength = reply.objectsLength();
            const result: ObjectData[] = [];

            // ObjUnion enum: 0=NONE, 1=ObjId, 2=UuObjId, 3=AnonymousObjId
            const ANONYMOUS_OBJ_TYPE = 3;

            for (let i = 0; i < objectsLength; i++) {
                const objInfo = reply.objects(i);
                if (!objInfo) continue;

                const obj = objInfo.obj();

                // Skip anonymous objects - they can't be referenced in eval calls
                if (obj && obj.objType() === ANONYMOUS_OBJ_TYPE) {
                    continue;
                }

                const name = objInfo.name();
                const parent = objInfo.parent();
                const owner = objInfo.owner();
                const location = objInfo.location();

                const objStr = objToString(obj);
                if (!objStr) continue; // Skip objects we can't get an ID for

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
        try {
            const objectCurie = stringToCurie(obj.obj);
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
                    value: null,
                    owner: objToString(owner) || "",
                    definer: objToString(definer) || "",
                    readable: propInfo.r(),
                    writable: propInfo.w(),
                    chown: propInfo.chown(),
                });
            }

            setProperties(propList);

            const verbsReply = await getVerbsFlatBuffer(authToken, objectCurie, true);
            const verbsLength = verbsReply.verbsLength();
            const verbList: VerbData[] = [];
            const locationIndices = new Map<string, number>();

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
                const locationStr = objToString(location) || "";

                // Track index within each location
                if (!locationIndices.has(locationStr)) {
                    locationIndices.set(locationStr, 0);
                }
                const indexInLocation = locationIndices.get(locationStr)!;
                locationIndices.set(locationStr, indexInLocation + 1);

                // arg_spec is a vector of 3 symbols: [dobj, prep, iobj]
                const argSpecLength = verbInfo.argSpecLength();
                const dobj = argSpecLength > 0 ? verbInfo.argSpec(0)?.value() || "none" : "none";
                const prep = argSpecLength > 1 ? verbInfo.argSpec(1)?.value() || "none" : "none";
                const iobj = argSpecLength > 2 ? verbInfo.argSpec(2)?.value() || "none" : "none";

                verbList.push({
                    names,
                    owner: objToString(owner) || "",
                    location: locationStr,
                    readable: verbInfo.r(),
                    writable: verbInfo.w(),
                    executable: verbInfo.x(),
                    debug: verbInfo.d(),
                    dobj,
                    prep,
                    iobj,
                    indexInLocation,
                });
            }

            setVerbs(verbList);
            return propList;
        } catch (error) {
            console.error("Failed to load properties/verbs:", error);
            return []; // Return empty array on error
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
            const objectCurie = stringToCurie(prop.definer);
            const propValue = await getPropertyFlatBuffer(authToken, objectCurie, prop.name);
            const varValue = propValue.value();
            if (varValue) {
                const moorVar = new MoorVar(varValue);
                const jsValue = moorVar.toJS();
                // Update the property with both JS value and MoorVar
                setSelectedProperty({ ...prop, value: jsValue, moorVar });
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
            const objectCurie = stringToCurie(verb.location);
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

    const handleDetachVerbEditor = () => {
        if (!selectedVerb || !onOpenVerbInEditor) return;

        const objectCurie = stringToCurie(selectedVerb.location);
        const title = `#${selectedVerb.location}:${selectedVerb.names.join(" ")}`;

        // Open in the main verb editor system
        onOpenVerbInEditor(title, objectCurie, selectedVerb.names[0], verbCode);

        // Clear the embedded editor and go back to object view
        // Also clear restoration state to prevent the useEffect from re-selecting the verb
        setSelectedVerb(null);
        setEditorVisible(false);
        setActiveTab("objects");
        setLastEditorType(null);
        setLastVerbIndex(null);
        setLastVerbLocation(null);
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
                await performEvalFlatBuffer(authToken, userExpr);
            }

            // Handle other flag changes
            if (assignments.length > 0) {
                const expr = assignments.join("; ") + "; return 1;";
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
        const parentExpr = normalizeObjectInput(form.parent || "#-1");
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
            const previousIds = new Set(objects.map(o => o.obj));
            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "create() failed";
                throw new Error(msg);
            }

            // Extract the created object reference from the result
            let createdObjExpr: string | null = null;
            if (result && typeof result === "object") {
                if ("oid" in result) {
                    const oidResult = result as { oid?: number };
                    if (typeof oidResult.oid === "number") {
                        createdObjExpr = `#${oidResult.oid}`;
                    }
                } else if ("uuid" in result) {
                    const uuidResult = result as { uuid?: string };
                    if (typeof uuidResult.uuid === "string") {
                        // Format UUID properly: FFFFFF-FFFFFFFFFF
                        const packedValue = BigInt(uuidResult.uuid);
                        const formattedUuid = uuObjIdToString(packedValue);
                        createdObjExpr = `#${formattedUuid}`;
                    }
                }
            }
            if (!createdObjExpr) {
                console.error("Could not extract object reference from create() result");
            }

            // Set name if provided
            if (createdObjExpr && form.name.trim().length > 0) {
                const escapedName = form.name.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
                const nameExpr = `${createdObjExpr}.name = "${escapedName}"; return 1;`;
                try {
                    await performEvalFlatBuffer(authToken, nameExpr);
                } catch (error) {
                    console.error("Failed to set name:", error);
                    throw new Error(`Failed to set name: ${error instanceof Error ? error.message : String(error)}`);
                }
            }

            // Set flags if any are set
            if (createdObjExpr && form.flags !== 0) {
                const assignments: string[] = [];
                if ((form.flags & (1 << 1)) !== 0) {
                    assignments.push(`${createdObjExpr}.programmer = 1`);
                }
                if ((form.flags & (1 << 2)) !== 0) {
                    assignments.push(`${createdObjExpr}.wizard = 1`);
                }
                if ((form.flags & (1 << 4)) !== 0) {
                    assignments.push(`${createdObjExpr}.r = 1`);
                }
                if ((form.flags & (1 << 5)) !== 0) {
                    assignments.push(`${createdObjExpr}.w = 1`);
                }
                if ((form.flags & (1 << 7)) !== 0) {
                    assignments.push(`${createdObjExpr}.f = 1`);
                }
                if (assignments.length > 0) {
                    const flagsExpr = assignments.join("; ") + "; return 1;";
                    try {
                        await performEvalFlatBuffer(authToken, flagsExpr);
                    } catch (error) {
                        console.error("Failed to set flags:", error);
                        throw new Error(
                            `Failed to set flags: ${error instanceof Error ? error.message : String(error)}`,
                        );
                    }
                }
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

    const handleDumpObject = async () => {
        if (!selectedObject) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setActionMessage("Unable to determine object reference.");
            return;
        }

        try {
            const expr = `return dump_object(${objectExpr});`;
            const result = await performEvalFlatBuffer(authToken, expr);

            // Check for error
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "dump_object() failed";
                throw new Error(msg);
            }

            // Result should be an array of strings
            if (!Array.isArray(result)) {
                throw new Error("dump_object() returned unexpected result");
            }

            // Join the lines with newlines
            const content = result.join("\n");

            // Create a blob and download it
            const blob = new Blob([content], { type: "text/plain" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `${objectExpr.replace("#", "")}.moo`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);

            setActionMessage(`Dumped ${describeObject(selectedObject)} to file`);
            setTimeout(() => setActionMessage(null), 3000);
        } catch (error) {
            console.error("Failed to dump object:", error);
            setActionMessage(`Failed to dump object: ${error instanceof Error ? error.message : String(error)}`);
            setTimeout(() => setActionMessage(null), 5000);
        }
    };

    const readFileAsText = (file: File): Promise<string> => {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(String(reader.result ?? ""));
            reader.onerror = () => reject(reader.error || new Error("Failed to read file"));
            reader.readAsText(file);
        });
    };

    const handleReloadObjectSubmit = async (form: ReloadObjectFormValues) => {
        if (!selectedObject) return;

        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr || objectExpr === "#-1") {
            setReloadDialogError("Unable to determine object reference.");
            return;
        }

        setIsSubmittingReload(true);
        setReloadDialogError(null);

        try {
            const objdefText = await readFileAsText(form.objdefFile);
            const objdefLines = objdefText.split(/\r?\n/);
            const objdefLiteral = listToMooLiteral(objdefLines);

            let expr = `return reload_object(${objdefLiteral}, [], ${objectExpr});`;

            if (form.constantsFile) {
                const constantsText = await readFileAsText(form.constantsFile);
                const constantsLines = constantsText.split(/\r?\n/);
                const constantsLiteral = listToMooLiteral(constantsLines);
                expr = `constants = parse_objdef_constants(${constantsLiteral}); `
                    + `return reload_object(${objdefLiteral}, constants, ${objectExpr});`;
            }

            const result = await performEvalFlatBuffer(authToken, expr);
            if (result && typeof result === "object" && "error" in result) {
                const errorResult = result as { error?: { msg?: string } };
                const msg = errorResult.error?.msg ?? "reload_object() failed";
                throw new Error(msg);
            }

            const updatedObjects = await loadObjects();
            const updated = updatedObjects.find(obj => obj.obj === selectedObject.obj);
            if (updated) {
                setSelectedObject(updated);
                setEditingName(updated.name);
                await loadPropertiesAndVerbs(updated);
            }

            setShowReloadDialog(false);
            setActionMessage(`Reloaded ${describeObject(selectedObject)}`);
            setTimeout(() => setActionMessage(null), 3000);
        } catch (error) {
            setReloadDialogError(error instanceof Error ? error.message : String(error));
        } finally {
            setIsSubmittingReload(false);
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
    const handleMouseMove = useCallback((e: MouseEvent) => {
        if (isSplitDragging && containerRef.current) {
            const rect = containerRef.current.getBoundingClientRect();
            const relativeY = e.clientY - rect.top;
            // Calculate the height for the browser pane, accounting for the title bar
            // Find the title bar height (we'll subtract it)
            const titleBar = containerRef.current.querySelector("[aria-labelledby=\"object-browser-title\"]")
                ?.children[0];
            const titleBarHeight = titleBar ? (titleBar as HTMLElement).offsetHeight : 0;
            const availableHeight = rect.height - titleBarHeight;

            // Set minimum and maximum heights (20% to 80% of available height)
            const minHeight = availableHeight * 0.2;
            const maxHeight = availableHeight * 0.8;
            const newHeight = Math.max(minHeight, Math.min(maxHeight, relativeY - titleBarHeight));
            setBrowserPaneHeight(newHeight);
        }
    }, [isSplitDragging]);

    const handleMouseUp = useCallback(() => {
        setIsSplitDragging(false);
    }, []);

    const handleRunTest = async (verb: VerbData) => {
        if (!selectedObject) return;
        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr) return;

        // Use the first name for calling, or the one starting with test_
        const verbName = verb.names.find(n => isTestVerb(n)) || verb.names[0];
        // For now, assume test verbs don't take arguments or we pass none
        const expr = `return ${objectExpr}:${verbName}();`;

        setActionMessage(`Running test ${verbName}...`);
        try {
            const result = await performEvalFlatBuffer(authToken, expr);
            let success = true;
            let resultStr = "";
            let errorStr = undefined;

            if (result && typeof result === "object" && "error" in result) {
                success = false;
                const errorResult = result as { error?: { msg?: string } };
                errorStr = errorResult.error?.msg ?? "Test failed";
            } else if (result !== undefined) {
                // Try to format result nicely
                if (typeof result === "object") {
                    if ("oid" in result) {
                        resultStr = `#${result.oid}`;
                    } else if ("uuid" in result) {
                        resultStr = `#${uuObjIdToString(BigInt(result.uuid as string))}`;
                    } else {
                        resultStr = JSON.stringify(result);
                    }
                } else {
                    resultStr = String(result);
                }
            }

            setTestResults([{
                verb: verbName,
                location: verb.location,
                success,
                result: resultStr,
                error: errorStr,
            }]);
            setShowTestResultsDialog(true);
            setActionMessage(null);
        } catch (error) {
            setTestResults([{
                verb: verbName,
                location: verb.location,
                success: false,
                error: error instanceof Error ? error.message : String(error),
            }]);
            setShowTestResultsDialog(true);
            setActionMessage(null);
        }
    };

    const handleRunAllTests = async () => {
        if (!selectedObject) return;
        const objectExpr = normalizeObjectInput(selectedObject.obj ? `#${selectedObject.obj}` : "");
        if (!objectExpr) return;

        // Find all test verbs for this object (excluding inherited ones)
        const testVerbs = verbs.filter(v => v.names.some(n => isTestVerb(n)) && v.location === selectedObject.obj);

        if (testVerbs.length === 0) {
            setActionMessage("No test verbs found.");
            setTimeout(() => setActionMessage(null), 3000);
            return;
        }

        setIsRunningTests(true);
        setActionMessage(`Running ${testVerbs.length} tests...`);
        const results: TestResult[] = [];

        for (const verb of testVerbs) {
            const verbName = verb.names.find(n => isTestVerb(n)) || verb.names[0];
            const expr = `return ${objectExpr}:${verbName}();`;

            try {
                const result = await performEvalFlatBuffer(authToken, expr);
                let success = true;
                let resultStr = "";
                let errorStr = undefined;

                if (result && typeof result === "object" && "error" in result) {
                    success = false;
                    const errorResult = result as { error?: { msg?: string } };
                    errorStr = errorResult.error?.msg ?? "Test failed";
                } else {
                    if (typeof result === "object") {
                        if ("oid" in result) {
                            resultStr = `#${result.oid}`;
                        } else if ("uuid" in result) {
                            resultStr = `#${uuObjIdToString(BigInt(result.uuid as string))}`;
                        } else {
                            resultStr = JSON.stringify(result);
                        }
                    } else {
                        resultStr = String(result);
                    }
                }
                results.push({
                    verb: verbName,
                    location: verb.location,
                    success,
                    result: resultStr,
                    error: errorStr,
                });
            } catch (error) {
                results.push({
                    verb: verbName,
                    location: verb.location,
                    success: false,
                    error: error instanceof Error ? error.message : String(error),
                });
            }
        }

        setTestResults(results);
        setShowTestResultsDialog(true);
        setIsRunningTests(false);
        setActionMessage(null);
    };

    const handleSplitDragStart = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        setIsSplitDragging(true);
        e.preventDefault();
        e.stopPropagation();
    }, []);

    const handleSplitTouchStart = useCallback((e: React.TouchEvent) => {
        setIsSplitDragging(true);
        e.preventDefault();
        e.stopPropagation();
    }, []);

    const handleTouchMove = useCallback((e: TouchEvent) => {
        if (isSplitDragging && containerRef.current) {
            const touch = e.touches[0];
            const rect = containerRef.current.getBoundingClientRect();
            const relativeY = touch.clientY - rect.top;
            const titleBar = containerRef.current.querySelector("[aria-labelledby=\"object-browser-title\"]")
                ?.children[0];
            const titleBarHeight = titleBar ? (titleBar as HTMLElement).offsetHeight : 0;
            const availableHeight = rect.height - titleBarHeight;

            const minHeight = availableHeight * 0.2;
            const maxHeight = availableHeight * 0.8;
            const newHeight = Math.max(minHeight, Math.min(maxHeight, relativeY - titleBarHeight));
            setBrowserPaneHeight(newHeight);
        }
    }, [isSplitDragging]);

    const handleTouchEnd = useCallback(() => {
        setIsSplitDragging(false);
    }, []);

    // Group properties by definer
    const groupedProperties = React.useMemo(() => {
        const filterLower = propertyFilter.toLowerCase();
        const filteredProps = properties.filter(prop => prop.name.toLowerCase().includes(filterLower));

        // Track the order definers appear in the original array (API order = ancestor order)
        const definerOrder = new Map<string, number>();
        for (const prop of properties) {
            if (!definerOrder.has(prop.definer)) {
                definerOrder.set(prop.definer, definerOrder.size);
            }
        }

        const groups = new Map<string, PropertyData[]>();
        for (const prop of filteredProps) {
            const definer = prop.definer;
            if (!groups.has(definer)) {
                groups.set(definer, []);
            }
            groups.get(definer)!.push(prop);
        }
        let entries = Array.from(groups.entries()).sort((a, b) => {
            // Current object always first
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            // Otherwise preserve API order (nearest ancestor first)
            const orderA = definerOrder.get(a[0]) ?? Infinity;
            const orderB = definerOrder.get(b[0]) ?? Infinity;
            return orderA - orderB;
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
        let filteredVerbs = verbs.filter(verb => verb.names.some(name => name.toLowerCase().includes(filterLower)));

        if (!showTests) {
            filteredVerbs = filteredVerbs.filter(verb => !verb.names.some(name => isTestVerb(name)));
        }

        // Track the order locations appear in the original array (API order = ancestor order)
        const locationOrder = new Map<string, number>();
        for (const verb of verbs) {
            if (!locationOrder.has(verb.location)) {
                locationOrder.set(verb.location, locationOrder.size);
            }
        }

        const groups = new Map<string, VerbData[]>();
        for (const verb of filteredVerbs) {
            const location = verb.location;
            if (!groups.has(location)) {
                groups.set(location, []);
            }
            groups.get(location)!.push(verb);
        }

        let entries = Array.from(groups.entries()).sort((a, b) => {
            // Current object always first
            if (selectedObject && a[0] === selectedObject.obj) return -1;
            if (selectedObject && b[0] === selectedObject.obj) return 1;
            // Otherwise preserve API order (nearest ancestor first)
            const orderA = locationOrder.get(a[0]) ?? Infinity;
            const orderB = locationOrder.get(b[0]) ?? Infinity;
            return orderA - orderB;
        });
        if (!showInheritedVerbs && selectedObject) {
            const currentId = selectedObject.obj;
            entries = entries.filter(([location]) => location === currentId);
        }
        return entries;
    }, [verbs, selectedObject, verbFilter, showInheritedVerbs, showTests]);

    // Track which verbs are overridden or have duplicate names
    const verbLabels = React.useMemo(() => {
        const overridden = new Set<string>(); // Set of "location:index" keys for overridden verbs
        const duplicateNames = new Set<string>(); // Set of "location:index" keys for duplicate names
        const seenNamesGlobal = new Set<string>(); // Verb names seen across all locations

        for (const [location, verbList] of groupedVerbs) {
            const seenNamesInLocation = new Map<string, number>(); // Track name counts per location

            for (const verb of verbList) {
                const verbName = verb.names[0];
                const key = `${location}:${verb.indexInLocation}`;

                // Check if this is a duplicate name within the same location
                if (seenNamesInLocation.has(verbName)) {
                    duplicateNames.add(key);
                } else {
                    seenNamesInLocation.set(verbName, 1);
                }

                // Check if this verb name was seen in a more-specific location (overridden)
                if (seenNamesGlobal.has(verbName)) {
                    overridden.add(key);
                } else {
                    seenNamesGlobal.add(verbName);
                }
            }
        }

        return { overridden, duplicateNames };
    }, [groupedVerbs]);

    // Add global mouse/touch event listeners for internal split dragging
    useEffect(() => {
        if (isSplitDragging) {
            document.addEventListener("mousemove", handleMouseMove);
            document.addEventListener("mouseup", handleMouseUp);
            document.addEventListener("touchmove", handleTouchMove, { passive: false });
            document.addEventListener("touchend", handleTouchEnd);
            document.body.style.userSelect = "none";

            return () => {
                document.removeEventListener("mousemove", handleMouseMove);
                document.removeEventListener("mouseup", handleMouseUp);
                document.removeEventListener("touchmove", handleTouchMove);
                document.removeEventListener("touchend", handleTouchEnd);
                document.body.style.userSelect = "";
            };
        }
    }, [isSplitDragging, handleMouseMove, handleMouseUp, handleTouchMove, handleTouchEnd]);

    const baseFontSize = fontSize;
    const secondaryFontSize = Math.max(8, fontSize - 1);

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

    // Helper to check if object ID is UUID-based (contains "-" like "FFFFFF-FFFFFFFFFF")
    const isUuidObject = (objId: string): boolean => {
        return objId.includes("-");
    };

    // Filter and group objects by type
    const filteredObjects = objects
        .filter(obj => {
            const filterLower = filter.toLowerCase();
            // Strip leading $ for matching against dollarNames
            const filterNormalized = filterLower.startsWith("$") ? filterLower.slice(1) : filterLower;
            const dollarName = dollarNames.get(obj.obj);
            return obj.name.toLowerCase().includes(filterLower)
                || obj.obj.includes(filter)
                || (dollarName && dollarName.toLowerCase().includes(filterNormalized));
        });

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

    const getDollarName = (objId: string): string | null => {
        return dollarNames.get(objId) || null;
    };

    const describeObject = (obj: ObjectData): string => {
        const id = normalizeObjectInput(obj.obj) || "#?";
        return obj.name ? `${id} ("${obj.name}")` : id;
    };

    // Format an object reference for the "from ..." header
    // Shows: $name / #id ("object name") or just #id ("object name") if no $ name
    const formatInheritedFrom = (objId: string): string => {
        const dollarName = getDollarName(objId);
        const objData = objects.find(o => o.obj === objId);
        const displayRef = normalizeObjectRef(objId).display;

        let result = "";
        if (dollarName) {
            result = `$${dollarName} / `;
        }
        result += displayRef;
        if (objData?.name) {
            result += ` ("${objData.name}")`;
        }
        return result;
    };

    const isSplitDraggable = splitMode && typeof onSplitDrag === "function";

    // Title bar component that uses the drag hook (must be inside EditorWindow)
    const TitleBar: React.FC = () => {
        const titleBarDragProps = useTitleBarDrag();

        return (
            <div
                {...(isSplitDraggable
                    ? {
                        onMouseDown: onSplitDrag,
                        onTouchStart: onSplitTouchStart,
                        style: {
                            cursor: "row-resize",
                            touchAction: "none",
                        },
                    }
                    : titleBarDragProps)}
                className="editor-title-bar"
            >
                <h3 id="object-browser-title" className="editor-title">
                    Object Browser
                </h3>
                <div className="flex gap-sm">
                    <div className="font-size-control" onClick={(e) => e.stopPropagation()}>
                        <button
                            onClick={decreaseFontSize}
                            aria-label="Decrease browser font size"
                            className="font-size-button"
                            style={{
                                cursor: fontSize <= MIN_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize <= MIN_FONT_SIZE ? 0.5 : 1,
                                fontSize: `${secondaryFontSize}px`,
                            }}
                            disabled={fontSize <= MIN_FONT_SIZE}
                        >
                            –
                        </button>
                        <span
                            className="font-size-display"
                            style={{ fontSize: `${secondaryFontSize}px` }}
                            aria-live="polite"
                        >
                            {fontSize}px
                        </span>
                        <button
                            onClick={increaseFontSize}
                            aria-label="Increase browser font size"
                            className="font-size-button"
                            style={{
                                cursor: fontSize >= MAX_FONT_SIZE ? "not-allowed" : "pointer",
                                opacity: fontSize >= MAX_FONT_SIZE ? 0.5 : 1,
                                fontSize: `${secondaryFontSize}px`,
                            }}
                            disabled={fontSize >= MAX_FONT_SIZE}
                        >
                            +
                        </button>
                    </div>
                    <div className="browser-inherited-controls" onClick={(e) => e.stopPropagation()}>
                        <span className="browser-inherited-label-text">
                            Inherited
                        </span>
                        <button
                            type="button"
                            className={`browser-inherited-toggle ${showInheritedProperties ? "active" : ""}`}
                            onClick={() => setShowInheritedProperties(prev => !prev)}
                            aria-label="Show inherited properties"
                            aria-pressed={showInheritedProperties}
                            title={showInheritedProperties
                                ? "Hide inherited properties"
                                : "Show inherited properties"}
                        >
                            P
                        </button>
                        <button
                            type="button"
                            className={`browser-inherited-toggle ${showInheritedVerbs ? "active" : ""}`}
                            onClick={() => setShowInheritedVerbs(prev => !prev)}
                            aria-label="Show inherited verbs"
                            aria-pressed={showInheritedVerbs}
                            title={showInheritedVerbs ? "Hide inherited verbs" : "Show inherited verbs"}
                        >
                            V
                        </button>
                        <button
                            type="button"
                            className={`browser-inherited-toggle ${showTests ? "active" : ""}`}
                            onClick={() => setShowTests(prev => !prev)}
                            aria-label="Show test verbs"
                            aria-pressed={showTests}
                            title={showTests ? "Hide test verbs" : "Show test verbs"}
                        >
                            T
                        </button>
                    </div>
                    {/* Split/Float toggle button - only on non-touch devices */}
                    {!isTouchDevice && onToggleSplitMode && (
                        <button
                            className="browser-mode-toggle"
                            onClick={(e) => {
                                e.stopPropagation();
                                onToggleSplitMode();
                            }}
                            aria-label={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            title={isInSplitMode ? "Switch to floating window" : "Switch to split screen"}
                            style={{ fontSize: `${secondaryFontSize}px` }}
                        >
                            {isInSplitMode ? "🪟" : "⬌"}
                        </button>
                    )}
                    {/* Fullscreen toggle button */}
                    <button
                        className="browser-mode-toggle"
                        onClick={(e) => {
                            e.stopPropagation();
                            setIsFullscreen(prev => !prev);
                        }}
                        aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
                        title={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
                        style={{ fontSize: `${secondaryFontSize}px` }}
                    >
                        {isFullscreen ? "🗗" : "🗖"}
                    </button>
                    <button
                        className="editor-btn-close"
                        onClick={onClose}
                        aria-label="Close object browser"
                    >
                        <span aria-hidden="true">×</span>
                    </button>
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
            defaultSize={{ width: 1000, height: 700 }}
            minSize={{ width: 600, height: 400 }}
            ariaLabel="Object Browser"
            className={`object_browser_container ${isFullscreen ? "fullscreen-mobile" : ""}`}
        >
            <div
                ref={containerRef}
                style={{ fontSize: `${baseFontSize}px`, display: "flex", flexDirection: "column", height: "100%" }}
            >
                <TitleBar />

                {/* Main content area - 3 panes + editor */}
                <div className="browser-content">
                    {/* Tab navigation for small screens */}
                    {useTabLayout && (
                        <div className="browser-tabs">
                            <button
                                className={`browser-tab ${activeTab === "objects" ? "active" : ""}`}
                                onClick={() => setActiveTab("objects")}
                            >
                                Objects
                            </button>
                            <button
                                className={`browser-tab ${activeTab === "properties" ? "active" : ""}`}
                                onClick={() => setActiveTab("properties")}
                                disabled={!selectedObject}
                            >
                                Properties
                            </button>
                            <button
                                className={`browser-tab ${activeTab === "verbs" ? "active" : ""}`}
                                onClick={() => setActiveTab("verbs")}
                                disabled={!selectedObject}
                            >
                                Verbs
                            </button>
                        </div>
                    )}

                    {/* Top area - 3 panes */}
                    <div
                        className={`browser-panes ${useTabLayout ? "tabbed" : ""}`}
                        style={{
                            height: (editorVisible || selectedObject)
                                ? `${browserPaneHeight}px`
                                : "100%",
                        }}
                    >
                        {/* Objects pane */}
                        <div
                            className={`browser-pane ${!useTabLayout || activeTab === "objects" ? "active" : ""}`}
                            role="region"
                            aria-label="Objects"
                        >
                            <div className="browser-pane-header">
                                <span
                                    className="browser-pane-title"
                                    style={{ fontSize: `${secondaryFontSize}px` }}
                                >
                                    Objects
                                </span>
                                <button
                                    type="button"
                                    className="btn btn-sm"
                                    onClick={() => {
                                        setShowCreateDialog(true);
                                    }}
                                    style={{ fontSize: `${secondaryFontSize}px` }}
                                    title="Add new object"
                                >
                                    + Add
                                </button>
                            </div>
                            <div className="p-sm border-bottom bg-secondary">
                                <input
                                    type="text"
                                    placeholder="Filter objects..."
                                    value={filter}
                                    onChange={(e) => setFilter(e.target.value)}
                                    className="w-full p-xs border rounded-sm"
                                    style={{ fontSize: `${baseFontSize}px` }}
                                />
                            </div>
                            <div
                                ref={objectsPaneRef}
                                className="browser-pane-content"
                                style={{ fontSize: `${baseFontSize}px` }}
                            >
                                {isLoading
                                    ? (
                                        <div className="p-md text-secondary">
                                            Loading objects...
                                        </div>
                                    )
                                    : (
                                        <>
                                            {/* Numeric OID objects */}
                                            {numericObjects.map((obj) => {
                                                const dollarName = getDollarName(obj.obj);
                                                return (
                                                    <div
                                                        key={obj.obj}
                                                        data-obj-id={obj.obj}
                                                        className={`browser-item ${
                                                            selectedObject?.obj === obj.obj ? "selected" : ""
                                                        }`}
                                                        onClick={() => handleObjectSelect(obj)}
                                                        onKeyDown={(e) => {
                                                            if (e.key === "Enter" || e.key === " ") {
                                                                e.preventDefault();
                                                                handleObjectSelect(obj);
                                                            }
                                                        }}
                                                        tabIndex={0}
                                                        role="button"
                                                        aria-pressed={selectedObject?.obj === obj.obj}
                                                    >
                                                        <div className="browser-item-name font-bold">
                                                            {dollarName ? `$${dollarName} / ` : ""}#{obj.obj}{" "}
                                                            {obj.name && `("${obj.name}")`}{" "}
                                                            {formatObjectFlags(obj.flags) && (
                                                                <span
                                                                    className="text-secondary"
                                                                    style={{
                                                                        opacity: selectedObject?.obj === obj.obj
                                                                            ? "0.7"
                                                                            : "1",
                                                                        color: selectedObject?.obj === obj.obj
                                                                            ? "inherit"
                                                                            : undefined,
                                                                        fontWeight: "400",
                                                                    }}
                                                                >
                                                                    ({formatObjectFlags(obj.flags)})
                                                                </span>
                                                            )}
                                                        </div>
                                                    </div>
                                                );
                                            })}

                                            {/* Separator and UUID objects section */}
                                            {uuidObjects.length > 0 && (
                                                <>
                                                    <div
                                                        className="browser-inherited-label"
                                                        style={{
                                                            borderTop: "2px solid var(--color-border-medium)",
                                                            fontSize: `${secondaryFontSize}px`,
                                                        }}
                                                    >
                                                        UUID Objects
                                                    </div>
                                                    {uuidObjects.map((obj) => {
                                                        const dollarName = getDollarName(obj.obj);
                                                        return (
                                                            <div
                                                                key={obj.obj}
                                                                data-obj-id={obj.obj}
                                                                className={`browser-item ${
                                                                    selectedObject?.obj === obj.obj
                                                                        ? "selected"
                                                                        : ""
                                                                }`}
                                                                onClick={() => handleObjectSelect(obj)}
                                                                onKeyDown={(e) => {
                                                                    if (e.key === "Enter" || e.key === " ") {
                                                                        e.preventDefault();
                                                                        handleObjectSelect(obj);
                                                                    }
                                                                }}
                                                                tabIndex={0}
                                                                role="button"
                                                                aria-pressed={selectedObject?.obj === obj.obj}
                                                            >
                                                                <div className="browser-item-name font-bold">
                                                                    {dollarName ? `$${dollarName} / ` : ""}#{obj
                                                                        .obj} {obj.name && `("${obj.name}")`}{" "}
                                                                    {formatObjectFlags(obj.flags) && (
                                                                        <span
                                                                            className="text-secondary"
                                                                            style={{
                                                                                opacity: selectedObject?.obj === obj.obj
                                                                                    ? "0.7"
                                                                                    : "1",
                                                                                color: selectedObject?.obj === obj.obj
                                                                                    ? "inherit"
                                                                                    : undefined,
                                                                                fontWeight: "400",
                                                                            }}
                                                                        >
                                                                            ({formatObjectFlags(obj.flags)})
                                                                        </span>
                                                                    )}
                                                                </div>
                                                            </div>
                                                        );
                                                    })}
                                                </>
                                            )}
                                        </>
                                    )}
                            </div>
                        </div>

                        {/* Properties pane */}
                        <div
                            className={`browser-pane ${!useTabLayout || activeTab === "properties" ? "active" : ""}`}
                            role="region"
                            aria-label="Properties"
                        >
                            <div className="browser-pane-header">
                                <span
                                    className="browser-pane-title"
                                    style={{ fontSize: `${secondaryFontSize}px` }}
                                >
                                    Properties
                                </span>
                                {selectedObject && (
                                    <button
                                        type="button"
                                        className="btn btn-sm"
                                        onClick={() => {
                                            setAddPropertyDialogError(null);
                                            setActionMessage(null);
                                            setShowAddPropertyDialog(true);
                                        }}
                                        disabled={isSubmittingAddProperty}
                                        aria-label="Add property"
                                        title="Add property"
                                        style={{
                                            cursor: isSubmittingAddProperty ? "not-allowed" : "pointer",
                                            opacity: isSubmittingAddProperty ? 0.6 : 1,
                                            fontSize: `${secondaryFontSize}px`,
                                        }}
                                    >
                                        + Add
                                    </button>
                                )}
                            </div>
                            <div className="p-sm border-bottom bg-secondary">
                                <input
                                    type="text"
                                    placeholder="Filter properties..."
                                    value={propertyFilter}
                                    onChange={(e) => setPropertyFilter(e.target.value)}
                                    className="w-full p-xs border rounded-sm"
                                    style={{ fontSize: `${baseFontSize}px` }}
                                />
                            </div>
                            <div
                                className="browser-pane-content"
                                style={{ fontSize: `${baseFontSize}px` }}
                            >
                                {!selectedObject
                                    ? (
                                        <div className="p-md text-secondary">
                                            Select an object to view properties
                                        </div>
                                    )
                                    : properties.length === 0
                                    ? (
                                        <div className="p-md text-secondary">
                                            No properties
                                        </div>
                                    )
                                    : (
                                        groupedProperties.map(([definer, props], _groupIdx) => (
                                            <div key={definer}>
                                                {definer !== selectedObject.obj && showInheritedProperties && (
                                                    <div
                                                        className="browser-inherited-label"
                                                        style={{ fontSize: `${secondaryFontSize}px` }}
                                                    >
                                                        from {formatInheritedFrom(definer)}
                                                    </div>
                                                )}
                                                {props.map((prop, idx) => (
                                                    <div
                                                        key={`${definer}-${idx}`}
                                                        className={`browser-item ${
                                                            selectedProperty?.name === prop.name
                                                                && selectedProperty?.definer === prop.definer
                                                                ? "selected"
                                                                : ""
                                                        }`}
                                                        onClick={() => handlePropertySelect(prop)}
                                                        onKeyDown={(e) => {
                                                            if (e.key === "Enter" || e.key === " ") {
                                                                e.preventDefault();
                                                                handlePropertySelect(prop);
                                                            }
                                                        }}
                                                        tabIndex={0}
                                                        role="button"
                                                        aria-pressed={selectedProperty?.name === prop.name
                                                            && selectedProperty?.definer === prop.definer}
                                                    >
                                                        <div className="browser-item-name font-bold">
                                                            {prop.name}{" "}
                                                            <span
                                                                className="text-secondary"
                                                                style={{
                                                                    opacity: selectedProperty?.name === prop.name
                                                                            && selectedProperty?.definer
                                                                                === prop.definer
                                                                        ? "0.7"
                                                                        : "1",
                                                                    color: selectedProperty?.name === prop.name
                                                                            && selectedProperty?.definer
                                                                                === prop.definer
                                                                        ? "inherit"
                                                                        : undefined,
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
                            className={`browser-pane ${!useTabLayout || activeTab === "verbs" ? "active" : ""}`}
                            role="region"
                            aria-label="Verbs"
                        >
                            <div className="browser-pane-header">
                                <span
                                    className="browser-pane-title"
                                    style={{ fontSize: `${secondaryFontSize}px` }}
                                >
                                    Verbs
                                </span>
                                {selectedObject && (
                                    <div className="flex gap-xs">
                                        <button
                                            type="button"
                                            className="btn btn-sm"
                                            onClick={handleRunAllTests}
                                            disabled={isRunningTests
                                                || verbs.every(v =>
                                                    !v.names.some(n => isTestVerb(n))
                                                    || v.location !== selectedObject.obj
                                                )}
                                            aria-label="Run all tests"
                                            title="Run all tests"
                                            style={{
                                                cursor: isRunningTests || verbs.every(v =>
                                                        !v.names.some(n => isTestVerb(n))
                                                        || v.location !== selectedObject.obj
                                                    )
                                                    ? "not-allowed"
                                                    : "pointer",
                                                opacity: isRunningTests
                                                        || verbs.every(v =>
                                                            !v.names.some(n => isTestVerb(n))
                                                            || v.location !== selectedObject.obj
                                                        )
                                                    ? 0.6
                                                    : 1,
                                                fontSize: `${secondaryFontSize}px`,
                                            }}
                                        >
                                            🧪 Run Tests
                                        </button>
                                        <button
                                            type="button"
                                            className="btn btn-sm"
                                            onClick={() => {
                                                setAddVerbDialogError(null);
                                                setActionMessage(null);
                                                setShowAddVerbDialog(true);
                                            }}
                                            disabled={isSubmittingAddVerb}
                                            aria-label="Add verb"
                                            title="Add verb"
                                            style={{
                                                cursor: isSubmittingAddVerb ? "not-allowed" : "pointer",
                                                opacity: isSubmittingAddVerb ? 0.6 : 1,
                                                fontSize: `${secondaryFontSize}px`,
                                            }}
                                        >
                                            + Add
                                        </button>
                                    </div>
                                )}
                            </div>
                            <div className="p-sm border-bottom bg-secondary">
                                <input
                                    type="text"
                                    placeholder="Filter verbs..."
                                    value={verbFilter}
                                    onChange={(e) => setVerbFilter(e.target.value)}
                                    className="w-full p-xs border rounded-sm"
                                    style={{ fontSize: `${baseFontSize}px` }}
                                />
                            </div>
                            <div
                                className="browser-pane-content"
                                style={{ fontSize: `${baseFontSize}px` }}
                            >
                                {!selectedObject
                                    ? (
                                        <div className="p-md text-secondary">
                                            Select an object to view verbs
                                        </div>
                                    )
                                    : verbs.length === 0
                                    ? (
                                        <div className="p-md text-secondary">
                                            No verbs
                                        </div>
                                    )
                                    : (
                                        groupedVerbs.map(([location, verbList], _groupIdx) => (
                                            <div key={location}>
                                                {location !== selectedObject.obj && showInheritedVerbs && (
                                                    <div
                                                        className="browser-inherited-label"
                                                        style={{ fontSize: `${secondaryFontSize}px` }}
                                                    >
                                                        from {formatInheritedFrom(location)}
                                                    </div>
                                                )}
                                                {verbList.map((verb, idx) => (
                                                    <div
                                                        key={`${location}-${idx}`}
                                                        className={`browser-item ${
                                                            selectedVerb?.location === verb.location
                                                                && selectedVerb?.indexInLocation
                                                                    === verb.indexInLocation
                                                                ? "selected"
                                                                : ""
                                                        }`}
                                                        onClick={() => handleVerbSelect(verb)}
                                                        onKeyDown={(e) => {
                                                            if (e.key === "Enter" || e.key === " ") {
                                                                e.preventDefault();
                                                                handleVerbSelect(verb);
                                                            }
                                                        }}
                                                        tabIndex={0}
                                                        role="button"
                                                        aria-pressed={selectedVerb?.location === verb.location
                                                            && selectedVerb?.indexInLocation === verb.indexInLocation}
                                                    >
                                                        <div className="browser-item-name font-bold">
                                                            {verb.names.some(n => isTestVerb(n)) && (
                                                                <span
                                                                    title="Unit Test"
                                                                    style={{ marginRight: "4px" }}
                                                                >
                                                                    🧪
                                                                </span>
                                                            )}
                                                            {verb.names.join(" ")}{" "}
                                                            <span
                                                                className="text-secondary"
                                                                style={{
                                                                    opacity: selectedVerb?.location === verb.location
                                                                            && selectedVerb?.indexInLocation
                                                                                === verb.indexInLocation
                                                                        ? "0.7"
                                                                        : "1",
                                                                    color: selectedVerb?.location === verb.location
                                                                            && selectedVerb?.indexInLocation
                                                                                === verb.indexInLocation
                                                                        ? "inherit"
                                                                        : undefined,
                                                                    fontWeight: "400",
                                                                    fontSize: `${secondaryFontSize}px`,
                                                                }}
                                                            >
                                                                ({verb.readable ? "r" : ""}
                                                                {verb.writable ? "w" : ""}
                                                                {verb.executable ? "x" : ""}
                                                                {verb.debug ? "d" : ""})
                                                                {verbLabels.duplicateNames.has(
                                                                    `${location}:${verb.indexInLocation}`,
                                                                ) && " (duplicate name)"}
                                                                {verbLabels.overridden.has(
                                                                    `${location}:${verb.indexInLocation}`,
                                                                ) && " (overridden)"}
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
                            className={`browser-resize-handle ${isSplitDragging ? "dragging" : ""}`}
                            onMouseDown={handleSplitDragStart}
                            onTouchStart={handleSplitTouchStart}
                            style={{
                                position: "relative",
                                zIndex: 10,
                            }}
                        />
                    )}

                    {/* Bottom editor area */}
                    {(editorVisible || selectedObject) && (
                        <div className="flex-1 overflow-hidden bg-secondary">
                            {selectedObject && !selectedProperty && !selectedVerb && (
                                <ObjectInfoEditor
                                    object={selectedObject}
                                    objects={objects}
                                    authToken={authToken}
                                    onNavigate={handleNavigateToObject}
                                    normalizeObjectRef={normalizeObjectRef}
                                    normalizeObjectInput={normalizeObjectInput}
                                    getDollarName={getDollarName}
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
                                    onDumpObject={handleDumpObject}
                                    onReloadObject={() => {
                                        setReloadDialogError(null);
                                        setActionMessage(null);
                                        setShowReloadDialog(true);
                                    }}
                                    isSubmittingCreate={isSubmittingCreate}
                                    isSubmittingRecycle={isSubmittingRecycle}
                                    isSubmittingReload={isSubmittingReload}
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
                                    objectCurie={stringToCurie(selectedProperty.definer)}
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
                                        chown: selectedProperty.chown,
                                    }}
                                    onNavigateToObject={handleNavigateToObject}
                                    normalizeObjectInput={normalizeObjectInput}
                                    getDollarName={getDollarName}
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
                                    objectCurie={stringToCurie(selectedVerb.location)}
                                    verbName={selectedVerb.names[0]}
                                    verbNames={selectedVerb.names.join(" ")}
                                    initialContent={verbCode}
                                    authToken={authToken}
                                    splitMode={true}
                                    isInSplitMode={true}
                                    onToggleSplitMode={onOpenVerbInEditor ? handleDetachVerbEditor : undefined}
                                    owner={selectedVerb.owner}
                                    definer={selectedVerb.location}
                                    permissions={{
                                        readable: selectedVerb.readable,
                                        writable: selectedVerb.writable,
                                        executable: selectedVerb.executable,
                                        debug: selectedVerb.debug,
                                    }}
                                    argspec={{
                                        dobj: selectedVerb.dobj,
                                        prep: selectedVerb.prep,
                                        iobj: selectedVerb.iobj,
                                    }}
                                    onSave={() => {
                                        // Reload verbs list in background to update the list
                                        if (selectedObject) {
                                            loadPropertiesAndVerbs(selectedObject);
                                        }
                                    }}
                                    onDelete={() => {
                                        setVerbToDelete(selectedVerb);
                                        setDeleteVerbDialogError(null);
                                        setShowDeleteVerbDialog(true);
                                    }}
                                    onRun={selectedVerb.names.some(n => isTestVerb(n))
                                        ? () => handleRunTest(selectedVerb)
                                        : undefined}
                                    normalizeObjectInput={normalizeObjectInput}
                                    getDollarName={getDollarName}
                                />
                            )}
                        </div>
                    )}
                </div>
            </div>
            {showCreateDialog && (
                <CreateChildDialog
                    key={selectedObject?.obj || "new"}
                    defaultParent={selectedObject ? `#${selectedObject.obj}` : "#-1"}
                    defaultOwner={selectedObject
                        ? (normalizeObjectInput(selectedObject.owner ? `#${selectedObject.owner}` : "") || "player")
                        : "player"}
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
            {showReloadDialog && selectedObject && (
                <ReloadObjectDialog
                    key={`reload-${selectedObject.obj}`}
                    objectLabel={describeObject(selectedObject)}
                    objectId={selectedObject.obj}
                    onCancel={() => setShowReloadDialog(false)}
                    onSubmit={handleReloadObjectSubmit}
                    isSubmitting={isSubmittingReload}
                    errorMessage={reloadDialogError}
                />
            )}
            {showTestResultsDialog && (
                <TestResultsDialog
                    results={testResults}
                    onClose={() => setShowTestResultsDialog(false)}
                />
            )}
        </EditorWindow>
    );
};

interface TestResultsDialogProps {
    results: TestResult[];
    onClose: () => void;
}

const TestResultsDialog: React.FC<TestResultsDialogProps> = ({
    results,
    onClose,
}) => {
    return (
        <DialogSheet
            title="Test Results"
            titleId="test-results-title"
            onCancel={onClose}
            maxWidth="800px"
        >
            <div className="dialog-sheet-content">
                <div style={{ maxHeight: "60vh", overflowY: "auto" }}>
                    <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.9em" }}>
                        <thead
                            style={{
                                position: "sticky",
                                top: 0,
                                backgroundColor: "var(--color-bg-secondary)",
                                zIndex: 1,
                            }}
                        >
                            <tr>
                                <th
                                    style={{
                                        textAlign: "left",
                                        padding: "8px",
                                        borderBottom: "2px solid var(--color-border-medium)",
                                    }}
                                >
                                    Status
                                </th>
                                <th
                                    style={{
                                        textAlign: "left",
                                        padding: "8px",
                                        borderBottom: "2px solid var(--color-border-medium)",
                                    }}
                                >
                                    Verb
                                </th>
                                <th
                                    style={{
                                        textAlign: "left",
                                        padding: "8px",
                                        borderBottom: "2px solid var(--color-border-medium)",
                                    }}
                                >
                                    Location
                                </th>
                                <th
                                    style={{
                                        textAlign: "left",
                                        padding: "8px",
                                        borderBottom: "2px solid var(--color-border-medium)",
                                    }}
                                >
                                    Result/Error
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {results.map((result, idx) => (
                                <tr key={idx} style={{ borderBottom: "1px solid var(--color-border-light)" }}>
                                    <td style={{ padding: "8px" }}>
                                        {result.success ? "✅" : "❌"}
                                    </td>
                                    <td style={{ padding: "8px", fontFamily: "var(--font-mono)" }}>
                                        {result.verb}
                                    </td>
                                    <td style={{ padding: "8px", fontFamily: "var(--font-mono)" }}>
                                        #{result.location}
                                    </td>
                                    <td
                                        style={{
                                            padding: "8px",
                                            fontFamily: "var(--font-mono)",
                                            whiteSpace: "pre-wrap",
                                        }}
                                    >
                                        {result.success
                                            ? result.result
                                            : <span style={{ color: "var(--color-text-error)" }}>{result.error}</span>}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
                <div className="button-group">
                    <button type="button" onClick={onClose} className="btn btn-primary">
                        Close
                    </button>
                </div>
            </div>
        </DialogSheet>
    );
};

interface CreateChildDialogProps {
    defaultParent: string;
    defaultOwner: string;
    objectTypeOptions: Array<{ value: string; label: string }>;
    onCancel: () => void;
    onSubmit: (form: CreateChildFormValues) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const CreateChildDialog: React.FC<CreateChildDialogProps> = ({
    defaultParent,
    defaultOwner,
    objectTypeOptions,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [parent, setParent] = useState(defaultParent);
    const [owner, setOwner] = useState(defaultOwner);
    const [objectType, setObjectType] = useState<string>("server-default");
    const [initArgs, setInitArgs] = useState<string>("");
    const [name, setName] = useState<string>("");
    const [programmer, setProgrammer] = useState(false);
    const [wizard, setWizard] = useState(false);
    const [readable, setReadable] = useState(false);
    const [writable, setWritable] = useState(false);
    const [fertile, setFertile] = useState(false);

    useEffect(() => {
        setParent(defaultParent);
        setOwner(defaultOwner);
        setObjectType("server-default");
        setInitArgs("");
        setName("");
        setProgrammer(false);
        setWizard(false);
        setReadable(false);
        setWritable(false);
        setFertile(false);
    }, [defaultParent, defaultOwner]);

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        let flags = 0;
        if (programmer) flags |= 1 << 1;
        if (wizard) flags |= 1 << 2;
        if (readable) flags |= 1 << 4;
        if (writable) flags |= 1 << 5;
        if (fertile) flags |= 1 << 7;
        onSubmit({ parent, owner, objectType, initArgs, name, flags });
    };

    return (
        <DialogSheet title="Create Object" titleId="create-object-title" onCancel={onCancel}>
            <form onSubmit={handleSubmit} className="dialog-sheet-content form-stack">
                <label className="form-group">
                    <span className="form-group-label">Parent (MOO expression)</span>
                    <input
                        type="text"
                        value={parent}
                        onChange={(e) => setParent(e.target.value)}
                        placeholder="#-1"
                        autoFocus
                        className="form-input font-mono"
                    />
                </label>
                <label className="form-group">
                    <span className="form-group-label">Owner (MOO expression)</span>
                    <input
                        type="text"
                        value={owner}
                        onChange={(e) => setOwner(e.target.value)}
                        placeholder="player"
                        className="form-input font-mono"
                    />
                </label>
                <label className="form-group">
                    <span className="form-group-label">Object type</span>
                    <select
                        value={objectType}
                        onChange={(e) => setObjectType(e.target.value)}
                        className="form-input font-mono"
                    >
                        {objectTypeOptions.map((option) => (
                            <option key={option.value} value={option.value}>
                                {option.label}
                            </option>
                        ))}
                    </select>
                </label>
                <label className="form-group">
                    <span className="form-group-label">Initialization arguments</span>
                    <textarea
                        value={initArgs}
                        onChange={(e) => setInitArgs(e.target.value)}
                        placeholder="{}"
                        rows={3}
                        className="form-input font-mono"
                    />
                    <span className="form-group-hint">
                        Provide a MOO list literal (for example <code>{"{}"}</code> or{" "}
                        <code>{"{"}player{"}"}</code>). These arguments are passed to the object's{" "}
                        <code>:initialize</code> verb if it has one. Leave blank to skip initialization.
                    </span>
                </label>
                <label className="form-group">
                    <span className="form-group-label">Name (optional)</span>
                    <input
                        type="text"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder="Unnamed Object"
                        className="form-input font-mono"
                    />
                </label>
                <div className="form-group">
                    <span className="form-group-label">Flags</span>
                    <div className="permission-flags">
                        <label className="permission-flag-item">
                            <input
                                type="checkbox"
                                checked={programmer}
                                onChange={(e) => setProgrammer(e.target.checked)}
                            />
                            <span className="permission-flag-icon">p</span>
                            <span className="permission-flag-text">Programmer</span>
                        </label>
                        <label className="permission-flag-item">
                            <input
                                type="checkbox"
                                checked={wizard}
                                onChange={(e) => setWizard(e.target.checked)}
                            />
                            <span className="permission-flag-icon">w</span>
                            <span className="permission-flag-text">Wizard</span>
                        </label>
                        <label className="permission-flag-item">
                            <input
                                type="checkbox"
                                checked={readable}
                                onChange={(e) => setReadable(e.target.checked)}
                            />
                            <span className="permission-flag-icon">r</span>
                            <span className="permission-flag-text">Readable</span>
                        </label>
                        <label className="permission-flag-item">
                            <input
                                type="checkbox"
                                checked={writable}
                                onChange={(e) => setWritable(e.target.checked)}
                            />
                            <span className="permission-flag-icon">W</span>
                            <span className="permission-flag-text">Writable</span>
                        </label>
                        <label className="permission-flag-item">
                            <input
                                type="checkbox"
                                checked={fertile}
                                onChange={(e) => setFertile(e.target.checked)}
                            />
                            <span className="permission-flag-icon">f</span>
                            <span className="permission-flag-text">Fertile</span>
                        </label>
                    </div>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={isSubmitting}
                        className="btn btn-primary"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Creating…" : "Create"}
                    </button>
                </div>
            </form>
        </DialogSheet>
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
        <DialogSheet
            title="Recycle Object?"
            titleId="recycle-object-title"
            maxWidth="480px"
            role="alertdialog"
            onCancel={onCancel}
        >
            <div className="dialog-sheet-content form-stack">
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
                    <p className="m-0">
                        Recycling <strong>{objectLabel}</strong> is irreversible. Its contents will move to{" "}
                        <code>#-1</code>
                        and <code>:recycle</code> will be invoked if defined.
                    </p>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="button"
                        onClick={onConfirm}
                        disabled={isSubmitting}
                        className="btn btn-danger"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Recycling…" : "Recycle"}
                    </button>
                </div>
            </div>
        </DialogSheet>
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
        <DialogSheet title="Add Property" titleId="add-property-title" onCancel={onCancel}>
            <form onSubmit={handleSubmit} className="dialog-sheet-content form-stack">
                <p className="m-0 text-secondary">
                    Add a new property to <strong>{objectLabel}</strong>.
                </p>
                <label className="form-group">
                    <span className="form-group-label">Property name</span>
                    <input
                        type="text"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder="prop_name"
                        autoFocus
                        required
                        className="form-input font-mono"
                    />
                </label>
                <label className="form-group">
                    <span className="form-group-label">Initial value (MOO expression)</span>
                    <input
                        type="text"
                        value={value}
                        onChange={(e) => setValue(e.target.value)}
                        placeholder="0"
                        required
                        className="form-input font-mono"
                    />
                    <span className="form-group-hint">
                        Examples: <code>0</code>, <code>""</code>, <code>{"{}"}</code>, <code>player</code>
                    </span>
                </label>
                <label className="form-group">
                    <span className="form-group-label">Owner (MOO expression)</span>
                    <input
                        type="text"
                        value={owner}
                        onChange={(e) => setOwner(e.target.value)}
                        placeholder="player"
                        className="form-input font-mono"
                    />
                </label>
                <div className="form-group">
                    <span className="form-group-label">Permissions</span>
                    <span className="form-group-hint">
                        r=read, w=write, c=chown
                    </span>
                    <div className="permission-checkboxes">
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">r</span>
                        </label>
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">w</span>
                        </label>
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">c</span>
                        </label>
                    </div>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={isSubmitting}
                        className="btn btn-primary"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Adding…" : "Add Property"}
                    </button>
                </div>
            </form>
        </DialogSheet>
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
        <DialogSheet title="Add Verb" titleId="add-verb-title" onCancel={onCancel}>
            <form onSubmit={handleSubmit} className="dialog-sheet-content form-stack">
                <p className="m-0 text-secondary">
                    Add a new verb to <strong>{objectLabel}</strong>.
                </p>
                <div className="form-group">
                    <span className="form-group-label">Verb type</span>
                    <div className="verb-type-selector">
                        <label className="verb-type-option">
                            <input
                                type="radio"
                                name="verbType"
                                checked={verbType === "method"}
                                onChange={() => handleVerbTypeChange("method")}
                            />
                            <div className="verb-type-description">
                                <span className="verb-type-title">Method</span>
                                <span className="verb-type-subtitle">
                                    Called from code (<code>this none this</code>, with <code>x</code>)
                                </span>
                            </div>
                        </label>
                        <label className="verb-type-option">
                            <input
                                type="radio"
                                name="verbType"
                                checked={verbType === "command"}
                                onChange={() => handleVerbTypeChange("command")}
                            />
                            <div className="verb-type-description">
                                <span className="verb-type-title">Command</span>
                                <span className="verb-type-subtitle">
                                    Player command (e.g. <code>this none none</code>, no <code>x</code>)
                                </span>
                            </div>
                        </label>
                    </div>
                </div>
                <label className="form-group">
                    <span className="form-group-label">Verb names (space-separated)</span>
                    <input
                        type="text"
                        value={names}
                        onChange={(e) => setNames(e.target.value)}
                        placeholder="get take grab"
                        autoFocus
                        required
                        className="form-input font-mono"
                    />
                    <span className="form-group-hint">
                        Example: <code>get take grab</code> creates aliases for the same verb
                    </span>
                </label>
                <label className="form-group">
                    <span className="form-group-label">Owner (MOO expression)</span>
                    <input
                        type="text"
                        value={owner}
                        onChange={(e) => setOwner(e.target.value)}
                        placeholder="player"
                        className="form-input font-mono"
                    />
                </label>
                <div className="form-group">
                    <span className="form-group-label">Permissions</span>
                    <span className="form-group-hint">
                        r=read, w=write, x=exec, d=raise errors (usually keep on)
                    </span>
                    <div className="permission-checkboxes">
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">r</span>
                        </label>
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">w</span>
                        </label>
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">x</span>
                        </label>
                        <label className="permission-checkbox-item">
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
                            <span className="permission-checkbox-label">d</span>
                        </label>
                    </div>
                </div>
                <div className="form-group">
                    <span className="form-group-label">Verb argument specification</span>
                    <div className="verb-argspec-grid">
                        <label className="verb-argspec-column">
                            <span className="verb-argspec-label">dobj</span>
                            <select
                                value={dobj}
                                onChange={(e) => setDobj(e.target.value)}
                                className="verb-argspec-select"
                            >
                                <option value="none">none</option>
                                <option value="any">any</option>
                                <option value="this">this</option>
                            </select>
                        </label>
                        <label className="verb-argspec-column">
                            <span className="verb-argspec-label">prep</span>
                            <select
                                value={prep}
                                onChange={(e) => setPrep(e.target.value)}
                                className="verb-argspec-select"
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
                                <option value="named">named</option>
                            </select>
                        </label>
                        <label className="verb-argspec-column">
                            <span className="verb-argspec-label">iobj</span>
                            <select
                                value={iobj}
                                onChange={(e) => setIobj(e.target.value)}
                                className="verb-argspec-select"
                            >
                                <option value="none">none</option>
                                <option value="any">any</option>
                                <option value="this">this</option>
                            </select>
                        </label>
                    </div>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={isSubmitting}
                        className="btn btn-primary"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Adding…" : "Add Verb"}
                    </button>
                </div>
            </form>
        </DialogSheet>
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
        <DialogSheet
            title="Remove Verb?"
            titleId="delete-verb-title"
            maxWidth="480px"
            role="alertdialog"
            onCancel={onCancel}
        >
            <div className="dialog-sheet-content form-stack">
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
                    <p className="m-0">
                        Remove verb <code>{verbName}</code> from{" "}
                        <strong>{objectLabel}</strong>? This action cannot be undone.
                    </p>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="button"
                        onClick={onConfirm}
                        disabled={isSubmitting}
                        className="btn btn-danger"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Removing…" : "Remove Verb"}
                    </button>
                </div>
            </div>
        </DialogSheet>
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
        <DialogSheet
            title="Delete Property?"
            titleId="delete-property-title"
            maxWidth="480px"
            role="alertdialog"
            onCancel={onCancel}
        >
            <div className="dialog-sheet-content form-stack">
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
                    <p className="m-0">
                        Delete property <code>{propertyName}</code> from{" "}
                        <strong>{objectLabel}</strong>? This action cannot be undone.
                    </p>
                </div>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="button"
                        onClick={onConfirm}
                        disabled={isSubmitting}
                        className="btn btn-danger"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Deleting…" : "Delete Property"}
                    </button>
                </div>
            </div>
        </DialogSheet>
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
        <div className="flag-checkbox-item">
            <input
                type="checkbox"
                checked={checked}
                onChange={(e) => onChange(e.target.checked)}
                disabled={isSubmitting}
                className="flag-checkbox-input"
            />
            <div className="flag-checkbox-content">
                <div className="flag-checkbox-header">
                    <strong className="flag-checkbox-label">{label}</strong>
                    <code className="flag-char">{flagChar}</code>
                </div>
                <div className="flag-checkbox-description">{description}</div>
            </div>
        </div>
    );

    return (
        <DialogSheet title="Edit Object Flags" titleId="edit-flags-title" onCancel={onCancel}>
            <form onSubmit={handleSubmit} className="dialog-sheet-content form-stack">
                <p className="m-0 text-secondary">
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
                    <div className="dialog-error">
                        {errorMessage}
                    </div>
                )}

                <div className="button-group" style={{ marginTop: "1em" }}>
                    <button
                        type="button"
                        onClick={onCancel}
                        disabled={isSubmitting}
                        className="btn btn-secondary"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={isSubmitting}
                        className="btn btn-primary"
                        style={{
                            opacity: isSubmitting ? 0.6 : 1,
                            cursor: isSubmitting ? "not-allowed" : "pointer",
                            fontWeight: 700,
                        }}
                    >
                        {isSubmitting ? "Saving…" : "Save Flags"}
                    </button>
                </div>
            </form>
        </DialogSheet>
    );
};

interface ReloadObjectDialogProps {
    objectLabel: string;
    objectId: string;
    onCancel: () => void;
    onSubmit: (form: ReloadObjectFormValues) => void;
    isSubmitting: boolean;
    errorMessage: string | null;
}

const ReloadObjectDialog: React.FC<ReloadObjectDialogProps> = ({
    objectLabel,
    objectId,
    onCancel,
    onSubmit,
    isSubmitting,
    errorMessage,
}) => {
    const [objdefFile, setObjdefFile] = useState<File | null>(null);
    const [constantsFile, setConstantsFile] = useState<File | null>(null);
    const [showConstants, setShowConstants] = useState(false);
    const [confirmation, setConfirmation] = useState("");

    const expectedConfirmation = `#${objectId}`;
    const canSubmit = objdefFile !== null && confirmation.trim() === expectedConfirmation;

    const handleSubmit = (event: React.FormEvent) => {
        event.preventDefault();
        if (!objdefFile) return;
        onSubmit({ objdefFile, constantsFile, confirmation });
    };

    return (
        <DialogSheet
            title="Reload Object From Objdef"
            titleId="reload-object-title"
            maxWidth="520px"
            role="alertdialog"
            onCancel={onCancel}
        >
            <form onSubmit={handleSubmit} className="dialog-sheet-content form-stack">
                <div
                    style={{
                        padding: "0.75em",
                        borderRadius: "var(--radius-sm)",
                        border: "1px solid var(--color-text-warning)",
                        backgroundColor: "color-mix(in srgb, var(--color-text-warning) 12%, transparent)",
                        color: "var(--color-text-primary)",
                        fontFamily: "inherit",
                        display: "grid",
                        gap: "0.5em",
                    }}
                >
                    <strong>Reloading replaces the current object.</strong>
                    <ul className="m-0" style={{ paddingLeft: "1.1em" }}>
                        <li>Properties and verbs not in the objdef will be deleted.</li>
                        <li>Flags, name, owner, parent, and location will be overwritten.</li>
                        <li>There is no undo for this action.</li>
                    </ul>
                </div>
                <p className="m-0 text-secondary">
                    Reload <strong>{objectLabel}</strong> from an objdef file.
                </p>
                <label className="form-group">
                    <span className="form-group-label">Objdef file (.moo)</span>
                    <input
                        type="file"
                        accept=".moo,text/plain"
                        onChange={(e) => setObjdefFile(e.target.files?.[0] ?? null)}
                        required
                        className="form-input"
                    />
                </label>
                <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                        setShowConstants((prev) => {
                            if (prev) {
                                setConstantsFile(null);
                            }
                            return !prev;
                        });
                    }}
                    style={{ alignSelf: "flex-start" }}
                >
                    {showConstants ? "Hide constants file" : "Add constants file"}
                </button>
                {showConstants && (
                    <label className="form-group">
                        <span className="form-group-label">Constants file (constants.moo)</span>
                        <input
                            type="file"
                            accept=".moo,text/plain"
                            onChange={(e) => setConstantsFile(e.target.files?.[0] ?? null)}
                            className="form-input"
                        />
                    </label>
                )}
                <label className="form-group">
                    <span className="form-group-label">Type {expectedConfirmation} to confirm</span>
                    <input
                        type="text"
                        value={confirmation}
                        onChange={(e) => setConfirmation(e.target.value)}
                        placeholder={expectedConfirmation}
                        className="form-input font-mono"
                        required
                    />
                </label>
                {errorMessage && (
                    <div role="alert" className="dialog-error">
                        {errorMessage}
                    </div>
                )}
                <div className="button-group">
                    <button type="button" onClick={onCancel} className="btn btn-secondary">
                        Cancel
                    </button>
                    <button
                        type="submit"
                        disabled={!canSubmit || isSubmitting}
                        className="btn btn-danger"
                        style={{
                            opacity: !canSubmit || isSubmitting ? 0.6 : 1,
                            cursor: !canSubmit || isSubmitting ? "not-allowed" : "pointer",
                        }}
                    >
                        {isSubmitting ? "Reloading…" : "Reload"}
                    </button>
                </div>
            </form>
        </DialogSheet>
    );
};

interface ObjectInfoEditorProps {
    object: ObjectData;
    objects: ObjectData[];
    authToken: string;
    onNavigate: (objectId: string) => void;
    normalizeObjectRef: (raw: string) => { display: string; objectId: string | null };
    normalizeObjectInput: (raw: string) => string;
    getDollarName: (objId: string) => string | null;
    onCreateChild: () => void;
    onRecycle: () => void;
    onEditFlags: () => void;
    onDumpObject: () => void;
    onReloadObject: () => void;
    isSubmittingCreate: boolean;
    isSubmittingRecycle: boolean;
    isSubmittingReload: boolean;
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
    getDollarName,
    onCreateChild,
    onRecycle,
    onEditFlags,
    onDumpObject,
    onReloadObject,
    isSubmittingCreate,
    isSubmittingRecycle,
    isSubmittingReload,
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

        // Look up object name and $ name from the objects list
        const objData = objects.find(o => o.obj === objectId);
        const dollarName = objectId ? getDollarName(objectId) : null;

        let displayText = "";
        if (dollarName) {
            displayText = `$${dollarName} / `;
        }
        displayText += display;
        if (objData && objData.name) {
            displayText += ` ("${objData.name}")`;
        }

        if (!objectId) {
            return (
                <span className="font-mono text-secondary">
                    {displayText}
                </span>
            );
        }
        return (
            <button
                type="button"
                className="btn-link font-mono"
                onClick={() => onNavigate(objectId)}
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
        <div className="browser-section">
            <div
                className="browser-section-header"
                onClick={() => setExpanded(!isExpanded)}
            >
                <span style={{ fontSize: "9px" }}>{isExpanded ? "▼" : "▶"}</span>
                <span>{title} ({count})</span>
            </div>
            {isExpanded && <div className="browser-section-content">{content}</div>}
        </div>
    );

    const renderObjectRefSimple = (raw: string): React.ReactNode => {
        const { display, objectId } = normalizeObjectRef(raw);

        // Look up the object name and $ name
        const objData = objectId ? objects.find(o => o.obj === objectId) : null;
        const dollarName = objectId ? getDollarName(objectId) : null;

        let badgeText = "";
        if (dollarName) {
            badgeText = `$${dollarName} / `;
        }
        badgeText += display;

        const tooltip = objData?.name || null;

        if (!objectId) {
            return (
                <span className="object-ref-badge" title={tooltip || undefined}>
                    {badgeText}
                </span>
            );
        }
        return (
            <button
                type="button"
                className="object-ref-badge clickable"
                onClick={() => onNavigate(objectId)}
                title={tooltip || undefined}
            >
                {badgeText}
            </button>
        );
    };

    return (
        <div className="h-full flex-col bg-secondary">
            {/* Title bar */}
            <div className="editor-title-bar">
                <h3 className="editor-title" style={{ alignItems: "baseline" }}>
                    <span className="font-bold">Object info</span>
                    <span
                        className="text-secondary font-mono"
                        style={{
                            fontSize: "0.9em",
                            fontWeight: "normal",
                            textAlign: "center",
                            flex: 1,
                            marginLeft: "var(--space-sm)",
                            marginRight: "var(--space-sm)",
                        }}
                    >
                        {(() => {
                            const dollarName = getDollarName(object.obj);
                            let text = "";
                            if (dollarName) {
                                text = `$${dollarName} / `;
                            }
                            text += normalizeObjectRef(object.obj).display;
                            if (object.name) {
                                text += ` ("${object.name}")`;
                            }
                            return text;
                        })()}
                    </span>
                </h3>
                <div className="flex gap-sm" style={{ flexWrap: "nowrap" }}>
                    <button
                        type="button"
                        className="btn btn-sm btn-success"
                        onClick={onCreateChild}
                        disabled={!object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle}
                        style={{
                            cursor: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? "not-allowed"
                                : "pointer",
                            opacity: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? 0.6
                                : 1,
                            whiteSpace: "nowrap",
                        }}
                    >
                        Create Child
                    </button>
                    <button
                        type="button"
                        className="btn btn-sm btn-warning"
                        onClick={onRecycle}
                        disabled={!object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle}
                        style={{
                            cursor: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? "not-allowed"
                                : "pointer",
                            opacity: !object || object.obj === "-1" || isSubmittingCreate || isSubmittingRecycle
                                ? 0.6
                                : 1,
                            whiteSpace: "nowrap",
                        }}
                    >
                        Recycle
                    </button>
                    <button
                        type="button"
                        className="btn btn-sm"
                        onClick={onDumpObject}
                        disabled={!object || object.obj === "-1"}
                        style={{
                            cursor: !object || object.obj === "-1" ? "not-allowed" : "pointer",
                            opacity: !object || object.obj === "-1" ? 0.6 : 1,
                            whiteSpace: "nowrap",
                        }}
                        title="Export object definition to .moo file"
                    >
                        Export Objdef
                    </button>
                    <button
                        type="button"
                        className="btn btn-sm"
                        onClick={onReloadObject}
                        disabled={!object || object.obj === "-1" || isSubmittingReload}
                        style={{
                            cursor: !object || object.obj === "-1" || isSubmittingReload
                                ? "not-allowed"
                                : "pointer",
                            opacity: !object || object.obj === "-1" || isSubmittingReload ? 0.6 : 1,
                            whiteSpace: "nowrap",
                        }}
                        title="Reload object definition from .moo file"
                    >
                        Reload Objdef
                    </button>
                </div>
            </div>

            {/* Content area with metadata and hierarchy */}
            <div className="flex-1 overflow-auto">
                {/* Object metadata section */}
                <div
                    className="p-md bg-tertiary border-top border-bottom flex-wrap"
                    style={{ fontSize: "0.9em", display: "flex", gap: "var(--space-md)", alignItems: "center" }}
                >
                    {/* Name editor */}
                    <div className="flex gap-sm items-center" style={{ gap: "6px" }}>
                        <span className="text-secondary" style={{ fontFamily: "var(--font-ui)" }}>
                            Name:
                        </span>
                        <input
                            type="text"
                            value={editingName}
                            onChange={(e) => onNameChange(e.target.value)}
                            disabled={isSavingName}
                            className="font-mono border rounded-sm"
                            style={{
                                padding: "2px 6px",
                                fontSize: "0.95em",
                                minWidth: "120px",
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
                            className="btn btn-sm"
                            onClick={onNameSave}
                            disabled={isSavingName || editingName === object.name}
                            style={{
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
                    <div style={{ width: "1px", height: "20px", backgroundColor: "var(--color-border-medium)" }} />

                    {/* Flags */}
                    <div className="flex gap-sm items-center" style={{ gap: "6px" }}>
                        <span className="text-secondary" style={{ fontFamily: "var(--font-ui)" }}>
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
