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

// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com>
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

// ! Shared theme and font helpers for the web client

export type Theme = "dark" | "light" | "crt" | "crt-amber";
export type FontStyle = "proportional" | "monospace";

export const RETRO_THEMES: ReadonlySet<Theme> = new Set(["crt", "crt-amber"]);

export const THEME_STORAGE_KEY = "theme";
export const FONT_STORAGE_KEY = "font-style";

const hasWindow = () => typeof window !== "undefined";
const hasDocument = () => typeof document !== "undefined";

const isTheme = (value: string | null): value is Theme =>
    value === "dark" || value === "light" || value === "crt" || value === "crt-amber";

const isFontStyle = (value: string | null): value is FontStyle => value === "proportional" || value === "monospace";

export const loadStoredTheme = (): Theme | null => {
    if (!hasWindow()) return null;
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isTheme(stored) ? stored : null;
};

export const loadStoredFontStyle = (): FontStyle | null => {
    if (!hasWindow()) return null;
    const stored = window.localStorage.getItem(FONT_STORAGE_KEY);
    return isFontStyle(stored) ? stored : null;
};

export const resolveInitialTheme = (): Theme => {
    const stored = loadStoredTheme();
    if (stored) return stored;
    if (hasWindow()) {
        const prefersDark = window.matchMedia
            ? window.matchMedia("(prefers-color-scheme: dark)").matches
            : false;
        if (prefersDark) return "dark";
    }
    return "light";
};

export const resolveInitialFontStyle = (): FontStyle => {
    const stored = loadStoredFontStyle();
    return stored ?? "proportional";
};

export const persistTheme = (theme: Theme) => {
    if (!hasWindow()) return;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
};

export const persistFontStyle = (fontStyle: FontStyle) => {
    if (!hasWindow()) return;
    window.localStorage.setItem(FONT_STORAGE_KEY, fontStyle);
};

/**
 * Apply theme class to the document body, including CRT scanlines for retro themes.
 */
export const applyThemeToDom = (theme: Theme) => {
    if (!hasDocument()) return;

    document.body.classList.remove("light-theme", "crt-theme", "crt-amber-theme");

    if (theme === "light") {
        document.body.classList.add("light-theme");
    } else if (theme === "crt") {
        document.body.classList.add("crt-theme");
    } else if (theme === "crt-amber") {
        document.body.classList.add("crt-amber-theme");
    }

    updateScanlines(theme);
};

/**
 * Apply font style by updating the relevant CSS custom property.
 */
export const applyFontStyle = (fontStyle: FontStyle) => {
    if (!hasDocument()) return;

    const root = document.documentElement;
    if (fontStyle === "monospace") {
        root.style.setProperty("--font-sans", "var(--font-mono)");
    } else {
        root.style.removeProperty("--font-sans");
    }
};

const updateScanlines = (theme: Theme) => {
    if (!hasDocument()) return;

    const existingScanlines = document.querySelectorAll(".crt-scanlines");
    existingScanlines.forEach(el => el.remove());

    if (!RETRO_THEMES.has(theme)) {
        return;
    }

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
};
