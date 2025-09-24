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

import React, { useEffect, useState } from "react";
import { applyTheme } from "./ThemeProvider";

type Theme = "dark" | "light" | "crt" | "crt-amber";

/**
 * Theme toggle component for switching between dark, light, and CRT modes
 *
 * @returns A button that cycles between dark, light, and CRT themes
 */
export const ThemeToggle: React.FC = () => {
    // Check if user has a saved theme preference
    const savedTheme = localStorage.getItem("theme") as Theme | null;
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

    // Initialize theme state (use saved preference, fallback to system preference, default to dark)
    const [currentTheme, setCurrentTheme] = useState<Theme>(
        savedTheme || (prefersDark ? "dark" : "light"),
    );

    // Apply the theme class when state changes
    useEffect(() => {
        applyTheme(currentTheme);
    }, [currentTheme]);

    // Cycle through themes: dark -> light -> crt -> crt-amber -> dark
    const cycleTheme = () => {
        const themeOrder: Theme[] = ["dark", "light", "crt", "crt-amber"];
        const currentIndex = themeOrder.indexOf(currentTheme);
        const nextIndex = (currentIndex + 1) % themeOrder.length;
        setCurrentTheme(themeOrder[nextIndex]);
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
            aria-label={`Switch theme (current: ${currentTheme})`}
            title="Click to cycle through Dark → Light → RetroGreen → RetroAmber themes"
        >
            <span>Theme</span>
            <span className="theme-indicator">
                {getThemeDisplay(currentTheme)}
            </span>
        </button>
    );
};
