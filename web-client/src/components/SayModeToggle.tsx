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

// ! Say mode toggle - when enabled, input defaults to "say" command

import React from "react";
import { usePersistentState } from "../hooks/usePersistentState";

const SAY_MODE_STORAGE_KEY = "sayModeEnabled";

const serializeBool = (value: boolean) => value ? "true" : "false";
const deserializeBool = (raw: string): boolean | null => {
    if (raw === "true") return true;
    if (raw === "false") return false;
    return null;
};

export const SayModeToggle: React.FC = () => {
    const [sayModeEnabled, setSayModeEnabled] = usePersistentState<boolean>(
        SAY_MODE_STORAGE_KEY,
        false,
        {
            serialize: serializeBool,
            deserialize: deserializeBool,
        },
    );

    const toggle = () => {
        setSayModeEnabled(prev => {
            const newValue = !prev;
            window.dispatchEvent(new CustomEvent("sayModeChanged", { detail: newValue }));
            return newValue;
        });
    };

    return (
        <div className="settings-item">
            <span>Say Mode</span>
            <button
                className="settings-value-button"
                onClick={toggle}
                role="switch"
                aria-checked={sayModeEnabled}
                aria-label={`Say mode ${sayModeEnabled ? "enabled" : "disabled"}`}
                aria-describedby="say-mode-description"
                title="When enabled, text input defaults to 'say' command"
            >
                {sayModeEnabled ? "✓ On" : "Off"}
            </button>
            <span id="say-mode-description" className="sr-only">
                When enabled, typed text will be sent as speech by default. Backspace to switch to command mode, or
                start with a known verb.
            </span>
        </div>
    );
};

/**
 * Get the current say mode setting from localStorage
 */
export const getSayModeEnabled = (): boolean => {
    if (typeof window === "undefined") {
        return false;
    }
    const saved = window.localStorage.getItem(SAY_MODE_STORAGE_KEY);
    const parsed = saved ? deserializeBool(saved) : null;
    return parsed ?? false;
};
