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
import { CommandEchoToggle } from "./CommandEchoToggle";
import { FontSizeControl } from "./FontSizeControl";
import { FontToggle } from "./FontToggle";
import { SpeechBubbleToggle } from "./SpeechBubbleToggle";
import { ThemeToggle } from "./ThemeToggle";

interface SettingsPanelProps {
    isOpen: boolean;
    onClose: () => void;
    narrativeFontSize: number;
    onDecreaseNarrativeFontSize: () => void;
    onIncreaseNarrativeFontSize: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({
    isOpen,
    onClose,
    narrativeFontSize,
    onDecreaseNarrativeFontSize,
    onIncreaseNarrativeFontSize,
}) => {
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
                        <h3>Display</h3>
                        <ThemeToggle />
                        <FontToggle />
                        <div className="settings-item">
                            <span>Font size</span>
                            <FontSizeControl
                                fontSize={narrativeFontSize}
                                onDecrease={onDecreaseNarrativeFontSize}
                                onIncrease={onIncreaseNarrativeFontSize}
                            />
                        </div>
                    </div>

                    <div className="settings-section">
                        <h3>Interface</h3>
                        <CommandEchoToggle />
                        <SpeechBubbleToggle />
                    </div>

                    <div className="settings-section">
                        <h3>About</h3>
                        <div className="settings-item">
                            <span>Version</span>
                            <span className="font-mono text-sm text-secondary">
                                {__GIT_HASH__}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </>
    );
};
