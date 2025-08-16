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

interface SettingsPanelProps {
    isOpen: boolean;
    onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose }) => {
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
