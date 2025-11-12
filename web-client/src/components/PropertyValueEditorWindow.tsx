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

import React, { useCallback } from "react";
import { PropertyValueEditorSession } from "../hooks/usePropertyValueEditor";
import { useTouchDevice } from "../hooks/useTouchDevice";
import { EditorWindow } from "./EditorWindow";
import { PropertyValueEditor } from "./PropertyValueEditor";

interface PropertyValueEditorWindowProps {
    visible: boolean;
    authToken: string;
    session: PropertyValueEditorSession;
    onClose: () => void;
    onRefresh: () => Promise<void>;
    splitMode?: boolean;
    onToggleSplitMode?: () => void;
    isInSplitMode?: boolean;
}

const normalizeObjectInput = (raw: string): string => {
    if (!raw) return "";
    const trimmed = raw.trim();
    if (!trimmed) return "";
    if (
        trimmed.startsWith("#")
        || trimmed.startsWith("$")
        || trimmed.startsWith("player")
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

const DEFAULT_SIZE = { width: 700, height: 550 };

export const PropertyValueEditorWindow: React.FC<PropertyValueEditorWindowProps> = ({
    visible,
    authToken,
    session,
    onClose,
    onRefresh,
    splitMode = false,
    onToggleSplitMode,
    isInSplitMode = false,
}) => {
    const isTouchDevice = useTouchDevice();

    const handleRefresh = useCallback(() => {
        void onRefresh();
    }, [onRefresh]);

    return (
        <EditorWindow
            visible={visible}
            onClose={onClose}
            splitMode={splitMode}
            defaultPosition={{ x: 80, y: 80 }}
            defaultSize={DEFAULT_SIZE}
            minSize={{ width: 400, height: 320 }}
            ariaLabel={`Property value editor for ${session.propertyName}`}
            className="property_editor_container"
        >
            <PropertyValueEditor
                authToken={authToken}
                objectCurie={session.objectCurie}
                propertyName={session.propertyName}
                propertyValue={session.propertyValue}
                onSave={handleRefresh}
                onCancel={onClose}
                owner={session.owner}
                definer={session.definer}
                permissions={session.permissions}
                normalizeObjectInput={normalizeObjectInput}
                getDollarName={() => null}
                splitMode={splitMode}
                onToggleSplitMode={onToggleSplitMode}
                isInSplitMode={isInSplitMode}
                isTouchDevice={isTouchDevice}
            />
        </EditorWindow>
    );
};
