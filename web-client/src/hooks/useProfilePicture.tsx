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

// Hook for managing player profile pictures

import * as flatbuffers from "flatbuffers";
import { useCallback, useEffect, useState } from "react";
import { Var } from "../generated/moor-var/var";
import { VarBinary } from "../generated/moor-var/var-binary";
import { VarList } from "../generated/moor-var/var-list";
import { VarStr } from "../generated/moor-var/var-str";
import { VarUnion } from "../generated/moor-var/var-union";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";

interface ProfilePictureData {
    contentType: string;
    data: Uint8Array;
}

export const useProfilePicture = (authToken: string | null, playerOid: string | null) => {
    const [profilePicture, setProfilePicture] = useState<ProfilePictureData | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchProfilePicture = useCallback(async () => {
        if (!authToken || !playerOid) {
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Invoke the profile_picture verb on the player object
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                playerOid,
                "profile_picture",
            );

            // Result is already JS: [content_type, binary_data]
            if (!Array.isArray(result) || result.length < 2) {
                setProfilePicture(null);
                return;
            }

            const contentType = result[0];
            const binaryData = result[1];

            if (typeof contentType !== "string" || !(binaryData instanceof Uint8Array)) {
                setProfilePicture(null);
                return;
            }

            setProfilePicture({
                contentType,
                data: binaryData,
            });
        } catch (err) {
            // If the verb doesn't exist, we'll get an error - that's OK
            console.debug("Profile picture fetch failed (verb may not exist):", err);
            setProfilePicture(null);
            setError(null); // Don't show error to user
        } finally {
            setLoading(false);
        }
    }, [authToken, playerOid]);

    // Fetch on mount and when auth changes
    useEffect(() => {
        fetchProfilePicture();
    }, [fetchProfilePicture]);

    const uploadProfilePicture = useCallback(async (file: File) => {
        if (!authToken || !playerOid) {
            throw new Error("Not authenticated");
        }

        setLoading(true);
        setError(null);

        try {
            // Read file as ArrayBuffer
            const arrayBuffer = await file.arrayBuffer();
            const data = new Uint8Array(arrayBuffer);

            // Build FlatBuffer arguments: [content-type, binary]
            const builder = new flatbuffers.Builder(1024);

            // Build the content-type string Var
            const contentTypeStrOffset = builder.createString(file.type);
            const varStrOffset = VarStr.createVarStr(builder, contentTypeStrOffset);
            const contentTypeVarOffset = Var.createVar(builder, VarUnion.VarStr, varStrOffset);

            // Build the binary data Var
            const binaryDataOffset = VarBinary.createDataVector(builder, data);
            const varBinaryOffset = VarBinary.createVarBinary(builder, binaryDataOffset);
            const binaryVarOffset = Var.createVar(builder, VarUnion.VarBinary, varBinaryOffset);

            // Build the list containing both arguments
            const elementsVectorOffset = VarList.createElementsVector(builder, [contentTypeVarOffset, binaryVarOffset]);
            const varListOffset = VarList.createVarList(builder, elementsVectorOffset);
            const listVarOffset = Var.createVar(builder, VarUnion.VarList, varListOffset);

            builder.finish(listVarOffset);

            // Get the bytes
            const bytes = builder.asUint8Array();

            // Invoke set_profile_picture verb with the constructed argument
            await invokeVerbFlatBuffer(authToken, playerOid, "set_profile_picture", bytes);

            // After upload succeeds, refresh the profile picture
            await fetchProfilePicture();
        } catch (err) {
            console.error("Upload error:", err);
            const errorMessage = err instanceof Error ? err.message : "Failed to upload profile picture";
            setError(errorMessage);
            throw err;
        } finally {
            setLoading(false);
        }
    }, [authToken, playerOid, fetchProfilePicture]);

    return {
        profilePicture,
        loading,
        error,
        uploadProfilePicture,
        refreshProfilePicture: fetchProfilePicture,
    };
};
