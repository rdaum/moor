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

import React, { useEffect, useRef, useState } from "react";
import { ActionSuggestion } from "../lib/rpc";

interface SuggestionStripProps {
    suggestions: string[];
    onSuggestionClick: (suggestion: string) => void;
    visible: boolean;
    selectedIndex?: number;
}

// Default common verbs for client-side ranking priority (not for initial display)
export const DEFAULT_VERBS = ["say", "emote", "look", "help", "get", "drop"];

export const SuggestionStrip: React.FC<SuggestionStripProps> = ({
    suggestions,
    onSuggestionClick,
    visible,
    selectedIndex = -1,
}) => {
    const stripRef = useRef<HTMLDivElement>(null);
    const selectedButtonRef = useRef<HTMLButtonElement>(null);

    // Scroll the selected suggestion into view
    useEffect(() => {
        if (selectedIndex >= 0 && selectedButtonRef.current && stripRef.current) {
            const button = selectedButtonRef.current;
            const container = stripRef.current;

            // Get button position relative to container
            const buttonRect = button.getBoundingClientRect();
            const containerRect = container.getBoundingClientRect();

            // Calculate if button is outside visible area
            const buttonLeft = buttonRect.left - containerRect.left;
            const buttonRight = buttonRect.right - containerRect.left;
            const containerWidth = container.clientWidth;

            // Scroll behavior to keep selected item visible
            if (buttonLeft < 0) {
                // Button is to the left, scroll left
                container.scrollBy({
                    left: buttonLeft - 20, // Add some padding
                    behavior: "smooth",
                });
            } else if (buttonRight > containerWidth) {
                // Button is to the right, scroll right
                container.scrollBy({
                    left: buttonRight - containerWidth + 20, // Add some padding
                    behavior: "smooth",
                });
            }
        }
    }, [selectedIndex]);

    if (!visible || suggestions.length === 0) {
        return null;
    }

    return (
        <div ref={stripRef} className="suggestion-strip">
            {suggestions.map((suggestion, index) => (
                <button
                    key={`${suggestion}-${index}`}
                    ref={index === selectedIndex ? selectedButtonRef : null}
                    className={`suggestion-button ${index === selectedIndex ? "selected" : ""}`}
                    onClick={() => onSuggestionClick(suggestion)}
                    type="button"
                    aria-pressed={index === selectedIndex}
                >
                    {suggestion}
                </button>
            ))}
        </div>
    );
};

/**
 * Converts action suggestions from API to display strings
 * Chooses the best alias from each verb group for display
 */
export function actionSuggestionsToStrings(actions: ActionSuggestion[]): string[] {
    return actions.map(action => {
        // Choose the best alias for display
        return chooseBestAlias(action.verb_aliases);
    });
}

/**
 * Chooses the best alias to display from a list of verb aliases
 * Prefers longer, more descriptive names over single letters
 */
function chooseBestAlias(aliases: string[]): string {
    if (aliases.length === 0) return "";
    if (aliases.length === 1) return aliases[0];

    // Rank ALL aliases by preference (higher score = better)
    const rankedAliases = aliases.map(alias => ({
        alias,
        score: getAliasDisplayScore(alias),
    })).sort((a, b) => b.score - a.score);

    return rankedAliases[0].alias;
}

/**
 * Scores an alias for display preference (higher = better)
 */
function getAliasDisplayScore(alias: string): number {
    let score = 0;

    // Heavy penalty for underscores (internal/technical names)
    if (alias.includes("_")) {
        score -= 50;
    }

    // Bonus for containing wildcards (like "inv*entory") - these are user-facing patterns
    if (alias.includes("*")) {
        score += 20;
    }

    // Moderate preference for reasonable lengths (3-8 chars is sweet spot)
    if (alias.length >= 3 && alias.length <= 8) {
        score += 10;
    } else if (alias.length > 8) {
        // Long names are often technical - small penalty
        score -= 2;
    }

    // Small penalty for single letters (but not as harsh as underscores)
    if (alias.length === 1) {
        score -= 5;
    }

    // Very small penalty for @ commands (they might still be the best option)
    if (alias.startsWith("@")) {
        score -= 2;
    }

    return score;
}

/**
 * Filters suggestions based on current input text
 */
export function filterSuggestionsByInput(suggestions: string[], input: string): string[] {
    const query = input.trim().toLowerCase();

    // Handle @ commands specially - only show them if user typed @
    if (query.startsWith("@")) {
        const atQuery = query.substring(1); // Remove the @
        return suggestions.filter(suggestion => {
            if (suggestion.startsWith("@")) {
                const suggestionWithoutAt = suggestion.substring(1).toLowerCase();
                return suggestionWithoutAt.startsWith(atQuery);
            }
            return false; // Only show @ commands when user types @
        });
    } else {
        // For non-@ commands, filter out @ commands and use prefix matching
        return suggestions.filter(suggestion => {
            if (suggestion.startsWith("@")) {
                return false; // Don't show @ commands unless user types @
            }
            // If no input, show all non-@ suggestions; otherwise use prefix matching
            return !query || suggestion.toLowerCase().startsWith(query);
        });
    }
}

