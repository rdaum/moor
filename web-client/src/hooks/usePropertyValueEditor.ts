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

import { useCallback, useState } from "react";
import { MoorVar } from "../lib/MoorVar";
import { getPropertyFlatBuffer } from "../lib/rpc-fb";
import { objToString } from "../lib/var";

export interface PropertyValueEditorSession {
    id: string;
    title: string;
    objectCurie: string;
    propertyName: string;
    propertyValue: MoorVar;
    owner?: string;
    definer?: string;
    permissions?: { readable: boolean; writable: boolean; chown: boolean };
    presentationId?: string;
}

export const usePropertyValueEditor = () => {
    const [propertyValueEditorSession, setPropertyValueEditorSession] = useState<PropertyValueEditorSession | null>(
        null,
    );

    const fetchPropertyValue = useCallback(async (
        objectCurie: string,
        propertyName: string,
        authToken: string,
    ) => {
        const propertyValue = await getPropertyFlatBuffer(authToken, objectCurie, propertyName);
        const varValue = propertyValue.value();
        if (!varValue) {
            throw new Error("Property returned no value");
        }

        const moorVar = new MoorVar(varValue);
        const propInfo = propertyValue.propInfo();

        return {
            propertyValue: moorVar,
            owner: propInfo ? objToString(propInfo.owner()) || undefined : undefined,
            definer: propInfo ? objToString(propInfo.definer()) || undefined : undefined,
            permissions: propInfo
                ? {
                    readable: propInfo.r(),
                    writable: propInfo.w(),
                    chown: propInfo.chown(),
                }
                : undefined,
        };
    }, []);

    const launchPropertyValueEditor = useCallback(async (
        title: string,
        objectCurie: string,
        propertyName: string,
        authToken: string,
        presentationId?: string,
    ) => {
        const payload = await fetchPropertyValue(objectCurie, propertyName, authToken);
        setPropertyValueEditorSession({
            id: `${objectCurie}.${propertyName}`,
            title,
            objectCurie,
            propertyName,
            presentationId,
            ...payload,
        });
    }, [fetchPropertyValue]);

    const refreshPropertyValueEditor = useCallback(async (authToken: string) => {
        if (!propertyValueEditorSession) {
            return;
        }
        const payload = await fetchPropertyValue(
            propertyValueEditorSession.objectCurie,
            propertyValueEditorSession.propertyName,
            authToken,
        );
        setPropertyValueEditorSession(prev =>
            prev
                ? {
                    ...prev,
                    ...payload,
                }
                : prev
        );
    }, [fetchPropertyValue, propertyValueEditorSession]);

    const closePropertyValueEditor = useCallback(() => {
        setPropertyValueEditorSession(null);
    }, []);

    return {
        propertyValueEditorSession,
        launchPropertyValueEditor,
        refreshPropertyValueEditor,
        closePropertyValueEditor,
    };
};
