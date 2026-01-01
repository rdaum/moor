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

// Hook for managing player description

import * as flatbuffers from "flatbuffers";
import { useCallback, useEffect, useState } from "react";
import { Var } from "../generated/moor-var/var";
import { VarList } from "../generated/moor-var/var-list";
import { VarStr } from "../generated/moor-var/var-str";
import { VarUnion } from "../generated/moor-var/var-union";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";

export const usePlayerDescription = (authToken: string | null, playerOid: string | null) => {
    const [playerDescription, setPlayerDescription] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchPlayerDescription = useCallback(async () => {
        if (!authToken || !playerOid) {
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Invoke the description verb on the player object
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                playerOid,
                "description",
            );

            if (!result || typeof result !== "string") {
                setPlayerDescription(null);
                return;
            }

            setPlayerDescription(result);
        } catch (err) {
            console.debug("Player description fetch failed (verb may not exist):", err);
            setPlayerDescription(null);
            setError(null); // Don't show error to user
        } finally {
            setLoading(false);
        }
    }, [authToken, playerOid]);

    // Fetch on mount and when auth changes
    useEffect(() => {
        fetchPlayerDescription();
    }, [fetchPlayerDescription]);

    const updatePlayerDescription = useCallback(
        async (description: string) => {
            if (!authToken || !playerOid) {
                throw new Error("Not authenticated");
            }

            setLoading(true);
            setError(null);

            try {
                // Build FlatBuffer argument: list containing the description string
                const builder = new flatbuffers.Builder(1024);

                // Build the description string Var
                const descriptionStrOffset = builder.createString(description);
                const varStrOffset = VarStr.createVarStr(builder, descriptionStrOffset);
                const descriptionVarOffset = Var.createVar(builder, VarUnion.VarStr, varStrOffset);

                // Wrap in a list (verb args must be a list)
                const elementsVectorOffset = VarList.createElementsVector(builder, [
                    descriptionVarOffset,
                ]);
                const varListOffset = VarList.createVarList(builder, elementsVectorOffset);
                const listVarOffset = Var.createVar(builder, VarUnion.VarList, varListOffset);

                builder.finish(listVarOffset);

                // Get the bytes
                const bytes = builder.asUint8Array();

                // Invoke set_description verb with the constructed argument
                await invokeVerbFlatBuffer(authToken, playerOid, "set_description", bytes);

                // After update succeeds, refresh the description
                await fetchPlayerDescription();
            } catch (err) {
                console.error("Update error:", err);
                const errorMessage = err instanceof Error
                    ? err.message
                    : "Failed to update player description";
                setError(errorMessage);
                throw err;
            } finally {
                setLoading(false);
            }
        },
        [authToken, playerOid, fetchPlayerDescription],
    );

    return {
        playerDescription,
        loading,
        error,
        updatePlayerDescription,
        refreshPlayerDescription: fetchPlayerDescription,
    };
};
