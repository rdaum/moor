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

// ! Quick-tap verb buttons for common actions

import React, { useCallback, useEffect, useRef, useState } from "react";
import { PALETTE_VERBS } from "../lib/known-verbs";

interface VerbPaletteProps {
    visible: boolean;
    onVerbSelect: (verb: string) => void;
}

type ScrollPosition = "no-overflow" | "scrolled-start" | "scrolled-middle" | "scrolled-end";

const SWIPE_THRESHOLD = 20; // Minimum vertical distance to trigger swipe-down

export const VerbPalette: React.FC<VerbPaletteProps> = ({ visible, onVerbSelect }) => {
    const paletteRef = useRef<HTMLDivElement>(null);
    const [scrollPosition, setScrollPosition] = useState<ScrollPosition>("no-overflow");
    const touchStartRef = useRef<{ x: number; y: number; verb: string } | null>(null);

    const updateScrollPosition = useCallback(() => {
        const el = paletteRef.current;
        if (!el) return;

        const { scrollLeft, scrollWidth, clientWidth } = el;
        const hasOverflow = scrollWidth > clientWidth;

        if (!hasOverflow) {
            setScrollPosition("no-overflow");
        } else if (scrollLeft <= 1) {
            setScrollPosition("scrolled-start");
        } else if (scrollLeft + clientWidth >= scrollWidth - 1) {
            setScrollPosition("scrolled-end");
        } else {
            setScrollPosition("scrolled-middle");
        }
    }, []);

    useEffect(() => {
        const el = paletteRef.current;
        if (!el) return;

        updateScrollPosition();

        el.addEventListener("scroll", updateScrollPosition, { passive: true });
        window.addEventListener("resize", updateScrollPosition);

        return () => {
            el.removeEventListener("scroll", updateScrollPosition);
            window.removeEventListener("resize", updateScrollPosition);
        };
    }, [updateScrollPosition, visible]);

    const handlePointerDown = useCallback((e: React.PointerEvent, verb: string) => {
        // Only track touch/pen, not mouse (mouse has click)
        if (e.pointerType === "mouse") return;
        touchStartRef.current = { x: e.clientX, y: e.clientY, verb };
    }, []);

    const handlePointerUp = useCallback((e: React.PointerEvent, verb: string) => {
        if (e.pointerType === "mouse") return;
        if (!touchStartRef.current || touchStartRef.current.verb !== verb) {
            touchStartRef.current = null;
            return;
        }

        const deltaY = e.clientY - touchStartRef.current.y;
        const deltaX = Math.abs(e.clientX - touchStartRef.current.x);

        // Swipe down: positive deltaY, and more vertical than horizontal
        if (deltaY > SWIPE_THRESHOLD && deltaY > deltaX) {
            e.preventDefault();
            e.stopPropagation();
            onVerbSelect(verb);
        }

        touchStartRef.current = null;
    }, [onVerbSelect]);

    if (!visible) return null;

    return (
        <div className={`verb-palette-container ${scrollPosition}`}>
            <div
                ref={paletteRef}
                className="verb-palette"
                role="toolbar"
                aria-label="Quick command buttons. Select a verb to start a command. Swipe down on a verb to select it."
            >
                {PALETTE_VERBS.map(({ verb, label }) => (
                    <button
                        key={verb}
                        className="verb-chip"
                        onClick={() => onVerbSelect(verb)}
                        onPointerDown={(e) => handlePointerDown(e, verb)}
                        onPointerUp={(e) => handlePointerUp(e, verb)}
                        type="button"
                        aria-label={`${verb} command`}
                    >
                        {label}
                    </button>
                ))}
            </div>
        </div>
    );
};
