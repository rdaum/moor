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

// ! Font style toggle for switching between proportional and monospace fonts

import React, { useEffect, useState } from "react";

type FontStyle = "proportional" | "monospace";

/**
 * Font toggle component for switching between proportional and monospace fonts
 * Hidden for retro themes (CRT/Amber) which always use monospace
 *
 * @returns A button that toggles between proportional (default) and monospace fonts, or null if retro theme
 */
export const FontToggle: React.FC = () => {
    // Track current theme to hide toggle for retro themes
    const [currentTheme, setCurrentTheme] = useState<string>(
        localStorage.getItem("theme") || "dark",
    );

    // Check if user has a saved font preference
    const savedFont = localStorage.getItem("font-style") as FontStyle | null;

    // Initialize font state (default to proportional)
    const [currentFont, setCurrentFont] = useState<FontStyle>(
        savedFont || "proportional",
    );

    // Listen for theme changes
    useEffect(() => {
        const checkTheme = () => {
            const theme = localStorage.getItem("theme") || "dark";
            setCurrentTheme(theme);
        };

        // Check theme on mount and when storage changes
        checkTheme();
        window.addEventListener("storage", checkTheme);

        // Poll for theme changes (since ThemeToggle doesn't dispatch events)
        const interval = setInterval(checkTheme, 500);

        return () => {
            window.removeEventListener("storage", checkTheme);
            clearInterval(interval);
        };
    }, []);

    // Apply the font style when state changes
    useEffect(() => {
        const isRetroTheme = currentTheme === "crt" || currentTheme === "crt-amber";
        // Don't apply font style for retro themes - they handle it themselves
        if (!isRetroTheme) {
            applyFontStyle(currentFont);
            // Save preference
            localStorage.setItem("font-style", currentFont);
        }
    }, [currentFont, currentTheme]);

    // Don't render for retro themes
    const isRetroTheme = currentTheme === "crt" || currentTheme === "crt-amber";
    if (isRetroTheme) {
        return null;
    }

    // Toggle between proportional and monospace
    const toggleFont = () => {
        setCurrentFont(current => current === "proportional" ? "monospace" : "proportional");
    };

    const getFontDisplay = (font: FontStyle) => {
        switch (font) {
            case "proportional":
                return "Aa Proportional";
            case "monospace":
                return "⌨ Monospace";
        }
    };

    return (
        <button
            className="font-toggle-row"
            onClick={toggleFont}
            aria-label={`Switch font style (current: ${currentFont})`}
            title="Toggle between proportional and monospace fonts"
        >
            <span>Font</span>
            <span className="font-indicator">
                {getFontDisplay(currentFont)}
            </span>
        </button>
    );
};

/**
 * Apply font style by updating CSS custom property
 */
export function applyFontStyle(fontStyle: FontStyle) {
    const root = document.documentElement;

    if (fontStyle === "monospace") {
        // Override --font-sans to use monospace
        root.style.setProperty("--font-sans", "var(--font-mono)");
    } else {
        // Reset to default (remove override, let CSS variable definition take over)
        root.style.removeProperty("--font-sans");
    }
}
