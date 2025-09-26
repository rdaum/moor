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

// ! Command echo toggle component for enabling/disabling input command echoing

import React, { useEffect, useState } from "react";

/**
 * Command echo toggle component for controlling whether typed commands are echoed to output
 *
 * @returns A button that toggles command echoing on/off
 */
export const CommandEchoToggle: React.FC = () => {
    // Check if user has a saved preference, default to true (echo enabled)
    const savedEchoSetting = localStorage.getItem("echoCommands");
    const [echoEnabled, setEchoEnabled] = useState<boolean>(
        savedEchoSetting !== null ? savedEchoSetting === "true" : true,
    );

    // Save preference when state changes
    useEffect(() => {
        localStorage.setItem("echoCommands", echoEnabled.toString());
    }, [echoEnabled]);

    const toggleEcho = () => {
        setEchoEnabled(prev => !prev);
    };

    // Return full-width clickable row for settings
    return (
        <button
            className="theme-toggle-row"
            onClick={toggleEcho}
            role="switch"
            aria-checked={echoEnabled}
            aria-label={`Command echoing ${echoEnabled ? "enabled" : "disabled"}`}
            aria-describedby="echo-description"
            title="Toggle whether typed commands are echoed to the output window"
        >
            <span>Echo Commands</span>
            <span className="theme-indicator" aria-live="polite">
                {echoEnabled ? "✅ On" : "❌ Off"}
            </span>
            <span id="echo-description" className="sr-only">
                Controls whether your typed commands appear in the output window. Helpful for screen readers when
                disabled.
            </span>
        </button>
    );
};

/**
 * Get the current command echo setting from localStorage
 */
export const getCommandEchoEnabled = (): boolean => {
    const savedEchoSetting = localStorage.getItem("echoCommands");
    return savedEchoSetting !== null ? savedEchoSetting === "true" : true;
};
