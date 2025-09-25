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

// ! Settings panel with theme toggle and other options

import React from "react";
import { ThemeToggle } from "./ThemeToggle";

export type ConnectionModePreference = "auto" | "sse" | "websocket";

interface ConnectionModeToggleProps {
    value: ConnectionModePreference;
    onChange: (mode: ConnectionModePreference) => void;
}

const ConnectionModeToggle: React.FC<ConnectionModeToggleProps> = ({ value, onChange }) => {
    return (
        <div className="connection-mode-toggle" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                    type="radio"
                    name="connection-mode"
                    value="auto"
                    checked={value === "auto"}
                    onChange={() => onChange("auto")}
                />
                Auto-detect
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                    type="radio"
                    name="connection-mode"
                    value="sse"
                    checked={value === "sse"}
                    onChange={() => onChange("sse")}
                />
                Server-Sent Events
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                    type="radio"
                    name="connection-mode"
                    value="websocket"
                    checked={value === "websocket"}
                    onChange={() => onChange("websocket")}
                />
                WebSocket
            </label>
        </div>
    );
};

interface SettingsPanelProps {
    isOpen: boolean;
    onClose: () => void;
    connectionMode: ConnectionModePreference;
    onConnectionModeChange: (mode: ConnectionModePreference) => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = (
    { isOpen, onClose, connectionMode, onConnectionModeChange },
) => {
    if (!isOpen) return null;

    return (
        <>
            {/* Backdrop */}
            <div className="settings-backdrop" onClick={onClose} />

            {/* Settings panel */}
            <div className="settings-panel">
                <div className="settings-header">
                    <h2>Settings</h2>
                    <button
                        className="settings-close"
                        onClick={onClose}
                        aria-label="Close settings"
                    >
                        ×
                    </button>
                </div>

                <div className="settings-content">
                    <div className="settings-section">
                        <h3>Appearance</h3>
                        <ThemeToggle />
                    </div>

                    <div className="settings-section">
                        <h3>Connection</h3>
                        <div className="settings-item">
                            <span>Connection Type</span>
                            <ConnectionModeToggle
                                value={connectionMode}
                                onChange={onConnectionModeChange}
                            />
                        </div>
                    </div>

                    <div className="settings-section">
                        <h3>Account</h3>
                        <div className="settings-item">
                            <span>Profile settings</span>
                            <span className="settings-placeholder">Coming soon</span>
                        </div>
                    </div>
                </div>
            </div>
        </>
    );
};
