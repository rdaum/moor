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

// ! Hook to detect touch-capable devices

import { useEffect, useState } from "react";

/**
 * Detects if the device has touch capabilities
 * Uses both pointer media query and maxTouchPoints for reliability
 */
export const useTouchDevice = (): boolean => {
    const [isTouchDevice, setIsTouchDevice] = useState(() => {
        if (typeof window === "undefined") return false;

        // Check for touch points (most reliable)
        if (navigator.maxTouchPoints > 0) return true;

        // Fallback to checking if touch events exist
        if ("ontouchstart" in window) return true;

        return false;
    });

    useEffect(() => {
        if (typeof window === "undefined") return;

        // Also listen to pointer media query changes
        const pointerQuery = window.matchMedia("(pointer: coarse)");
        const handleChange = () => {
            setIsTouchDevice(
                navigator.maxTouchPoints > 0
                    || "ontouchstart" in window
                    || pointerQuery.matches,
            );
        };

        handleChange();
        pointerQuery.addEventListener("change", handleChange);

        return () => {
            pointerQuery.removeEventListener("change", handleChange);
        };
    }, []);

    return isTouchDevice;
};
