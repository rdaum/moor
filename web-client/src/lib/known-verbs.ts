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

// ! Known verbs for smart detection and verb palette

/**
 * Common MOO verbs that should bypass say-mode prefix.
 * When the user's input starts with one of these, send it raw.
 */
export const KNOWN_VERBS = new Set([
    // Looking and examining
    "look",
    "l",
    "examine",
    "exam",
    "ex",
    "read",
    // Movement
    "go",
    "move",
    "walk",
    "run",
    "north",
    "n",
    "south",
    "s",
    "east",
    "e",
    "west",
    "w",
    "northeast",
    "ne",
    "northwest",
    "nw",
    "southeast",
    "se",
    "southwest",
    "sw",
    "up",
    "u",
    "down",
    "d",
    "in",
    "out",
    // Object manipulation
    "get",
    "take",
    "grab",
    "pick",
    "drop",
    "put",
    "place",
    "throw",
    "give",
    "hand",
    "open",
    "close",
    "lock",
    "unlock",
    // Inventory
    "inventory",
    "inv",
    "i",
    // Communication (explicit - user wants these, not say mode)
    "say",
    "whisper",
    "shout",
    "yell",
    "tell",
    "page",
    "emote",
    "pose",
    "me",
    // Information
    "help",
    "score",
    "time",
    "date",
    "version",
    "uptime",
    "who",
    "where",
    "what",
    // Builder/programmer commands (@ prefix)
    "@examine",
    "@exam",
    "@ex",
    "@who",
    "@where",
    "@what",
    "@create",
    "@recycle",
    "@dig",
    "@describe",
    "@rename",
    "@teleport",
    "@go",
    "@verb",
    "@property",
    "@prop",
    "@edit",
    "@list",
    "@show",
    "@password",
    "@sethome",
    "@quit",
    "@set",
    "@chmod",
    "@chown",
    // Common custom verbs
    "sit",
    "stand",
    "lie",
    "sleep",
    "wake",
    "eat",
    "drink",
    "use",
    "push",
    "pull",
    "turn",
    "press",
    "wear",
    "remove",
    "wield",
    "attack",
    "kill",
    "hit",
    "buy",
    "sell",
    "trade",
    "enter",
    "exit",
    "leave",
    "climb",
]);

/**
 * Check if a command starts with a known verb
 */
export function startsWithKnownVerb(input: string): boolean {
    const trimmed = input.trim().toLowerCase();
    const firstWord = trimmed.split(/\s+/)[0];
    if (!firstWord) return false;
    return KNOWN_VERBS.has(firstWord);
}

/**
 * Verbs to show in the quick palette - subset of common actions
 */
export const PALETTE_VERBS: PaletteVerb[] = [
    { verb: "say", label: "Say", placeholder: "What would you like to say?" },
    { verb: "emote", label: "Emote", placeholder: "What are you doing?" },
    { verb: "look", label: "Look", placeholder: "Where would you like to look?" },
    { verb: "help", label: "Help", placeholder: "What do you need help with?" },
    { verb: "inventory", label: "Inv", placeholder: null },
    { verb: "get", label: "Get", placeholder: "What would you like to pick up?" },
    { verb: "drop", label: "Drop", placeholder: "What would you like to drop?" },
    { verb: "go", label: "Go", placeholder: "Where would you like to go?" },
    { verb: "examine", label: "Exam", placeholder: "What would you like to examine?" },
];

export interface PaletteVerb {
    verb: string;
    label: string;
    placeholder: string | null;
}

/**
 * Get placeholder text for a verb pill
 */
export function getVerbPlaceholder(verb: string): string | null {
    const entry = PALETTE_VERBS.find(v => v.verb === verb);
    return entry?.placeholder ?? null;
}

/**
 * Check if a word matches a verb pattern following LambdaMOO semantics.
 * Port of verbcasecmp() from mooR's Rust implementation.
 *
 * Wildcard behavior:
 * - `*` at the end: matches any string that begins with the prefix (e.g., "foo*" matches "foo", "foobar")
 * - `*` in the middle: matches any prefix of the full pattern that's at least as long as the part before the star
 *   (e.g., "foo*bar" matches "foo", "foob", "fooba", "foobar")
 * - Leading `*` are consumed but do NOT act as wildcards - exact matching resumes after them
 */
