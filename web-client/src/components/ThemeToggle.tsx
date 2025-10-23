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

import React from "react";
import { useTheme } from "./ThemeProvider";
import { type Theme } from "./themeSupport";

const THEME_SEQUENCE: Theme[] = ["dark", "light", "crt", "crt-amber"];

/**
 * Theme toggle component for switching between dark, light, and CRT modes
 *
 * @returns A button that cycles between dark, light, and CRT themes
 */
export const ThemeToggle: React.FC = () => {
    const { theme, setTheme } = useTheme();

    // Cycle through themes: dark -> light -> crt -> crt-amber -> dark
    const cycleTheme = () => {
        const currentIndex = THEME_SEQUENCE.indexOf(theme);
        const nextIndex = (currentIndex + 1) % THEME_SEQUENCE.length;
        setTheme(THEME_SEQUENCE[nextIndex]);
    };

    const getThemeDisplay = (theme: Theme) => {
        switch (theme) {
            case "dark":
                return "🌙 Dark";
            case "light":
                return "☀️ Light";
            case "crt":
                return "📺 RetroGreen";
            case "crt-amber":
                return "🟠 RetroAmber";
        }
    };

    // Return full-width clickable row for settings
    return (
        <button
            className="theme-toggle-row"
            onClick={cycleTheme}
            aria-label={`Switch theme (current: ${theme})`}
            title="Click to cycle through Dark → Light → RetroGreen → RetroAmber themes"
        >
            <span>Theme</span>
            <span className="theme-indicator">
                {getThemeDisplay(theme)}
            </span>
        </button>
    );
};
