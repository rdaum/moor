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

// Hook for managing player pronouns with graceful fallback for unsupported cores

import * as flatbuffers from "flatbuffers";
import { useCallback, useEffect, useState } from "react";
import { Var } from "../generated/moor-var/var";
import { VarList } from "../generated/moor-var/var-list";
import { VarStr } from "../generated/moor-var/var-str";
import { VarUnion } from "../generated/moor-var/var-union";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";

export const usePronouns = (authToken: string | null, playerOid: string | null) => {
    const [currentPronouns, setCurrentPronouns] = useState<string | null>(null);
    const [availablePresets, setAvailablePresets] = useState<string[]>([]);
    const [pronounsAvailable, setPronounsAvailable] = useState<boolean | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchPresets = useCallback(async () => {
        if (!authToken) {
            return;
        }

        try {
            // Fetch available presets from $pronouns:list_presets()
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                "sysobj:pronouns",
                "list_presets",
            );

            if (Array.isArray(result)) {
                setAvailablePresets(result.filter((p): p is string => typeof p === "string"));
            }
        } catch (err) {
            console.debug("Failed to fetch pronoun presets:", err);
            // Keep empty array on failure
        }
    }, [authToken]);

    const fetchPronouns = useCallback(async () => {
        if (!authToken || !playerOid) {
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Try to get current pronouns display string from the player
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                playerOid,
                "pronouns_display",
            );

            // Result should be a simple string like "they/them"
            if (typeof result === "string") {
                setCurrentPronouns(result);
                setPronounsAvailable(true);
            } else {
                // Verb exists but returned unexpected format
                setCurrentPronouns(null);
                setPronounsAvailable(true);
            }
        } catch (err) {
            // Pronouns not supported on this core
            console.debug("Pronouns not available:", err);
            setPronounsAvailable(false);
            setCurrentPronouns(null);
        } finally {
            setLoading(false);
        }
    }, [authToken, playerOid]);

    // Fetch presets and current pronouns on mount and when auth changes
    useEffect(() => {
        fetchPresets();
        fetchPronouns();
    }, [fetchPresets, fetchPronouns]);

    const updatePronouns = useCallback(
        async (pronouns: string) => {
            if (!authToken || !playerOid) {
                throw new Error("Not authenticated");
            }

            setLoading(true);
            setError(null);

            try {
                // Build FlatBuffer argument
                const builder = new flatbuffers.Builder(256);
                const pronounsStrOffset = builder.createString(pronouns);
                const varStrOffset = VarStr.createVarStr(builder, pronounsStrOffset);
                const pronounsVarOffset = Var.createVar(builder, VarUnion.VarStr, varStrOffset);
                const elementsVectorOffset = VarList.createElementsVector(builder, [pronounsVarOffset]);
                const varListOffset = VarList.createVarList(builder, elementsVectorOffset);
                const listVarOffset = Var.createVar(builder, VarUnion.VarList, varListOffset);
                builder.finish(listVarOffset);
                const bytes = builder.asUint8Array();

                await invokeVerbFlatBuffer(authToken, playerOid, "set_pronouns", bytes);

                // Refresh after update
                await fetchPronouns();
            } catch (err) {
                console.error("Update pronouns error:", err);
                const errorMessage = err instanceof Error
                    ? err.message
                    : "Failed to update pronouns";
                setError(errorMessage);
                throw err;
            } finally {
                setLoading(false);
            }
        },
        [authToken, playerOid, fetchPronouns],
    );

    return {
        currentPronouns,
        availablePresets,
        pronounsAvailable,
        loading,
        error,
        updatePronouns,
        refreshPronouns: fetchPronouns,
    };
};
