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

// ! Quick-tap verb buttons for common actions

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAuthContext } from "../context/AuthContext";
import { useVerbSuggestions, VerbSuggestion } from "../hooks/useVerbSuggestions";
import { PALETTE_VERBS, PaletteVerb } from "../lib/known-verbs";

interface VerbPaletteProps {
    visible: boolean;
    onVerbSelect: (verb: string, placeholder: string | null) => void;
}

type ScrollPosition = "no-overflow" | "scrolled-start" | "scrolled-middle" | "scrolled-end";

const SWIPE_THRESHOLD = 20; // Minimum vertical distance to trigger swipe-down

// Extract the base verb name from patterns like "l*ook" -> "look"
function extractVerbName(verbPattern: string): string {
    // Handle abbreviation patterns like "l*ook" -> "look"
    return verbPattern.replace("*", "");
}

// Extract label from verb pattern: "l*ook" -> "Look", "inventory" -> "Inv" (if in static list)
function extractVerbLabel(verbPattern: string): string {
    const baseName = extractVerbName(verbPattern);
    // Check if we have a custom label in static palette
    const staticEntry = PALETTE_VERBS.find(v => v.verb === baseName);
    if (staticEntry) {
        return staticEntry.label;
    }
    // Capitalize first letter
    return baseName.charAt(0).toUpperCase() + baseName.slice(1);
}

// Convert server suggestion to display format
function suggestionToDisplay(suggestion: VerbSuggestion): PaletteVerb {
    const verb = extractVerbName(suggestion.verb);
    const staticEntry = PALETTE_VERBS.find(v => v.verb === verb);
    // Use server hint, fall back to static placeholder
    const placeholder = suggestion.hint || staticEntry?.placeholder || null;
    return {
        verb,
        label: extractVerbLabel(suggestion.verb),
        placeholder,
    };
}

export const VerbPalette: React.FC<VerbPaletteProps> = ({ visible, onVerbSelect }) => {
    const paletteRef = useRef<HTMLDivElement>(null);
    const [scrollPosition, setScrollPosition] = useState<ScrollPosition>("no-overflow");
    const touchStartRef = useRef<{ x: number; y: number; verb: string } | null>(null);

    // Mouse drag-to-scroll state
    const isDraggingRef = useRef(false);
    const dragStartXRef = useRef(0);
    const scrollStartRef = useRef(0);
    const hasDraggedRef = useRef(false);

    // Get auth context for RPC calls
    const { authState } = useAuthContext();
    const authToken = authState.player?.authToken ?? null;
    const playerOid = authState.player?.oid ?? null;

    // Fetch verb suggestions from server
    const { suggestions, available } = useVerbSuggestions(authToken, playerOid);

    // Build display list: use server suggestions if available, otherwise static fallback
    // Sort @-prefixed verbs to the end
    const displayVerbs = useMemo((): PaletteVerb[] => {
        let verbs: PaletteVerb[];
        if (available && suggestions.length > 0) {
            verbs = suggestions.map(suggestionToDisplay);
        } else {
            verbs = PALETTE_VERBS;
        }
        // Sort: non-@ verbs first, then @-verbs
        return verbs.sort((a, b) => {
            const aIsAt = a.verb.startsWith("@");
            const bIsAt = b.verb.startsWith("@");
            if (aIsAt === bIsAt) return 0;
            return aIsAt ? 1 : -1;
        });
    }, [available, suggestions]);

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

    // Mouse drag-to-scroll handlers
    const handleMouseDown = useCallback((e: React.MouseEvent) => {
        const el = paletteRef.current;
        if (!el) return;

        isDraggingRef.current = true;
        hasDraggedRef.current = false;
        dragStartXRef.current = e.clientX;
        scrollStartRef.current = el.scrollLeft;
        el.style.cursor = "grabbing";
        el.style.userSelect = "none";
    }, []);

    const handleMouseMove = useCallback((e: React.MouseEvent) => {
        if (!isDraggingRef.current) return;
        const el = paletteRef.current;
        if (!el) return;

        const deltaX = e.clientX - dragStartXRef.current;
        if (Math.abs(deltaX) > 3) {
            hasDraggedRef.current = true;
        }
        el.scrollLeft = scrollStartRef.current - deltaX;
    }, []);

    const handleMouseUp = useCallback(() => {
        const el = paletteRef.current;
        if (el) {
            el.style.cursor = "";
            el.style.userSelect = "";
        }
        isDraggingRef.current = false;
    }, []);

    const handleMouseLeave = useCallback(() => {
        if (isDraggingRef.current) {
            handleMouseUp();
        }
    }, [handleMouseUp]);

    // Prevent click if we just dragged
    const handleClick = useCallback((e: React.MouseEvent, verb: string, placeholder: string | null) => {
        if (hasDraggedRef.current) {
            e.preventDefault();
            e.stopPropagation();
            return;
        }
        onVerbSelect(verb, placeholder);
    }, [onVerbSelect]);

    const handlePointerDown = useCallback((e: React.PointerEvent, verb: string) => {
        // Only track touch/pen, not mouse (mouse has click)
        if (e.pointerType === "mouse") return;
        touchStartRef.current = { x: e.clientX, y: e.clientY, verb };
    }, []);

    const handlePointerUp = useCallback((e: React.PointerEvent, verb: string, placeholder: string | null) => {
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
            onVerbSelect(verb, placeholder);
        }

        touchStartRef.current = null;
    }, [onVerbSelect]);

    if (!visible) return null;

    const showLeftIndicator = scrollPosition === "scrolled-middle" || scrollPosition === "scrolled-end";
    const showRightIndicator = scrollPosition === "scrolled-start" || scrollPosition === "scrolled-middle";

    return (
        <div className="verb-palette-wrapper">
            {showLeftIndicator && (
                <div className="verb-palette-indicator verb-palette-indicator-left" aria-hidden="true">‹</div>
            )}
            <div className={`verb-palette-container ${scrollPosition}`}>
                <div
                    ref={paletteRef}
                    className="verb-palette"
                    role="toolbar"
                    aria-label="Quick command buttons. Select a verb to start a command. Swipe down on a verb to select it."
                    onMouseDown={handleMouseDown}
                    onMouseMove={handleMouseMove}
                    onMouseUp={handleMouseUp}
                    onMouseLeave={handleMouseLeave}
                >
                    {displayVerbs.map(({ verb, label, placeholder }) => (
                        <button
                            key={verb}
                            className="verb-chip"
                            onClick={(e) => handleClick(e, verb, placeholder)}
                            onPointerDown={(e) => handlePointerDown(e, verb)}
                            onPointerUp={(e) => handlePointerUp(e, verb, placeholder)}
                            type="button"
                            aria-label={`${verb} command`}
                        >
                            {label}
                        </button>
                    ))}
                </div>
            </div>
            {showRightIndicator && (
                <div className="verb-palette-indicator verb-palette-indicator-right" aria-hidden="true">›</div>
            )}
        </div>
    );
};
