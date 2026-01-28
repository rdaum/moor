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

// ! Emoji toggle - when enabled, text emoticons like :) are converted to unicode emoji

import React from "react";
import { usePersistentState } from "../hooks/usePersistentState";

const EMOJI_STORAGE_KEY = "emojiEnabled";

const serializeBool = (value: boolean) => value ? "true" : "false";
const deserializeBool = (raw: string): boolean | null => {
    if (raw === "true") return true;
    if (raw === "false") return false;
    return null;
};

export const EmojiToggle: React.FC = () => {
    const [emojiEnabled, setEmojiEnabled] = usePersistentState<boolean>(
        EMOJI_STORAGE_KEY,
        true,
        {
            serialize: serializeBool,
            deserialize: deserializeBool,
        },
    );

    const toggle = () => {
        setEmojiEnabled(prev => {
            const newValue = !prev;
            window.dispatchEvent(new CustomEvent("emojiChanged", { detail: newValue }));
            return newValue;
        });
    };

    return (
        <div className="settings-item">
            <span>Emoji</span>
            <button
                type="button"
                className="settings-value-button"
                onClick={toggle}
                role="switch"
                aria-checked={emojiEnabled}
                aria-label={`Emoji conversion ${emojiEnabled ? "enabled" : "disabled"}`}
                aria-describedby="emoji-description"
                title="When enabled, text emoticons like :) are converted to emoji"
            >
                {emojiEnabled ? "✓ On" : "Off"}
            </button>
            <span id="emoji-description" className="sr-only">
                When enabled, text emoticons like :) :-) ;) are converted to unicode emoji in the narrative.
            </span>
        </div>
    );
};

/**
 * Get the current emoji setting from localStorage
 */
export const getEmojiEnabled = (): boolean => {
    if (typeof window === "undefined") {
        return true;
    }
    const saved = window.localStorage.getItem(EMOJI_STORAGE_KEY);
    const parsed = saved ? deserializeBool(saved) : null;
    return parsed ?? true;
};

/**
 * Convert text emoticons to unicode emoji.
 * Handles common emoticons like :) :-) ;) ;-) :( :-( :D :-D :P :-P etc.
 *
 * Uses negative lookbehinds to avoid false positives:
 * - (?<![a-zA-Z]) prevents matching after letters (avoids http://)
 * - (?<![0-9]) prevents matching after digits (avoids "8)" in lists)
 */
export const convertEmoticons = (text: string): string => {
    // Map of emoticons to unicode emoji
    // Order matters - longer patterns first to avoid partial matches
    // Patterns use negative lookbehinds to avoid false positives
    const emoticons: [RegExp, string][] = [
        // Happy faces - require not preceded by a letter
        [/(?<![a-zA-Z]):-\)/g, "😊"],
        [/(?<![a-zA-Z]):\)/g, "🙂"],
        // Winking faces
        [/(?<![a-zA-Z]);-\)/g, "😉"],
        [/(?<![a-zA-Z]);\)/g, "😉"],
        // Sad faces
        [/(?<![a-zA-Z]):-\(/g, "😞"],
        [/(?<![a-zA-Z]):\(/g, "🙁"],
        // Laughing/grinning
        [/(?<![a-zA-Z]):-D/g, "😃"],
        [/(?<![a-zA-Z]):D/g, "😃"],
        // Tongue out
        [/(?<![a-zA-Z]):-P/gi, "😛"],
        [/(?<![a-zA-Z]):P/gi, "😛"],
        // Surprised
        [/(?<![a-zA-Z]):-O/gi, "😮"],
        [/(?<![a-zA-Z]):O/gi, "😮"],
        // Heart
        [/<3/g, "❤️"],
        // Crying
        [/(?<![a-zA-Z]):'-\(/g, "😢"],
        [/(?<![a-zA-Z]):'\(/g, "😢"],
        // Cool - require not preceded by letter or digit (avoids "8)" in numbered lists)
        [/(?<![a-zA-Z0-9])8-\)/g, "😎"],
        [/(?<![a-zA-Z0-9])8\)/g, "😎"],
        // Confused - require not preceded by letter AND not followed by / (avoids http://)
        [/(?<![a-zA-Z]):-\/(?!\/)/g, "😕"],
        [/(?<![a-zA-Z]):\/(?!\/)/g, "😕"],
        // Kiss
        [/(?<![a-zA-Z]):-\*/g, "😘"],
        [/(?<![a-zA-Z]):\*/g, "😘"],
    ];

    let result = text;
    for (const [pattern, emoji] of emoticons) {
        result = result.replace(pattern, emoji);
    }
    return result;
};
