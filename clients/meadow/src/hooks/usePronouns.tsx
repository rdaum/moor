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

// Hook for managing player pronouns with graceful fallback for unsupported cores

import { buildStructuredArgs } from "@moor/web-sdk";
import { useCallback, useEffect, useState } from "react";
import { CustomPronouns, PronounInput, readCustomPronouns } from "../lib/pronouns";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";

export const usePronouns = (authToken: string | null, playerOid: string | null) => {
    const [currentPronouns, setCurrentPronouns] = useState<string | null>(null);
    const [availablePresets, setAvailablePresets] = useState<string[]>([]);
    const [pronounsAvailable, setPronounsAvailable] = useState<boolean | null>(null);
    const [customPronouns, setCustomPronouns] = useState<CustomPronouns | null>(null);
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

    const fetchCustomPronouns = useCallback(async () => {
        if (!authToken || !playerOid) return;
        try {
            const { result } = await invokeVerbFlatBuffer(authToken, playerOid, "pronouns_settings");
            setCustomPronouns(readCustomPronouns(result));
        } catch {
            // Other cores can offer presets without a custom pronoun editor.
            setCustomPronouns(null);
        }
    }, [authToken, playerOid]);

    const previewPronouns = useCallback(async (input: CustomPronouns): Promise<string[]> => {
        if (!authToken) throw new Error("Not authenticated");
        const { result } = await invokeVerbFlatBuffer(
            authToken,
            "sysobj:pronouns",
            "examples",
            buildStructuredArgs([{ ...input }]),
        );
        if (!Array.isArray(result) || !result.every(value => typeof value === "string")) {
            throw new Error("The server did not return a pronoun preview.");
        }
        return result;
    }, [authToken]);

    // Fetch presets and current pronouns on mount and when auth changes
    useEffect(() => {
        fetchPresets();
        fetchPronouns();
        fetchCustomPronouns();
    }, [fetchPresets, fetchPronouns, fetchCustomPronouns]);

    const updatePronouns = useCallback(
        async (pronouns: PronounInput) => {
            if (!authToken || !playerOid) {
                throw new Error("Not authenticated");
            }

            setLoading(true);
            setError(null);

            try {
                const bytes = buildStructuredArgs([typeof pronouns === "string" ? pronouns : { ...pronouns }]);

                await invokeVerbFlatBuffer(authToken, playerOid, "set_pronouns", bytes);

                // Refresh after update
                await Promise.all([fetchPronouns(), fetchCustomPronouns()]);
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
        [authToken, playerOid, fetchPronouns, fetchCustomPronouns],
    );

    return {
        currentPronouns,
        customPronouns,
        previewPronouns,
        availablePresets,
        pronounsAvailable,
        loading,
        error,
        updatePronouns,
        refreshPronouns: fetchPronouns,
    };
};
