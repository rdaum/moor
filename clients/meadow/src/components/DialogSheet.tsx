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
    modal?: boolean;
    title: string;
    titleId: string;
    headerActions?: React.ReactNode;
    onCancel: () => void;
    children: React.ReactNode;
    maxWidth?: string;
    role?: "dialog" | "alertdialog";
    ariaDescribedBy?: string;
}

const visibleFocusable = (root: HTMLElement) =>
    Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(element =>
        !element.matches(":disabled") && !element.closest("[hidden], [inert]")
        && element.getClientRects().length > 0
    );

const FOCUSABLE_SELECTOR = "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])";

export const DialogSheet: React.FC<DialogSheetProps> = ({
    modal = true,
    title,
    titleId,
    headerActions,
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
                const focusableElements = visibleFocusable(dialogRef.current);
                if (focusableElements.length > 0) {
                    (focusableElements[0] as HTMLElement).focus();
                } else {
                    // If no focusable elements, focus the dialog itself
                    dialogRef.current.focus();
                }
            }
        };

        // Use requestAnimationFrame to ensure DOM is ready
        const frame = requestAnimationFrame(focusFirstElement);

        // Cleanup: restore focus when dialog unmounts
        return () => {
            cancelAnimationFrame(frame);
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
                e.stopPropagation();
                onCancel();
                return;
            }

            // Focus trapping for Tab key
            if (modal && e.key === "Tab" && dialogRef.current) {
                const focusableElements = visibleFocusable(dialogRef.current);
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
        [onCancel, modal],
    );

    return (
        <>
            {modal && (
                <div className="dialog-sheet-backdrop" onClick={onCancel} role="presentation" aria-hidden="true" />
            )}
            <div
                ref={dialogRef}
                className={`dialog-sheet${modal ? "" : " dialog-sheet-nonmodal"}`}
                style={{ maxWidth }}
                role={role}
                aria-modal={modal}
                aria-labelledby={titleId}
                aria-describedby={ariaDescribedBy}
                tabIndex={-1}
                onKeyDown={handleKeyDown}
            >
                <div className="dialog-sheet-header">
                    <h2 id={titleId}>{title}</h2>
                    {headerActions}
                </div>
                {children}
            </div>
        </>
    );
};