export function verbcasecmp(pattern: string, word: string): boolean {
    if (pattern.toLowerCase() === word.toLowerCase()) {
        return true;
    }

    const patternLower = pattern.toLowerCase();
    const wordLower = word.toLowerCase();

    let pi = 0; // pattern index
    let wi = 0; // word index

    type StarType = "none" | "inner" | "end";
    let star: StarType = "none";
    let hasMatchedNonStar = false;

    // Main matching loop
    while (true) {
        // Handle consecutive asterisks
        while (pi < patternLower.length && patternLower[pi] === "*") {
            pi++;
            if (pi >= patternLower.length) {
                star = "end";
            } else {
                // Only treat as inner wildcard if we've matched non-star characters before
                star = hasMatchedNonStar ? "inner" : "none";
            }
        }

        // Check if we can continue matching
        if (pi >= patternLower.length) {
            break; // End of pattern
        }
        if (wi >= wordLower.length) {
            break; // End of word but pattern continues
        }
        if (patternLower[pi] === wordLower[wi]) {
            // Characters match, advance both
            pi++;
            wi++;
            hasMatchedNonStar = true;
        } else {
            break; // Characters don't match
        }
    }

    // Determine if we have a match based on what's left
    const wordConsumed = wi >= wordLower.length;
    const patternConsumed = pi >= patternLower.length;

    if (wordConsumed && star === "none") {
        return patternConsumed; // Exact match required
    }
    if (wordConsumed) {
        return true; // Word consumed and we had a wildcard
    }
    if (star === "end") {
        return true; // Trailing wildcard matches remaining word
    }
    return false;
}

/**
 * Extract the full verb name from a pattern by removing `*`.
 * E.g., "l*ook" -> "look", "foo*" -> "foo", "*bar" -> "bar"
 */
export function extractFullVerbName(pattern: string): string {
    return pattern.replace(/\*/g, "");
}

/**
 * Parse space-separated verb names into an array.
 * E.g., "look l*ook" -> ["look", "l*ook"]
 */
export function parseVerbNames(namesString: string): string[] {
    return namesString.split(/\s+/).filter(s => s.length > 0);
}

/**
 * For tab completion: given a verb pattern and user's prefix,
 * return the suffix to show as ghosted completion text.
 * Returns null if the prefix doesn't match.
 *
 * Combines verbcasecmp semantics with simple prefix matching:
 * - For patterns with `*`: respects minimum prefix (e.g., "l*ook" requires at least "l")
 * - For patterns without `*`: allows simple prefix matching (e.g., "say" matches "sa")
 *
 * E.g., pattern="l*ook", prefix="l" -> "ook" (verbcasecmp match)
 *       pattern="l*ook", prefix="lo" -> "ok" (verbcasecmp match)
 *       pattern="say", prefix="sa" -> "y" (simple prefix match)
 *       pattern="look", prefix="look" -> "" (already complete)
 *       pattern="look", prefix="x" -> null (no match)
 */
export function getCompletionSuffix(pattern: string, prefix: string): string | null {
    const fullName = extractFullVerbName(pattern);
    const hasWildcard = pattern.includes("*");

    let isMatch = false;

    if (hasWildcard) {
        // Use verbcasecmp for patterns with wildcards (respects minimum prefix)
        isMatch = verbcasecmp(pattern, prefix);
    } else {
        // Simple prefix match for patterns without wildcards
        isMatch = fullName.toLowerCase().startsWith(prefix.toLowerCase());
    }

    if (!isMatch) {
        return null;
    }

    // If prefix is already the full name or longer, no completion needed
    if (prefix.length >= fullName.length) {
        return "";
    }

    // Return the remaining characters (preserving the case from the pattern)
    return fullName.slice(prefix.length);
}

/**
 * Find the longest common prefix among a list of strings (case-insensitive).
 * Used for bash-style tab completion.
 */
export function findCommonPrefix(strings: string[]): string {
    if (strings.length === 0) return "";
    if (strings.length === 1) return strings[0].toLowerCase();

    let prefix = strings[0].toLowerCase();
    for (let i = 1; i < strings.length; i++) {
        const s = strings[i].toLowerCase();
        while (!s.startsWith(prefix)) {
            prefix = prefix.slice(0, -1);
            if (prefix === "") return "";
        }
    }
    return prefix;
}
