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

// ! Mobile top navigation bar with hamburger menu and settings

import React from "react";
import { useTitle } from "../hooks/useTitle";

interface TopNavBarProps {
    onSettingsToggle: () => void;
    onAccountToggle: () => void;
    onBrowserToggle?: () => void;
    narrativeFontSize: number;
    onDecreaseNarrativeFontSize: () => void;
    onIncreaseNarrativeFontSize: () => void;
}

export const TopNavBar: React.FC<TopNavBarProps> = ({
    onSettingsToggle,
    onAccountToggle,
    onBrowserToggle,
    narrativeFontSize,
    onDecreaseNarrativeFontSize,
    onIncreaseNarrativeFontSize,
}) => {
    const title = useTitle();

    return (
        <div className="top-nav-bar">
            <button
                className="hamburger-menu"
                onClick={onSettingsToggle}
                aria-label="Open settings menu"
            >
                <span className="hamburger-line"></span>
                <span className="hamburger-line"></span>
                <span className="hamburger-line"></span>
            </button>

            <div className="nav-title">{title}</div>

            <div className="flex gap-sm items-center">
                <div className="font-size-control">
                    <button
                        onClick={onDecreaseNarrativeFontSize}
                        aria-label="Decrease narrative font size"
                        className="font-size-button"
                        disabled={narrativeFontSize <= 10}
                    >
                        –
                    </button>
                    <span
                        className="font-size-display"
                        aria-live="polite"
                    >
                        {narrativeFontSize}px
                    </span>
                    <button
                        onClick={onIncreaseNarrativeFontSize}
                        aria-label="Increase narrative font size"
                        className="font-size-button"
                        disabled={narrativeFontSize >= 24}
                    >
                        +
                    </button>
                </div>
                {onBrowserToggle && (
                    <button
                        className="account-icon"
                        onClick={onBrowserToggle}
                        aria-label="Open object browser"
                        title="Object Browser"
                    >
                        <svg
                            width="24"
                            height="24"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                        >
                            <rect x="3" y="3" width="7" height="7" />
                            <rect x="14" y="3" width="7" height="7" />
                            <rect x="3" y="14" width="7" height="7" />
                            <rect x="14" y="14" width="7" height="7" />
                        </svg>
                    </button>
                )}

                <button
                    className="account-icon"
                    onClick={onAccountToggle}
                    aria-label="Account menu"
                >
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                        <circle cx="12" cy="8" r="4" />
                        <path d="M12 14c-4 0-8 2-8 6v2h16v-2c0-4-4-6-8-6z" />
                    </svg>
                </button>
            </div>
        </div>
    );
};