/**
 * Combines and deduplicates suggestions from multiple sources
 */
export function combineSuggestions(
    defaultSuggestions: string[],
    apiSuggestions: string[],
    input: string,
    maxSuggestions?: number,
): string[] {
    // Use a Set to remove duplicates while preserving order
    const combined = new Set<string>();

    // Add default suggestions first (higher priority), but filter them by input
    const filteredDefaultSuggestions = filterSuggestionsByInput(defaultSuggestions, input);
    filteredDefaultSuggestions.forEach(suggestion => combined.add(suggestion));

    // Add API suggestions that aren't already included, also filtered by input
    const filteredApiSuggestions = filterSuggestionsByInput(apiSuggestions, input);
    filteredApiSuggestions.forEach(suggestion => combined.add(suggestion));

    // Convert back to array and optionally limit
    const result = Array.from(combined);
    return maxSuggestions ? result.slice(0, maxSuggestions) : result;
}

/**
 * MOO-style verb matching that mirrors LambdaMOO's verbcasecmp function
 *
 * Wildcard behavior:
 * - `*` at the end: matches any string that begins with the prefix (e.g., "foo*" matches "foo", "foobar")
 * - `*` in the middle: matches any prefix of the full pattern that's at least as long as the part before the star
 *   (e.g., "foo*bar" matches "foo", "foob", "fooba", "foobar")
 * - Leading `*` are consumed but do NOT act as wildcards - exact matching resumes after them
 */
export function verbCaseMatch(pattern: string, word: string): boolean {
    if (pattern === word) {
        return true;
    }

    const patternChars = pattern.toLowerCase().split("");
    const wordChars = word.toLowerCase().split("");

    let patternIndex = 0;
    let wordIndex = 0;

    enum StarType {
        None = "none",
        Inner = "inner", // * in the middle of pattern
        End = "end", // * at the end of pattern
    }

    let star = StarType.None;
    let hasMatchedNonStar = false;

    // Main matching loop - mirrors Rust verbcasecmp state machine
    while (true) {
        // Handle consecutive asterisks
        while (patternIndex < patternChars.length && patternChars[patternIndex] === "*") {
            patternIndex++;
            star = patternIndex >= patternChars.length
                ? StarType.End
                : (hasMatchedNonStar ? StarType.Inner : StarType.None); // Leading asterisks don't count as wildcards
        }

        // Check if we can continue matching
        if (patternIndex >= patternChars.length) {
            break; // End of pattern
        }
        if (wordIndex >= wordChars.length) {
            break; // End of word but pattern continues
        }
        if (patternChars[patternIndex] === wordChars[wordIndex]) {
            // Characters match, advance both
            patternIndex++;
            wordIndex++;
            hasMatchedNonStar = true;
        } else {
            break; // Characters don't match
        }
    }

    // Determine if we have a match based on what's left
    const wordConsumed = wordIndex >= wordChars.length;
    const patternConsumed = patternIndex >= patternChars.length;

    if (wordConsumed && star === StarType.None) {
        return patternConsumed; // Exact match
    }
    if (wordConsumed) {
        return true; // Word consumed and we had a wildcard
    }
    if (star === StarType.End) {
        return true; // Trailing wildcard matches remaining word
    }

    return false; // No match
}

/**
 * Filters API action suggestions based on input - only filters @ commands unless user types @
 */
export function filterActionSuggestionsByInput(actions: ActionSuggestion[], input: string): ActionSuggestion[] {
    const query = input.trim();

    // Handle @ commands specially - only show them if user typed @
    if (query.startsWith("@")) {
        // User typed @, so show only @ commands and filter by what follows @
        const atQuery = query.substring(1).toLowerCase(); // Remove the @
        return actions.filter(action => {
            return action.verb_aliases.some(alias => {
                if (alias.startsWith("@")) {
                    const aliasWithoutAt = alias.substring(1).toLowerCase();
                    return aliasWithoutAt.startsWith(atQuery);
                }
                return false; // Only show @ commands when user types @
            });
        });
    } else {
        // User didn't type @, so hide @ commands but show everything else
        return actions.filter(action => {
            // Skip if all aliases are @ commands
            return !action.verb_aliases.every(alias => alias.startsWith("@"));
        });
    }
}
