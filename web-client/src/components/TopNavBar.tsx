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
    onBrowserToggle?: () => void;
    narrativeFontSize: number;
    onDecreaseNarrativeFontSize: () => void;
    onIncreaseNarrativeFontSize: () => void;
}

export const TopNavBar: React.FC<TopNavBarProps> = ({
    onSettingsToggle,
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

            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <div
                    style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "4px",
                        backgroundColor: "var(--color-bg-secondary)",
                        border: "1px solid var(--color-border-medium)",
                        borderRadius: "var(--radius-sm)",
                        padding: "2px 6px",
                    }}
                >
                    <button
                        onClick={onDecreaseNarrativeFontSize}
                        aria-label="Decrease narrative font size"
                        style={{
                            background: "transparent",
                            border: "none",
                            color: "var(--color-text-secondary)",
                            cursor: narrativeFontSize <= 10 ? "not-allowed" : "pointer",
                            opacity: narrativeFontSize <= 10 ? 0.5 : 1,
                            fontSize: "14px",
                            padding: "2px 4px",
                        }}
                        disabled={narrativeFontSize <= 10}
                    >
                        –
                    </button>
                    <span
                        style={{
                            fontFamily: "var(--font-mono)",
                            fontSize: "12px",
                            color: "var(--color-text-secondary)",
                            minWidth: "38px",
                            textAlign: "center",
                        }}
                        aria-live="polite"
                    >
                        {narrativeFontSize}px
                    </span>
                    <button
                        onClick={onIncreaseNarrativeFontSize}
                        aria-label="Increase narrative font size"
                        style={{
                            background: "transparent",
                            border: "none",
                            color: "var(--color-text-secondary)",
                            cursor: narrativeFontSize >= 24 ? "not-allowed" : "pointer",
                            opacity: narrativeFontSize >= 24 ? 0.5 : 1,
                            fontSize: "14px",
                            padding: "2px 4px",
                        }}
                        disabled={narrativeFontSize >= 24}
                    >
                        +
                    </button>
                </div>
                {onBrowserToggle && (
                    <button
                        className="browser-icon"
                        onClick={onBrowserToggle}
                        aria-label="Open object browser"
                        title="Object Browser"
                        style={{
                            background: "none",
                            border: "none",
                            cursor: "pointer",
                            padding: "8px",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            color: "var(--color-text-primary)",
                        }}
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
                    aria-label="Account settings"
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
