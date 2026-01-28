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

// ! Verb palette toggle - show/hide quick verb buttons above input

import React from "react";
import { usePersistentState } from "../hooks/usePersistentState";

const VERB_PALETTE_STORAGE_KEY = "verbPaletteEnabled";

const serializeBool = (value: boolean) => value ? "true" : "false";
const deserializeBool = (raw: string): boolean | null => {
    if (raw === "true") return true;
    if (raw === "false") return false;
    return null;
};

export const VerbPaletteToggle: React.FC = () => {
    const [paletteEnabled, setPaletteEnabled] = usePersistentState<boolean>(
        VERB_PALETTE_STORAGE_KEY,
        true,
        {
            serialize: serializeBool,
            deserialize: deserializeBool,
        },
    );

    const toggle = () => {
        setPaletteEnabled(prev => {
            const newValue = !prev;
            window.dispatchEvent(new CustomEvent("verbPaletteChanged", { detail: newValue }));
            return newValue;
        });
    };

    return (
        <div className="settings-item">
            <span>Verb Palette</span>
            <button
                type="button"
                className="settings-value-button"
                onClick={toggle}
                role="switch"
                aria-checked={paletteEnabled}
                aria-label={`Verb palette ${paletteEnabled ? "enabled" : "disabled"}`}
                aria-describedby="verb-palette-description"
                title="Show quick verb buttons above the input area"
            >
                {paletteEnabled ? "✓ On" : "Off"}
            </button>
            <span id="verb-palette-description" className="sr-only">
                Shows a row of quick-tap verb buttons above the input area for common actions.
            </span>
        </div>
    );
};

/**
 * Get the current verb palette setting from localStorage
 */
export const getVerbPaletteEnabled = (): boolean => {
    if (typeof window === "undefined") {
        return true;
    }
    const saved = window.localStorage.getItem(VERB_PALETTE_STORAGE_KEY);
    const parsed = saved ? deserializeBool(saved) : null;
    return parsed ?? true;
};
