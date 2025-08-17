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

import { useCallback, useEffect, useState } from "react";
import {
    actionSuggestionsToStrings,
    combineSuggestions,
    DEFAULT_VERBS,
    filterActionSuggestionsByInput,
    filterSuggestionsByInput,
} from "../components/SuggestionStrip";
import { CommandSuggestionsResponse, getCommandSuggestions, SuggestionMode } from "../lib/rpc";

interface UseSuggestionsOptions {
    authToken: string | null;
    debounceMs?: number;
    maxSuggestions?: number;
    mode?: SuggestionMode;
}

interface UseSuggestionsReturn {
    suggestions: string[];
    isLoading: boolean;
    error: string | null;
}

/**
 * Custom hook for managing command suggestions with debouncing
 */
export function useSuggestions(
    input: string,
    options: UseSuggestionsOptions,
): UseSuggestionsReturn {
    const {
        authToken,
        debounceMs = 300,
        maxSuggestions = 20, // Increased from 6 to support dynamic sizing
        mode = "environment_actions",
    } = options;

    const [suggestions, setSuggestions] = useState<string[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Debounced effect for fetching suggestions
    useEffect(() => {
        // Don't set initial defaults - wait for API results

        // If no auth token, don't show anything
        if (!authToken) {
            setSuggestions([]);
            return;
        }

        // Always fetch API suggestions to get environment actions
        // We'll filter them based on input later

        // Use shorter delay for empty input to show environment actions quickly
        const delay = input.trim().length === 0 ? 100 : debounceMs;

        // Set up debounced API call
        const timeoutId = setTimeout(async () => {
            try {
                setIsLoading(true);
                setError(null);

                const response = await getCommandSuggestions(authToken, {
                    mode,
                    // No maxSuggestions limit - get all available suggestions
                });

                // Filter API suggestions to hide @ commands unless user types @
                const filteredApiSuggestions = filterActionSuggestionsByInput(response.action_suggestions, input);

                // Convert filtered API suggestions to strings
                const apiSuggestions = actionSuggestionsToStrings(filteredApiSuggestions);

                // Combine with filtered defaults, passing input for proper filtering
                // No maxSuggestions limit since we now support scrolling through all suggestions
                const combined = combineSuggestions(
                    DEFAULT_VERBS, // Use unfiltered defaults, combineSuggestions will filter
                    apiSuggestions,
                    input,
                );

                setSuggestions(combined);
            } catch (err) {
                console.error("Failed to fetch suggestions:", err);
                setError(err instanceof Error ? err.message : "Unknown error");
                // On error, just clear suggestions
                setSuggestions([]);
            } finally {
                setIsLoading(false);
            }
        }, delay);

        // Cleanup timeout on input change or unmount
        return () => {
            clearTimeout(timeoutId);
        };
    }, [input, authToken, debounceMs, maxSuggestions, mode]);

    return {
        suggestions,
        isLoading,
        error,
    };
}

/**
 * Hook for parsing input and determining appropriate suggestion mode
 */
export function useSuggestionMode(input: string): {
    mode: SuggestionMode;
    context: string;
} {
    return useCallback(() => {
        const trimmed = input.trim();

        // Empty input = environment actions
        if (!trimmed) {
            return { mode: "environment_actions" as SuggestionMode, context: "environment" };
        }

        // Simple heuristic for now - could be made more sophisticated
        const words = trimmed.split(/\s+/);

        if (words.length === 1) {
            // Single word - verb completion
            return { mode: "environment_actions" as SuggestionMode, context: "verb" };
        } else if (words.length === 2) {
            // Two words - could be verb + object
            return { mode: "verb_targets" as SuggestionMode, context: "direct_object" };
        } else {
            // Multiple words - environment actions for now
            return { mode: "environment_actions" as SuggestionMode, context: "complex" };
        }
    }, [input])();
}
