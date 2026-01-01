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

// Hook for fetching verb suggestions from the player object

import { useCallback, useEffect, useState } from "react";
import { invokeVerbFlatBuffer } from "../lib/rpc-fb";

export interface VerbSuggestion {
    verb: string;
    dobj: string;
    prep: string;
    iobj: string;
    objects: string[];
    hint: string | null;
}

export interface UseVerbSuggestionsResult {
    suggestions: VerbSuggestion[];
    loading: boolean;
    error: string | null;
    refresh: () => Promise<void>;
    available: boolean;
}

export const useVerbSuggestions = (
    authToken: string | null,
    playerOid: string | null,
): UseVerbSuggestionsResult => {
    const [suggestions, setSuggestions] = useState<VerbSuggestion[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [available, setAvailable] = useState(false);

    const fetchSuggestions = useCallback(async () => {
        if (!authToken || !playerOid) {
            return;
        }

        setLoading(true);
        setError(null);

        try {
            const { result } = await invokeVerbFlatBuffer(
                authToken,
                playerOid,
                "verb_suggestions",
            );

            if (!result) {
                setSuggestions([]);
                setAvailable(false);
                return;
            }

            if (!Array.isArray(result)) {
                setSuggestions([]);
                setAvailable(false);
                return;
            }

            // Parse the list of maps into VerbSuggestion objects
            const parsed: VerbSuggestion[] = result.map((item: Record<string, unknown>) => ({
                verb: String(item.verb || ""),
                dobj: String(item.dobj || "none"),
                prep: String(item.prep || "none"),
                iobj: String(item.iobj || "none"),
                objects: Array.isArray(item.objects)
                    ? item.objects.map((o: unknown) => String(o))
                    : [],
                hint: typeof item.hint === "string" ? item.hint : null,
            })).filter((s: VerbSuggestion) => s.verb !== "");

            setSuggestions(parsed);
            setAvailable(true);
        } catch (err) {
            // Verb may not exist on this core - fall back silently
            console.debug("verb_suggestions fetch failed (verb may not exist):", err);
            setSuggestions([]);
            setAvailable(false);
            setError(null);
        } finally {
            setLoading(false);
        }
    }, [authToken, playerOid]);

    useEffect(() => {
        fetchSuggestions();
    }, [fetchSuggestions]);

    return {
        suggestions,
        loading,
        error,
        refresh: fetchSuggestions,
        available,
    };
};
