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
