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

// ! Command echo toggle component for enabling/disabling input command echoing

import React from "react";
import { usePersistentState } from "../hooks/usePersistentState";

const ECHO_STORAGE_KEY = "echoCommands";

const serializeEcho = (value: boolean) => value ? "true" : "false";
const deserializeEcho = (raw: string): boolean | null => {
    if (raw === "true") return true;
    if (raw === "false") return false;
    return null;
};

/**
 * Command echo toggle component for controlling whether typed commands are echoed to output
 *
 * @returns A button that toggles command echoing on/off
 */
export const CommandEchoToggle: React.FC = () => {
    const [echoEnabled, setEchoEnabled] = usePersistentState<boolean>(
        ECHO_STORAGE_KEY,
        true,
        {
            serialize: serializeEcho,
            deserialize: deserializeEcho,
        },
    );

    const toggleEcho = () => {
        setEchoEnabled(prev => {
            const newValue = !prev;
            // Dispatch custom event for same-tab listeners
            window.dispatchEvent(new CustomEvent("commandEchoChanged", { detail: newValue }));
            return newValue;
        });
    };

    return (
        <div className="settings-item">
            <span>Echo Commands</span>
            <button
                className="settings-value-button"
                onClick={toggleEcho}
                role="switch"
                aria-checked={echoEnabled}
                aria-label={`Command echoing ${echoEnabled ? "enabled" : "disabled"}`}
                aria-describedby="echo-description"
                title="Toggle whether typed commands are echoed to the output window"
            >
                {echoEnabled ? "✓ On" : "Off"}
            </button>
            <span id="echo-description" className="sr-only">
                Controls whether your typed commands appear in the output window. Helpful for screen readers when
                disabled.
            </span>
        </div>
    );
};

/**
 * Get the current command echo setting from localStorage
 */
export const getCommandEchoEnabled = (): boolean => {
    if (typeof window === "undefined") {
        return true;
    }
    const savedEchoSetting = window.localStorage.getItem(ECHO_STORAGE_KEY);
    const parsed = savedEchoSetting ? deserializeEcho(savedEchoSetting) : null;
    return parsed ?? true;
};
