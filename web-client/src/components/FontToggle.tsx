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

import React from "react";
import { useTheme } from "./ThemeProvider";
import { type FontStyle } from "./themeSupport";

const getFontDisplay = (font: FontStyle) => {
    switch (font) {
        case "proportional":
            return "Aa Proportional";
        case "monospace":
            return "⌨ Monospace";
    }
};

/**
 * Font toggle component for switching between proportional and monospace fonts.
 * Hidden for retro themes (CRT/Amber) which always use monospace.
 */
export const FontToggle: React.FC = () => {
    const { fontStyle, setFontStyle, isRetroTheme } = useTheme();

    if (isRetroTheme) {
        return null;
    }

    const toggleFont = () => {
        setFontStyle(current => current === "proportional" ? "monospace" : "proportional");
    };

    return (
        <div className="settings-item">
            <span>Font</span>
            <button
                className="settings-value-button"
                onClick={toggleFont}
                aria-label={`Switch font style (current: ${fontStyle})`}
                title="Toggle between proportional and monospace fonts"
            >
                {getFontDisplay(fontStyle)}
            </button>
        </div>
    );
};
