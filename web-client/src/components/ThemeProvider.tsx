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

import React, { createContext, type Dispatch, type SetStateAction, useContext, useEffect, useMemo } from "react";
import { usePersistentState } from "../hooks/usePersistentState";
import {
    applyFontStyle,
    applyThemeToDom,
    FONT_STORAGE_KEY,
    type FontStyle,
    isFontStyle,
    isTheme,
    resolveInitialFontStyle,
    resolveInitialTheme,
    RETRO_THEMES,
    type Theme,
    THEME_STORAGE_KEY,
} from "./themeSupport";

interface ThemeContextValue {
    theme: Theme;
    setTheme: Dispatch<SetStateAction<Theme>>;
    fontStyle: FontStyle;
    setFontStyle: Dispatch<SetStateAction<FontStyle>>;
    isRetroTheme: boolean;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

/**
 * Theme provider component that initializes theme and font style preferences
 * and exposes them via React context.
 */
export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [theme, setTheme] = usePersistentState<Theme>(
        THEME_STORAGE_KEY,
        resolveInitialTheme,
        {
            serialize: value => value,
            deserialize: value => (isTheme(value) ? value : null),
        },
    );
    const [fontStyle, setFontStyle] = usePersistentState<FontStyle>(
        FONT_STORAGE_KEY,
        resolveInitialFontStyle,
        {
            serialize: value => value,
            deserialize: value => (isFontStyle(value) ? value : null),
        },
    );

    useEffect(() => {
        applyThemeToDom(theme);
    }, [theme]);

    useEffect(() => {
        if (RETRO_THEMES.has(theme)) {
            applyFontStyle("proportional");
            return;
        }
        applyFontStyle(fontStyle);
    }, [theme, fontStyle]);

    const value = useMemo<ThemeContextValue>(() => ({
        theme,
        setTheme,
        fontStyle,
        setFontStyle,
        isRetroTheme: RETRO_THEMES.has(theme),
    }), [theme, fontStyle, setTheme, setFontStyle]);

    return (
        <ThemeContext.Provider value={value}>
            {children}
        </ThemeContext.Provider>
    );
};

export const useTheme = () => {
    const context = useContext(ThemeContext);
    if (!context) {
        throw new Error("useTheme must be used within a ThemeProvider");
    }
    return context;
};
