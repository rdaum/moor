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

// ! Settings panel with theme toggle and other options

import React from "react";
import { CommandEchoToggle } from "./CommandEchoToggle";
import { EmojiToggle } from "./EmojiToggle";
import { FontSizeControl } from "./FontSizeControl";
import { FontToggle } from "./FontToggle";
import { SayModeToggle } from "./SayModeToggle";
import { ThemeToggle } from "./ThemeToggle";
import { VerbPaletteToggle } from "./VerbPaletteToggle";

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
                        <EmojiToggle />
                    </div>

                    <div className="settings-section">
                        <h3>Interface</h3>
                        <CommandEchoToggle />
                        <SayModeToggle />
                        <VerbPaletteToggle />
                    </div>

                    <div className="settings-section">
                        <h3>About</h3>
                        <div className="settings-item">
                            <span>Version</span>
                            <button
                                className="version-copy-button"
                                onClick={(e) => {
                                    navigator.clipboard.writeText(__GIT_HASH__);
                                    const btn = e.currentTarget;
                                    btn.textContent = "Copied!";
                                    setTimeout(() => {
                                        btn.textContent = __GIT_HASH__;
                                    }, 1500);
                                }}
                                title="Click to copy"
                            >
                                {__GIT_HASH__}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </>
    );
};
