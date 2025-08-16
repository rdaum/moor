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

/**
 * Theme toggle component for switching between light and dark modes
 *
 * @returns A button that toggles between light and dark themes, hidden until hover
 */
export const ThemeToggle: React.FC = () => {
    // Check if user has a saved theme preference
    const savedTheme = localStorage.getItem("theme");
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

    // Initialize theme state (use saved preference, fallback to system preference, default to dark)
    const [isDarkTheme, setIsDarkTheme] = useState<boolean>(
        savedTheme ? savedTheme === "dark" : prefersDark,
    );

    // Apply the theme class on initial load and when state changes
    useEffect(() => {
        if (isDarkTheme) {
            document.body.classList.remove("light-theme");
            localStorage.setItem("theme", "dark");
        } else {
            document.body.classList.add("light-theme");
            localStorage.setItem("theme", "light");
        }
    }, [isDarkTheme]);

    // Toggle theme function
    const toggleTheme = () => {
        console.log('Theme toggle clicked! Current isDarkTheme:', isDarkTheme);
        setIsDarkTheme(!isDarkTheme);
        console.log('Theme toggle - new state should be:', !isDarkTheme);
    };

    // Return full-width clickable row for settings
    return (
        <button
            className="theme-toggle-row"
            onClick={toggleTheme}
            aria-label={`Switch to ${isDarkTheme ? 'light' : 'dark'} theme`}
            aria-pressed={isDarkTheme ? "true" : "false"}
        >
            <span>Theme</span>
            <span className="theme-indicator">
                {isDarkTheme ? "🌙 Dark" : "☀️ Light"}
            </span>
        </button>
    );
};
