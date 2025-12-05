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

// ! Speech bubble toggle component for enabling/disabling speech bubble rendering

import React from "react";
import { usePersistentState } from "../hooks/usePersistentState";
import { useTheme } from "./ThemeProvider";

const SPEECH_BUBBLE_STORAGE_KEY = "speechBubblesEnabled";

const serializeBool = (value: boolean) => value ? "true" : "false";
const deserializeBool = (raw: string): boolean | null => {
    if (raw === "true") return true;
    if (raw === "false") return false;
    return null;
};

/**
 * Speech bubble toggle component for controlling whether say events render as speech bubbles
 * Hidden for retro themes (CRT/Amber) which don't support speech bubbles.
 */
export const SpeechBubbleToggle: React.FC = () => {
    const { isRetroTheme } = useTheme();
    const [enabled, setEnabled] = usePersistentState<boolean>(
        SPEECH_BUBBLE_STORAGE_KEY,
        false,
        {
            serialize: serializeBool,
            deserialize: deserializeBool,
        },
    );

    if (isRetroTheme) {
        return null;
    }

    const toggle = () => {
        setEnabled(prev => {
            const newValue = !prev;
            // Dispatch custom event for same-tab listeners
            window.dispatchEvent(new CustomEvent("speechBubblesChanged", { detail: newValue }));
            return newValue;
        });
    };

    return (
        <div className="settings-item">
            <span>Speech Bubbles</span>
            <button
                className="settings-value-button"
                onClick={toggle}
                role="switch"
                aria-checked={enabled}
                aria-label={`Speech bubbles ${enabled ? "enabled" : "disabled"}`}
                title="Toggle whether say events render as speech bubbles"
            >
                {enabled ? "✅ On" : "❌ Off"}
            </button>
        </div>
    );
};

/**
 * Get the current speech bubble setting from localStorage
 */
export const getSpeechBubblesEnabled = (): boolean => {
    if (typeof window === "undefined") {
        return false;
    }
    const saved = window.localStorage.getItem(SPEECH_BUBBLE_STORAGE_KEY);
    const parsed = saved ? deserializeBool(saved) : null;
    return parsed ?? false;
};
