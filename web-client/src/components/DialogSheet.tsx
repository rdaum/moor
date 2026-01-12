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

import React, { useCallback, useEffect, useRef } from "react";

interface DialogSheetProps {
    title: string;
    titleId: string;
    onCancel: () => void;
    children: React.ReactNode;
    maxWidth?: string;
    role?: "dialog" | "alertdialog";
    ariaDescribedBy?: string;
}

const FOCUSABLE_SELECTOR = "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])";

export const DialogSheet: React.FC<DialogSheetProps> = ({
    title,
    titleId,
    onCancel,
    children,
    maxWidth = "520px",
    role = "dialog",
    ariaDescribedBy,
}) => {
    const dialogRef = useRef<HTMLDivElement>(null);
    const previouslyFocusedRef = useRef<HTMLElement | null>(null);

    // Focus management: trap focus within dialog and restore on close
    useEffect(() => {
        // Store the element that had focus before the dialog opened
        previouslyFocusedRef.current = document.activeElement as HTMLElement;

        // Focus the first focusable element in the dialog
        const focusFirstElement = () => {
            if (dialogRef.current) {
                const focusableElements = dialogRef.current.querySelectorAll(FOCUSABLE_SELECTOR);
                if (focusableElements.length > 0) {
                    (focusableElements[0] as HTMLElement).focus();
                } else {
                    // If no focusable elements, focus the dialog itself
                    dialogRef.current.focus();
                }
            }
        };

        // Use requestAnimationFrame to ensure DOM is ready
        requestAnimationFrame(focusFirstElement);

        // Cleanup: restore focus when dialog unmounts
        return () => {
            if (previouslyFocusedRef.current && previouslyFocusedRef.current.focus) {
                previouslyFocusedRef.current.focus();
            }
        };
    }, []);

    // Handle keyboard events for focus trapping and escape
    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent) => {
            if (e.key === "Escape") {
                e.preventDefault();
                onCancel();
                return;
            }

            // Focus trapping for Tab key
            if (e.key === "Tab" && dialogRef.current) {
                const focusableElements = dialogRef.current.querySelectorAll(FOCUSABLE_SELECTOR);
                if (focusableElements.length === 0) return;

                const firstElement = focusableElements[0] as HTMLElement;
                const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                if (e.shiftKey) {
                    // Shift+Tab: if focus is on first element, wrap to last
                    if (document.activeElement === firstElement) {
                        e.preventDefault();
                        lastElement.focus();
                    }
                } else {
                    // Tab: if focus is on last element, wrap to first
                    if (document.activeElement === lastElement) {
                        e.preventDefault();
                        firstElement.focus();
                    }
                }
            }
        },
        [onCancel],
    );

    return (
        <>
            <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            <div
                ref={dialogRef}
                className="dialog-sheet"
                style={{ maxWidth }}
                role={role}
                aria-modal="true"
                aria-labelledby={titleId}
                aria-describedby={ariaDescribedBy}
                tabIndex={-1}
                onKeyDown={handleKeyDown}
            >
                <div className="dialog-sheet-header">
                    <h2 id={titleId}>{title}</h2>
                </div>
                {children}
            </div>
        </>
    );
};
