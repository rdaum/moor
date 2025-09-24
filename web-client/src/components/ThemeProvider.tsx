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

import React, { useEffect } from "react";

type Theme = "dark" | "light" | "crt" | "crt-amber";

/**
 * Theme provider component that initializes the theme on app load
 * This runs independently of the settings panel and theme toggle
 */
export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    // Initialize theme on component mount
    useEffect(() => {
        // Check if user has a saved theme preference
        const savedTheme = localStorage.getItem("theme") as Theme | null;
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;

        // Initialize theme (use saved preference, fallback to system preference, default to dark)
        const initialTheme: Theme = savedTheme || (prefersDark ? "dark" : "light");

        // Apply the theme class immediately
        applyTheme(initialTheme);
    }, []);

    return <>{children}</>;
};

/**
 * Apply theme class to document body
 * This function is used by both ThemeProvider and ThemeToggle
 */
export const applyTheme = (theme: Theme) => {
    // Remove all theme classes
    document.body.classList.remove("light-theme", "crt-theme", "crt-amber-theme");

    // Apply current theme class (dark is default, no class needed)
    if (theme === "light") {
        document.body.classList.add("light-theme");
    } else if (theme === "crt") {
        document.body.classList.add("crt-theme");
    } else if (theme === "crt-amber") {
        document.body.classList.add("crt-amber-theme");
    }

    // Save to localStorage
    localStorage.setItem("theme", theme);

    // Update scanlines for CRT themes
    updateScanlines(theme);
};

/**
 * Update scanline elements based on theme
 */
const updateScanlines = (theme: Theme) => {
    // Remove existing scanline elements
    const existingScanlines = document.querySelectorAll(".crt-scanlines");
    existingScanlines.forEach(el => el.remove());

    if (theme === "crt" || theme === "crt-amber") {
        // Create static scanlines only
        const staticScanlines = document.createElement("div");
        staticScanlines.className = "crt-scanlines";
        staticScanlines.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            pointer-events: none;
            background: linear-gradient(
                transparent 50%,
                ${theme === "crt" ? "rgba(0, 255, 65, 0.008)" : "rgba(255, 161, 40, 0.008)"} 50%
            );
            background-size: 100% 4px;
            z-index: 1000;
        `;
        document.body.appendChild(staticScanlines);
    }
};
