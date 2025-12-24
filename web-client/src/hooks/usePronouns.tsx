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

// Standard pronoun presets (same as ProfileSetupPanel)
const PRONOUN_PRESETS = ["they/them", "she/her", "he/him", "xe/xem", "it/its", "any", "ask"];

export const usePronouns = (authToken: string | null, playerOid: string | null) => {
    const [currentPronouns, setCurrentPronouns] = useState<string | null>(null);
    const [pronounsAvailable, setPronounsAvailable] = useState<boolean | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchPronouns = useCallback(async () => {
        if (!authToken || !playerOid) {
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Try to get current pronouns from the player
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                playerOid,
                "pronouns",
            );

            // Result should be a flyweight with a .display property
            if (result && typeof result === "object" && "display" in result) {
                setCurrentPronouns(result.display as string);
                setPronounsAvailable(true);
            } else if (typeof result === "string") {
                setCurrentPronouns(result);
                setPronounsAvailable(true);
            } else {
                // Pronouns verb exists but returned unexpected format
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

    // Fetch on mount and when auth changes
    useEffect(() => {
        fetchPronouns();
    }, [fetchPronouns]);

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
        availablePresets: PRONOUN_PRESETS,
        pronounsAvailable,
        loading,
        error,
        updatePronouns,
        refreshPronouns: fetchPronouns,
    };
};
